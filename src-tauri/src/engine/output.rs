//! 输出设备管理：共享模式输出流构建/重建、设备偏好持久化与热切换、设备枚举。
#![allow(deprecated)] // cpal name() 已废弃但仍作展示名兜底；设备名持久化以全名匹配
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{MixerDeviceSink, Player};

use super::{Engine, OutputDeviceInfo};
use crate::eq::EqSource;
use crate::wasapi_out;

/// WASAPI 渲染端点集合：端点 ID → 完整 FriendlyName（PKEY_Device_FriendlyName，
/// 形如「耳机 (WH-1000XM5 立体声)」）。
/// cpal 0.17 的 name() 优先取 DeviceDesc，蓝牙耳机等设备只会得到「耳机」
/// 这类短词——设备展示与匹配一律以这里的全名为准。
#[cfg(windows)]
fn wasapi_name_map() -> HashMap<String, String> {
    let mut m = HashMap::new();
    let _ = wasapi::initialize_mta();
    let Ok(coll) = wasapi::DeviceCollection::new(&wasapi::Direction::Render) else {
        return m;
    };
    for dev in coll.into_iter().flatten() {
        if let (Ok(id), Ok(name)) = (dev.get_id(), dev.get_friendlyname()) {
            if !name.is_empty() {
                m.insert(id, name);
            }
        }
    }
    m
}

/// 非 Windows：无 WASAPI 全名可用，统一走 cpal 展示名（ALSA/PulseAudio 名）
#[cfg(not(windows))]
fn wasapi_name_map() -> HashMap<String, String> {
    HashMap::new()
}

/// 单个 cpal 设备的展示名（WASAPI 全名优先，cpal 短名兜底）
fn display_name(dev: &rodio::cpal::Device, names: &HashMap<String, String>) -> String {
    if let Ok(id) = dev.id() {
        if let Some(n) = names.get(&id.1) {
            return n.clone();
        }
    }
    dev.name().unwrap_or_default()
}

/// 设备匹配：先按 WASAPI 全名（大小写不敏感），再按 cpal 短名兜底
/// （兼容旧版本以 DeviceDesc 短名持久化的 output_device 设置）
fn device_matches(dev: &rodio::cpal::Device, names: &HashMap<String, String>, pref: &str) -> bool {
    if display_name(dev, names).eq_ignore_ascii_case(pref) {
        return true;
    }
    dev.name()
        .as_deref()
        .map(|n| n.eq_ignore_ascii_case(pref))
        .unwrap_or(false)
}

/// 系统默认输出设备的展示名（WASAPI 全名优先，cpal 短名兜底）
#[cfg(windows)]
pub(crate) fn default_device_display_name() -> String {
    let _ = wasapi::initialize_mta();
    let full = wasapi::get_default_device(&wasapi::Direction::Render)
        .ok()
        .and_then(|d| d.get_friendlyname().ok())
        .filter(|n| !n.is_empty());
    if let Some(n) = full {
        return n;
    }
    rodio::cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default()
}

/// 非 Windows：cpal 默认输出设备名（device_watcher 的轮询比较基准）
#[cfg(not(windows))]
pub(crate) fn default_device_display_name() -> String {
    rodio::cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default()
}

