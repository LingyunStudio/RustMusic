//! 输出设备管理：共享模式输出流构建/重建、设备偏好持久化与热切换、设备枚举。
use std::sync::atomic::Ordering;
use std::sync::Arc;

use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{MixerDeviceSink, Player};

use super::{Engine, OutputDeviceInfo};
use crate::wasapi_out;
use crate::eq::EqSource;

impl Engine {
    /// 按设备偏好创建输出流与 Player；None = 系统默认设备。
    /// MixerDeviceSink 持有 cpal Stream（非 Send）：泄漏保活整个进程周期
    /// （与旧实现一致），设备切换时旧 stream 一起泄漏（仅结构体大小，代价可忽略）。
    #[allow(deprecated)]
    // 设备名持久化在设置里做匹配依据，description() 格式不同会导致已选设备失配
    pub(super) fn build_output(pref: Option<&str>) -> Result<(&'static MixerDeviceSink, Player), String> {
        let host = rodio::cpal::default_host();
        let device = match pref {
            Some(name) => host
                .output_devices()
                .map_err(|e| format!("枚举输出设备失败: {e}"))?
                .find(|d| d.name().as_deref().map(|n| n == name).unwrap_or(false))
                .ok_or_else(|| format!("输出设备「{name}」不存在"))?,
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
    #[allow(deprecated)] // 同 build_output：name() 是设备偏好的持久化键
    pub fn current_device_name(&self) -> String {
        // cpal 无"stream 绑定的设备"查询；按偏好返回，无偏好时取系统默认
        if let Some(name) = self.device_pref.read().as_deref() {
            return name.to_string();
        }
        rodio::cpal::default_host()
            .default_output_device()
            .and_then(|d| d.name().ok())
            .unwrap_or_default()
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

    /// 枚举输出设备（含"当前默认"标记）
    #[allow(deprecated)] // 同 build_output
    pub fn list_output_devices(&self) -> Vec<OutputDeviceInfo> {
        let host = rodio::cpal::default_host();
        let default_name = host
            .default_output_device()
            .and_then(|d| d.name().ok())
            .unwrap_or_default();
        let mut out = Vec::new();
        if let Ok(devices) = host.output_devices() {
            for d in devices {
                if let Ok(name) = d.name() {
                    out.push(OutputDeviceInfo {
                        is_default: name == default_name,
                        name,
                    });
                }
            }
        }
        out
    }
}