impl Engine {
    /// 按设备偏好创建输出流与 Player；None = 系统默认设备。
    /// MixerDeviceSink 持有 cpal Stream（非 Send）：泄漏保活整个进程周期
    /// （与旧实现一致），设备切换时旧 stream 一起泄漏（仅结构体大小，代价可忽略）。
    pub(super) fn build_output(
        pref: Option<&str>,
    ) -> Result<(&'static MixerDeviceSink, Player), String> {
        let host = rodio::cpal::default_host();
        let device = match pref {
            Some(name) => {
                let names = wasapi_name_map();
                host.output_devices()
                    .map_err(|e| format!("枚举输出设备失败: {e}"))?
                    .find(|d| device_matches(d, &names, name))
                    .ok_or_else(|| format!("输出设备「{name}」不存在"))?
            }
            None => host
                .default_output_device()
                .ok_or("没有可用的音频输出设备")?,
        };
        let leaked: &'static MixerDeviceSink = Box::leak(Box::new(
            rodio::DeviceSinkBuilder::from_device(device)
                .and_then(|b| b.open_stream())
                .map_err(|e| format!("打开输出设备失败: {e}"))?,
        ));
        let sink = Player::connect_new(leaked.mixer());
        sink.pause();
        Ok((leaked, sink))
    }

    /// 重建共享模式输出流与 Sink（独占会话跑过之后共享流已失效时调用）
    pub(super) fn rebuild_shared_output(&self) {
        match Self::build_output(self.device_pref.read().as_deref()) {
            Ok((handle, sink)) => {
                let mut old = self.sink.write();
                old.stop();
                *old = sink;
                *self.out.write() = handle;
                eprintln!("[engine] 共享输出流已重建");
            }
            Err(e) => eprintln!("[engine] 重建共享输出流失败: {e}"),
        }
    }

    /// 当前使用的输出设备名
    pub fn current_device_name(&self) -> String {
        // cpal 无"stream 绑定的设备"查询；按偏好返回，无偏好时取系统默认
        if let Some(name) = self.device_pref.read().as_deref() {
            return name.to_string();
        }
        default_device_display_name()
    }

    pub fn device_preference(&self) -> Option<String> {
        self.device_pref.read().clone()
    }

    pub fn set_device_preference(&self, name: Option<&str>) {
        *self.device_pref.write() = name.map(|s| s.to_string());
    }

    /// 切换输出设备：重建输出流与 Sink，当前曲目从进度处无缝续播
    pub fn switch_output_device(self: &Arc<Self>, name: Option<&str>) -> Result<(), String> {
        // 独占模式：按新设备重建独占会话
        if self.excl_active() {
            self.set_device_preference(name);
            self.rebuilding.store(true, Ordering::Relaxed);
            let result = (|| -> Result<(), String> {
                let info = self
                    .current
                    .read()
                    .clone()
                    .ok_or("当前没有正在播放的曲目")?;
                let pos = self.pos_ms.load(Ordering::Relaxed);
                let was_paused = self.user_paused.load(Ordering::Relaxed);
                self.rebuild_exclusive_at(&info, pos, was_paused)
            })();
            self.rebuilding.store(false, Ordering::Relaxed);
            return result;
        }
        // 重建期间 monitor 会因 sink 短暂为空误判"播完"，借用 rebuilding 标志屏蔽
        self.rebuilding.store(true, Ordering::Relaxed);
        let result = (|| -> Result<(), String> {
            let info = self.current.read().clone();
            let pos = self.pos_ms.load(Ordering::Relaxed);
            let was_paused = self.user_paused.load(Ordering::Relaxed);
            let volume = f32::from_bits(self.volume.load(Ordering::Relaxed));
            let speed = f32::from_bits(self.speed.load(Ordering::Relaxed));

            let (handle, sink) = Self::build_output(name)?;
            {
                let mut old_sink = self.sink.write();
                old_sink.stop();
                *old_sink = sink;
                *self.out.write() = handle;
            }
            self.set_device_preference(name);

            if let Some(info) = info {
                // 当前有曲目：从 pos 处重建播放链。
                // path 一律是本地路径（本地曲目或已缓存的在线音源文件）
                if !info.path.is_empty() {
                    match self.open_current_source(&info.path, pos) {
                        Ok(src) => {
                            let wrapped = EqSource::with_base(
                                src,
                                self.eq.clone(),
                                self.pos_ms.clone(),
                                pos as f64,
                            );
                            let sink = self.sink.read();
                            sink.clear();
                            sink.append(wrapped);
                            sink.set_volume(volume);
                            sink.set_speed(speed);
                            if was_paused {
                                sink.pause();
                            } else {
                                sink.play();
                            }
                            self.stopped.store(false, Ordering::Relaxed);
                        }
                        Err(e) => {
                            eprintln!("[engine] 切换设备后重开音频失败: {e}");
                        }
                    }
                }
            }
            Ok(())
        })();
        self.rebuilding.store(false, Ordering::Relaxed);
        result
    }
    /// 走共享路径前收尾：若独占会话还在播放（上一首独占中 / 独占开关刚关），
    /// 必须先终止并等设备交还系统混音器——否则旧会话继续出声、
    /// 新共享流被独占压制无声（表现为"旧歌不停、新歌无声"）
    pub(super) fn ensure_shared_output(&self) {
        if let Some(ctl) = self.excl.read().clone() {
            let released = wasapi_out::wait_session_exit(&ctl, 2500);
            if !released {
                eprintln!("[engine] 切共享前独占会话未退出，设备可能仍被占用");
            }
            *self.excl.write() = None;
            self.shared_broken.store(true, Ordering::Relaxed);
        }
        if self.shared_broken.swap(false, Ordering::Relaxed) {
            self.rebuild_shared_output();
        }
    }
    // ---------- 输出设备 ----------

    /// 枚举输出设备（含"当前默认"标记）。名称取 WASAPI 完整 FriendlyName
    /// （设备名只显示「耳机」这类短词时用户无法区分设备，全名带适配器信息）
    pub fn list_output_devices(&self) -> Vec<OutputDeviceInfo> {
        let host = rodio::cpal::default_host();
        let names = wasapi_name_map();
        let default_id = host
            .default_output_device()
            .and_then(|d| d.id().ok())
            .map(|i| i.1);
        let mut out = Vec::new();
        if let Ok(devices) = host.output_devices() {
            for d in devices {
                let name = display_name(&d, &names);
                if name.is_empty() {
                    continue;
                }
                let is_default = match (&default_id, d.id().ok()) {
                    (Some(a), Some(b)) => *a == b.1,
                    _ => false,
                };
                out.push(OutputDeviceInfo { is_default, name });
            }
        }
        out
    }

    /// 固定设备是否仍存在（device_watcher 用）。匹配规则与 build_output
    /// 一致：WASAPI 全名优先 + cpal 短名兜底——旧版本以短名（如「耳机」）
    /// 持久化的偏好不能被误判“已不存在”而清掉跟随系统
    pub fn is_known_device(&self, pref: &str) -> bool {
        let host = rodio::cpal::default_host();
        let names = wasapi_name_map();
        let Ok(devices) = host.output_devices() else {
            return false;
        };
        devices
            .into_iter()
            .any(|d| device_matches(&d, &names, pref))
    }
}
