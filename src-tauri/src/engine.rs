use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use lofty::prelude::*;
use parking_lot::RwLock;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, MixerDeviceSink, Player, Source};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::eq::{EqShared, EqSource};
use crate::smtc::SmtcMsg;
use crate::wasapi_out;

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub id: Option<i64>,
    /// "track"（本地曲目）| "url"（自定义在线音源）| "netease"（网易云在线曲库）
    pub kind: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: String,
    pub duration_ms: u64,
    #[serde(default)]
    pub nid: Option<i64>,
    #[serde(default)]
    pub qid: Option<String>,
    /// 酷狗曲目 hash（酷狗在线曲库）
    #[serde(default)]
    pub kgid: Option<String>,
    /// 播放音质描述（如 "320kbps" / "FLAC"），来自取链接响应
    #[serde(default)]
    pub quality: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayState {
    #[serde(flatten)]
    pub info: TrackInfo,
    pub playing: bool,
    /// 单调递增的"开播代次"：start() 每次 +1；前端据此区分
    /// "换曲开播"（进度归零）与"暂停/恢复"（保留进度）
    #[serde(default)]
    pub seq: u64,
}

/// 播放状态快照（含进度）：WebView 挂起恢复后前端主动拉取，
/// 作为恢复窗口期事件推送可能丢失时的权威同步手段
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayStateSnapshot {
    #[serde(flatten)]
    pub state: PlayState,
    pub pos: u64,
}

/// 输出设备信息（前端下拉用）
#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutputDeviceInfo {
    /// cpal 设备名（唯一标识）
    pub name: String,
    /// 是否当前系统默认输出设备
    pub is_default: bool,
}

pub struct Engine {
    /// 当前输出设备句柄（持有 cpal Stream 保活；设备切换时重建）
    out: RwLock<&'static MixerDeviceSink>,
    sink: RwLock<Player>,
    /// 边下边播：当前由流式下载供数的曲目（.part 路径 → 共享下载状态）。
    /// seek/设备切换/重建播放链时据此走阻塞式流读取器而不是直接开文件
    active_streaming: RwLock<Option<(String, Arc<crate::streaming::StreamingDownload>)>>,
    app: AppHandle,
    pub eq: Arc<EqShared>,
    pub pos_ms: Arc<AtomicU64>,
    pub dur_ms: Arc<AtomicU64>,
    pub user_paused: Arc<AtomicBool>,
    pub stopped: Arc<AtomicBool>,
    /// FLAC seek 重建播放链期间置位，避免 monitor 误判"播完"
    pub rebuilding: Arc<AtomicBool>,
    /// start() 换曲瞬间（clear 与 append 之间）置位，避免 monitor 误判"播完"
    pub switching: Arc<AtomicBool>,
    pub current: Arc<RwLock<Option<TrackInfo>>>,
    volume: Arc<AtomicU32>,
    speed: Arc<AtomicU32>,
    play_seq: AtomicU64,
    want_url: Arc<RwLock<Option<String>>>,
    /// 正在后台下载的 URL 集合，防止同一 URL 并发下载写坏缓存文件
    downloading: RwLock<HashSet<String>>,
    downloads_dir: PathBuf,
    /// 缓存上限（字节），0 = 不限制；下载完成后超限即 LRU 清理
    cache_limit: AtomicU64,
    smtc: Sender<SmtcMsg>,
    /// 用户指定的输出设备名（None = 跟随系统默认，设备热插拔时自动切换）
    device_pref: RwLock<Option<String>>,
    /// WASAPI 独占模式开关（设置项；切换后下一首生效）
    exclusive_enabled: AtomicBool,
    /// 独占会话建立后置位：共享模式的输出流会被系统标记失效，
    /// 回退/切回共享模式播放前需重建输出流
    shared_broken: AtomicBool,
    /// 活跃的独占播放会话（共享模式播放时为 None）
    excl: RwLock<Option<wasapi_out::ExclusiveCtl>>,
}

impl Engine {
    pub fn new(
        app: AppHandle,
        app_data: &Path,
        volume: f32,
        speed: f32,
        eq: Arc<EqShared>,
        cache_limit: u64,
        smtc: Sender<SmtcMsg>,
    ) -> Result<Self, String> {
        let (handle, sink) = Self::build_output(None)?;
        let downloads_dir = app_data.join("downloads");
        let _ = std::fs::create_dir_all(&downloads_dir);
        // 旧版缓存按 URL 哈希命名（无 net-/qq- 前缀），链接签名变化导致
        // 这些文件永不再命中，启动时清掉以免白占磁盘
        if let Ok(rd) = std::fs::read_dir(&downloads_dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let recognized =
                    name.starts_with("net-") || name.starts_with("qq-") || name.starts_with("url-");
                if !recognized {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        Ok(Self {
            out: RwLock::new(handle),
            sink: RwLock::new(sink),
            active_streaming: RwLock::new(None),
            app,
            eq,
            pos_ms: Arc::new(AtomicU64::new(0)),
            dur_ms: Arc::new(AtomicU64::new(0)),
            user_paused: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(true)),
            rebuilding: Arc::new(AtomicBool::new(false)),
            switching: Arc::new(AtomicBool::new(false)),
            current: Arc::new(RwLock::new(None)),
            volume: Arc::new(AtomicU32::new(volume.to_bits())),
            speed: Arc::new(AtomicU32::new(speed.to_bits())),
            play_seq: AtomicU64::new(0),
            want_url: Arc::new(RwLock::new(None)),
            downloading: RwLock::new(HashSet::new()),
            downloads_dir,
            cache_limit: AtomicU64::new(cache_limit),
            smtc,
            device_pref: RwLock::new(None),
            exclusive_enabled: AtomicBool::new(false),
            shared_broken: AtomicBool::new(false),
            excl: RwLock::new(None),
        })
    }

    /// 按设备偏好创建输出流与 Player；None = 系统默认设备。
    /// MixerDeviceSink 持有 cpal Stream（非 Send）：泄漏保活整个进程周期
    /// （与旧实现一致），设备切换时旧 stream 一起泄漏（仅结构体大小，代价可忽略）。
    fn build_output(pref: Option<&str>) -> Result<(&'static MixerDeviceSink, Player), String> {
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
    fn rebuild_shared_output(&self) {
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

    // ---------- 播放 ----------

    pub fn play_file(&self, info: TrackInfo) -> Result<(), String> {
        *self.want_url.write() = None;
        // 本地/完整缓存播放：清除流式下载关联（若有）。
        // 边下边播起播也走本函数——此时流式关联正是这首，保留供读取器使用
        let keep_streaming = self
            .active_streaming
            .read()
            .as_ref()
            .map_or(false, |(p, _)| *p == info.path);
        if !keep_streaming {
            *self.active_streaming.write() = None;
        }
        if self.exclusive_enabled.load(Ordering::Relaxed) {
            match self.start_exclusive(&info, 0) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    eprintln!("[engine] WASAPI 独占模式不可用，本次回退共享模式: {e}");
                    // 前端 toast 提示用户独占未生效（音量混音器仍在工作）
                    let _ = self.app.emit(
                        "player://exclusive-fallback",
                        serde_json::json!({ "reason": e }),
                    );
                }
            }
        }
        self.ensure_shared_output();
        let src = self.open_current_source(&info.path, 0)?;
        self.start(src, info)
    }

    /// 走共享路径前收尾：若独占会话还在播放（上一首独占中 / 独占开关刚关），
    /// 必须先终止并等设备交还系统混音器——否则旧会话继续出声、
    /// 新共享流被独占压制无声（表现为"旧歌不停、新歌无声"）
    fn ensure_shared_output(&self) {
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

    /// 按当前曲目打开采样源：若该曲目正由流式下载供数（边下边播），
    /// 走阻塞式流读取器（读到下载前沿等待新数据），否则直接打开本地文件。
    fn open_current_source(&self, path: &str, skip_ms: u64) -> Result<Box<dyn Source + Send>, String> {
        let streaming = self.active_streaming.read().clone();
        if let Some((spath, shared)) = streaming {
            if spath == path {
                if shared.is_mp4() {
                    return crate::symdec::SymphoniaSource::open_streaming(shared, skip_ms)
                        .map(|s| Box::new(s) as Box<dyn Source + Send>);
                }
                // 非 MP4 容器的流式文件（mp3/flac 等）走 rodio，同样喂流读取器
                let mut builder = Decoder::builder().with_seekable(true);
                let total = shared.snapshot().1;
                if total > 0 {
                    builder = builder.with_byte_len(total);
                }
                let file = crate::streaming::StreamingFile::open(shared)
                    .map_err(|e| format!("打开下载缓存失败: {e}"))?;
                let decoder = builder
                    .with_data(file)
                    .build()
                    .map_err(|e| format!("无法解码该音频文件: {e}"))?;
                return Ok(Box::new(
                    decoder.skip_duration(Duration::from_millis(skip_ms)),
                ));
            }
        }
        open_playable_source(path, skip_ms)
    }

    /// 启动 WASAPI 独占播放会话（skip_ms 用于 seek/重建时跳过开头）。
    /// 初始化在会话线程完成，此处阻塞等待协商结果（最多 3s）；
    /// 失败时采样源已被会话线程取走，由调用方重新解码回退共享模式。
    fn start_exclusive(&self, info: &TrackInfo, skip_ms: u64) -> Result<(), String> {
        self.rebuilding.store(true, Ordering::Relaxed);
        let result = (|| -> Result<(), String> {
            // 终止旧会话并等线程真正退出（设备随音频客户端释放）：
            // 旧会话还占着设备时开新会话，Initialize 会全部撞上 DEVICE_IN_USE
            if let Some(ctl) = self.excl.read().clone() {
                if !wasapi_out::wait_session_exit(&ctl, 2500) {
                    eprintln!("[engine] 旧独占会话未按时退出，继续尝试新会话");
                }
            }
            *self.excl.write() = None;
            // DASH（B 站 fMP4）由 helper 走 symphonia 直连源（rodio 会 panic），
            // 其余格式 rodio 解码 + skip_duration
            let src = self.open_current_source(&info.path, skip_ms)?;
            let wrapped =
                EqSource::with_base(src, self.eq.clone(), self.pos_ms.clone(), skip_ms as f64);
            let params = wasapi_out::ExclusiveParams {
                device_pref: self.device_pref.read().clone(),
                channels: wrapped.channels().get() as usize,
                src_rate: wrapped.sample_rate().get(),
                volume_bits: self.volume.clone(),
                speed_bits: self.speed.clone(),
            };
            let (ctl, rx) = wasapi_out::spawn_exclusive_session(wrapped, params)?;
            // 会话一创建就注册控制句柄：协商期间超时/切歌/关开关也能终止它，
            // 防止孤儿线程晚一步拿到设备后无人可控（表现为其它软件一直无声）
            *self.excl.write() = Some(ctl.clone());
            match rx.recv_timeout(Duration::from_secs(3)) {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    // 会话已退出（结果在退出后才发出），设备已释放
                    *self.excl.write() = None;
                    return Err(e);
                }
                Err(_) => {
                    // 协商超时：要求会话退出并等设备释放
                    let released = wasapi_out::wait_session_exit(&ctl, 2500);
                    if released {
                        *self.excl.write() = None;
                    } else {
                        // 线程卡在驱动调用里：保留句柄以便后续继续尝试终止，
                        // 并标记共享流可能失效（设备或被晚到的会话占用）
                        eprintln!("[engine] 独占会话协商超时且未退出，保留控制句柄");
                        self.shared_broken.store(true, Ordering::Relaxed);
                    }
                    return Err("独占模式初始化超时".into());
                }
            }
            self.shared_broken.store(true, Ordering::Relaxed);
            self.sink.read().clear();
            self.pos_ms.store(skip_ms, Ordering::Relaxed);
            self.dur_ms.store(info.duration_ms, Ordering::Relaxed);
            self.user_paused.store(false, Ordering::Relaxed);
            self.stopped.store(false, Ordering::Relaxed);
            Ok(())
        })();
        self.rebuilding.store(false, Ordering::Relaxed);
        if result.is_ok() {
            let info = info.clone();
            *self.current.write() = Some(info.clone());
            self.notify_smtc();
            let seq = self.play_seq.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = self.app.emit(
                "player://state",
                PlayState {
                    playing: true,
                    info,
                    seq,
                },
            );
        }
        result
    }

    /// 重建独占会话（seek / 倍速 / 设备切换时），保持暂停状态
    fn rebuild_exclusive_at(
        &self,
        info: &TrackInfo,
        ms: u64,
        was_paused: bool,
    ) -> Result<(), String> {
        let result = self.start_exclusive(info, ms);
        if result.is_ok() {
            if let Some(ctl) = self.excl.read().as_ref() {
                ctl.paused.store(was_paused, Ordering::Relaxed);
            }
            self.user_paused.store(was_paused, Ordering::Relaxed);
        }
        result
    }

    /// 停止独占会话并立即切回共享模式（从当前进度续播，保持暂停状态）。
    /// 用于关闭独占开关时立刻把设备还给系统混音器，恢复其它应用出声。
    pub fn stop_exclusive_resume_shared(self: &Arc<Self>) -> Result<(), String> {
        // 没有活跃独占会话、共享流也未被破坏时无需动作：
        // 避免反复拨动开关时无故重启共享播放链（当前曲目会跳一下）
        if !self.excl_active() && !self.shared_broken.load(Ordering::Relaxed) {
            return Ok(());
        }
        let info = self.current.read().clone();
        let pos = self.pos_ms.load(Ordering::Relaxed);
        let was_paused = self.user_paused.load(Ordering::Relaxed);
        // 已手动停止：只交还设备、重建共享流，不重新开始播放
        let was_stopped = self.stopped.load(Ordering::Relaxed);
        // 必须先等会话线程退出、设备交还系统混音器，再重建共享输出流：
        // 设备被独占期间新共享流打不开，会落得"关了独占还是无声"
        if let Some(ctl) = self.excl.read().clone() {
            if !wasapi_out::wait_session_exit(&ctl, 2500) {
                eprintln!("[engine] 独占会话未按时退出，设备可能仍被占用");
            }
        }
        *self.excl.write() = None;
        self.shared_broken.store(true, Ordering::Relaxed);
        self.rebuilding.store(true, Ordering::Relaxed);
        let result = (|| -> Result<(), String> {
            self.rebuild_shared_output();
            if let Some(info) = &info {
                if was_stopped {
                    return Ok(());
                }
                let src = self.open_current_source(&info.path, pos)?;
                let wrapped =
                    EqSource::with_base(src, self.eq.clone(), self.pos_ms.clone(), pos as f64);
                {
                    let sink = self.sink.read();
                    sink.clear();
                    sink.append(wrapped);
                    sink.set_volume(f32::from_bits(self.volume.load(Ordering::Relaxed)));
                    sink.set_speed(f32::from_bits(self.speed.load(Ordering::Relaxed)));
                    if was_paused {
                        sink.pause();
                    } else {
                        sink.play();
                    }
                }
                self.stopped.store(false, Ordering::Relaxed);
            }
            Ok(())
        })();
        self.rebuilding.store(false, Ordering::Relaxed);
        self.notify_smtc();
        self.emit_state(!was_paused && info.is_some() && !was_stopped);
        result
    }

    fn excl_active(&self) -> bool {
        self.excl
            .read()
            .as_ref()
            .map(|c| c.active.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    fn start<S>(&self, src: S, info: TrackInfo) -> Result<(), String>
    where
        S: Source<Item = f32> + Send + 'static,
    {
        let wrapped = EqSource::new(src, self.eq.clone(), self.pos_ms.clone());
        let diag_sr = wrapped.sample_rate();
        let diag_ch = wrapped.channels();
        let diag_dur = wrapped.total_duration();
        self.pos_ms.store(0, Ordering::Relaxed);
        // 边下边播起播时元数据可能未带时长：用解码器上报的总时长兜底
        let dur_ms = if info.duration_ms > 0 {
            info.duration_ms
        } else {
            diag_dur.map(|d| d.as_millis() as u64).unwrap_or(0)
        };
        self.dur_ms.store(dur_ms, Ordering::Relaxed);
        // 换曲瞬间 sink 短暂为空，置位避免 monitor 采样到 empty 误判"播完"
        self.switching.store(true, Ordering::Relaxed);
        let sink = self.sink.read();
        sink.clear();
        sink.append(wrapped);
        drop(sink);
        self.switching.store(false, Ordering::Relaxed);
        sink_play_common(
            &self.sink,
            self.volume.load(Ordering::Relaxed),
            self.speed.load(Ordering::Relaxed),
        );
        self.user_paused.store(false, Ordering::Relaxed);
        self.stopped.store(false, Ordering::Relaxed);
        #[cfg(debug_assertions)]
        eprintln!(
            "[engine] started: kind={} path={} total_duration={:?} sr={} ch={}",
            info.kind, info.path, diag_dur, diag_sr, diag_ch,
        );
        *self.current.write() = Some(info.clone());
        self.notify_smtc();
        let seq = self.play_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.app.emit(
            "player://state",
            PlayState {
                playing: true,
                info,
                seq,
            },
        );
        Ok(())
    }

    pub fn pause(&self) {
        if self.excl_active() {
            if let Some(ctl) = self.excl.read().as_ref() {
                ctl.paused.store(true, Ordering::Relaxed);
            }
            self.user_paused.store(true, Ordering::Relaxed);
            self.notify_smtc();
            self.emit_state(false);
            return;
        }
        let sink = self.sink.read();
        if sink.empty() {
            return;
        }
        sink.pause();
        drop(sink);
        self.user_paused.store(true, Ordering::Relaxed);
        self.notify_smtc();
        self.emit_state(false);
    }

    pub fn resume(&self) {
        // 曲目已自然播完（托盘隐藏期间"播完"事件可能丢失）：重启当前曲目，
        // 否则播放按钮会永远无响应（早期 return 既不播放也不发状态）
        if self.excl_active() {
            if let Some(ctl) = self.excl.read().as_ref() {
                ctl.paused.store(false, Ordering::Relaxed);
            }
            self.user_paused.store(false, Ordering::Relaxed);
            self.stopped.store(false, Ordering::Relaxed);
            self.notify_smtc();
            self.emit_state(true);
            return;
        }
        if self.sink.read().empty() {
            let info = self.current.read().clone();
            if let Some(info) = info {
                let _ = self.play_file(info);
            }
            return;
        }
        let sink = self.sink.read();
        sink.play();
        drop(sink);
        self.user_paused.store(false, Ordering::Relaxed);
        self.stopped.store(false, Ordering::Relaxed);
        self.notify_smtc();
        self.emit_state(true);
    }

    pub fn toggle(&self) {
        // sink 已空（停止/自然播完）时"播放"应重启当前曲目而不是走 pause 早退
        if self.excl_active() {
            if self.user_paused.load(Ordering::Relaxed) {
                self.resume();
            } else {
                self.pause();
            }
            return;
        }
        if self.user_paused.load(Ordering::Relaxed) || self.sink.read().empty() {
            self.resume();
        } else {
            self.pause();
        }
    }

    pub fn stop(&self) {
        // 先置 stopped 再停流：monitor 判定"自然播完"要求 !stopped，
        // 若先停流（is_active 变 false）后置 stopped，120ms tick 恰好
        // 采样在两步之间会误发 player://ended → 前端自动切歌
        self.stopped.store(true, Ordering::Relaxed);
        self.user_paused.store(false, Ordering::Relaxed);
        self.pos_ms.store(0, Ordering::Relaxed);
        // 停播即解除流式下载关联（下载本身继续至完成，最终化 .part 缓存）
        *self.active_streaming.write() = None;
        if self.excl_active() {
            if let Some(ctl) = self.excl.read().as_ref() {
                ctl.stop.store(true, Ordering::Relaxed);
                ctl.active.store(false, Ordering::Relaxed);
            }
            self.notify_smtc();
            self.emit_state(false);
            return;
        }
        let sink = self.sink.read();
        sink.stop();
        drop(sink);
        self.notify_smtc();
        self.emit_state(false);
    }

    pub fn seek(&self, ms: u64) -> Result<(), String> {
        // 独占模式：解码器在会话线程内，seek 走会话重建（同 FLAC 策略）
        if self.excl_active() {
            let info = self
                .current
                .read()
                .clone()
                .ok_or("当前没有正在播放的曲目")?;
            let was_paused = self.user_paused.load(Ordering::Relaxed);
            return self.rebuild_exclusive_at(&info, ms, was_paused);
        }
        let sink = self.sink.read();
        if sink.empty() {
            return Err("当前没有正在播放的曲目".into());
        }
        // FLAC：解码器不支持 seek，失败的 try_seek 还会重置解码器状态
        // （表现为进度先跳回开头再跳目标），直接走重建路径
        let info_opt = self.current.read().clone();
        let is_streaming = info_opt
            .as_ref()
            .map(|i| {
                self.active_streaming
                    .read()
                    .as_ref()
                    .map_or(false, |(p, _)| *p == i.path)
            })
            .unwrap_or(false);
        let is_flac = info_opt
            .as_ref()
            .map(|i| {
                // FLAC 与 MP4 容器（symdec 源）解码器都不支持就地 seek：走重建
                i.path.to_lowercase().ends_with(".flac") || is_mp4_container(&i.path)
            })
            .unwrap_or(false)
            || is_streaming;
        if is_flac {
            let info = info_opt.ok_or("当前没有正在播放的曲目")?;
            drop(sink);
            self.rebuilding.store(true, Ordering::Relaxed);
            let r = self.rebuild_at(&info, ms);
            self.rebuilding.store(false, Ordering::Relaxed);
            return r;
        }
        drop(info_opt);
        // 常规 seek；MP3 边界位置偶发失败时回退 300ms 重试
        match sink.try_seek(Duration::from_millis(ms)) {
            Ok(()) => Ok(()),
            Err(first) => {
                let back = ms.saturating_sub(300);
                match sink.try_seek(Duration::from_millis(back)) {
                    Ok(()) => Ok(()),
                    Err(_) => Err(format!("定位失败: {first}")),
                }
            }
        }
    }

    /// FLAC / MP4 容器（m4a、B 站 DASH 缓存等）专用：重开文件并丢弃到目标时长，重建播放链。
    /// 边下边播中的曲目同样适用（流读取器按包跳转到目标位置，缺失数据阻塞等待）。
    fn rebuild_at(&self, info: &TrackInfo, ms: u64) -> Result<(), String> {
        let src = self.open_current_source(&info.path, ms)?;
        let wrapped = EqSource::with_base(src, self.eq.clone(), self.pos_ms.clone(), ms as f64);
        self.pos_ms.store(ms, Ordering::Relaxed);
        self.dur_ms.store(info.duration_ms, Ordering::Relaxed);
        let sink = self.sink.read();
        sink.clear();
        sink.append(wrapped);
        sink.set_volume(f32::from_bits(self.volume.load(Ordering::Relaxed)));
        sink.set_speed(f32::from_bits(self.speed.load(Ordering::Relaxed)));
        let was_paused = self.user_paused.load(Ordering::Relaxed);
        if was_paused {
            sink.pause();
        } else {
            sink.play();
        }
        drop(sink);
        Ok(())
    }

    // ---------- 音量 / 速度 / 均衡器 ----------

    pub fn set_volume(&self, v: f32) {
        let v = v.clamp(0.0, 1.0);
        self.volume.store(v.to_bits(), Ordering::Relaxed);
        self.sink.read().set_volume(v);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    pub fn set_speed(&self, v: f32) {
        let v = v.clamp(0.5, 2.0);
        self.speed.store(v.to_bits(), Ordering::Relaxed);
        // 独占模式：倍速通过重建会话（重采样比）应用，从当前进度续播
        if self.excl_active() {
            if let Some(info) = self.current.read().clone() {
                let pos = self.pos_ms.load(Ordering::Relaxed);
                let was_paused = self.user_paused.load(Ordering::Relaxed);
                let _ = self.rebuild_exclusive_at(&info, pos, was_paused);
            }
            return;
        }
        self.sink.read().set_speed(v);
    }

    pub fn speed(&self) -> f32 {
        f32::from_bits(self.speed.load(Ordering::Relaxed))
    }

    // ---------- 状态查询 ----------

    pub fn is_active(&self) -> bool {
        self.excl_active() || !self.sink.read().empty()
    }

    /// 当前是否处于"正在播放"（供状态事件 / SMTC / 快照统一取用）
    fn now_playing(&self) -> bool {
        !self.user_paused.load(Ordering::Relaxed)
            && (self.excl_active() || !self.sink.read().empty())
    }

    /// WASAPI 独占模式开关（设置项；切换后下一首生效）
    pub fn set_exclusive_enabled(&self, enabled: bool) {
        self.exclusive_enabled.store(enabled, Ordering::Relaxed);
        eprintln!(
            "[engine] WASAPI 独占模式 = {}",
            if enabled { "开" } else { "关" }
        );
    }

    fn emit_state(&self, playing: bool) {
        if let Some(info) = self.current.read().clone() {
            let seq = self.play_seq.load(Ordering::Relaxed);
            let _ = self
                .app
                .emit("player://state", PlayState { playing, info, seq });
        }
    }

    /// WebView 挂起恢复后补发当前播放状态与进度（挂起期间发往前端的事件被丢弃）
    pub fn resync_ui(&self) {
        let playing = self.now_playing();
        self.emit_state(playing);
        // 进度帧只在真实播放中补发：引擎空闲时补发 pos(0,0) 会触发前端
        // pos 事件的“playing 自愈”，托盘往返后按钮凭空变成“播放中”
        if playing {
            let _ = self.app.emit(
                "player://pos",
                serde_json::json!({
                    "pos": self.pos_ms.load(Ordering::Relaxed),
                    "dur": self.dur_ms.load(Ordering::Relaxed),
                }),
            );
        }
    }

    /// 播放状态快照（引擎空闲但播过歌时也返回，playing=false）
    pub fn snapshot(&self) -> Option<PlayStateSnapshot> {
        let info = self.current.read().clone()?;
        let playing = self.now_playing();
        Some(PlayStateSnapshot {
            state: PlayState {
                info,
                playing,
                seq: self.play_seq.load(Ordering::Relaxed),
            },
            pos: self.pos_ms.load(Ordering::Relaxed),
        })
    }

    fn notify_smtc(&self) {
        let info = self.current.read().clone();
        let playing = self.now_playing();
        let _ = self.smtc.send(SmtcMsg::Update {
            info,
            playing,
            pos_ms: self.pos_ms.load(Ordering::Relaxed),
        });
    }

    /// 仅刷新系统媒体浮窗进度（播放中由 monitor 周期调用，不重复设置元数据）
    pub fn notify_smtc_pos(&self) {
        if self.current.read().is_none() {
            return;
        }
        let playing = self.now_playing();
        let _ = self.smtc.send(SmtcMsg::Position {
            playing,
            pos_ms: self.pos_ms.load(Ordering::Relaxed),
        });
    }

    // ---------- 输出设备 ----------

    /// 枚举输出设备（含"当前默认"标记）
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

    // ---------- 在线音源 ----------

    /// 稳定的缓存键：网易云/QQ 的 CDN 直链每次请求都带新的签名参数，
    /// 按 URL 哈希命名会导致同一首歌每次播放都重新下载；
    /// 因此用歌曲 ID + 实际音质做键（同一首歌同一音质只缓存一份）。
    /// 返回 (缓存键, 扩展名)。ext 用于从 URL 提前确定文件扩展名。
    fn cache_key_for(&self, url: &str, info: &TrackInfo) -> (String, String) {
        let ext = url
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .rsplit('.')
            .next()
            .map(|e| e.to_lowercase())
            .filter(|e| {
                matches!(
                    e.as_str(),
                    "mp3" | "flac" | "wav" | "ogg" | "oga" | "m4a" | "aac" | "mp4" | "m4b"
                        | "m4s"
                )
            })
            .unwrap_or_else(|| "bin".into());
        // 实际音质（FLAC 按无损档），落到缓存键里：换音质后不命中旧缓存
        let q = info
            .quality
            .as_deref()
            .map(|q| {
                if q.eq_ignore_ascii_case("flac") {
                    "flac".to_string()
                } else {
                    q.replace([' ', 'k'], "")
                }
            })
            .unwrap_or_else(|| "d".into());
        let key = match (info.kind.as_str(), info.nid, info.qid.as_deref()) {
            ("netease", Some(nid), _) => format!("net-{nid}-{q}"),
            ("qq", _, Some(qid)) => format!("qq-{qid}-{q}"),
            ("kugou", _, Some(kgid)) => format!("kug-{kgid}-{q}"),
            // B 站：qid 承载 "bvid-cid"，同一分P同一音质只缓存一份
            ("bilibili", _, Some(qid)) => format!("bili-{qid}-{q}"),
            // Navidrome：qid 为歌曲 id（流地址里的 salt/token 每次都变，
            // 不能按 URL 哈希，否则永远不命中缓存）
            ("navidrome", _, Some(qid)) => format!("nd-{qid}-{q}"),
            // 自定义在线音源没有稳定 ID，仍按 URL 哈希
            _ => {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                url.hash(&mut h);
                format!("url-{h:016x}-{q}", h = h.finish())
            }
        };
        (key, ext)
    }

    fn cache_path_for(&self, key: &str, ext: &str) -> PathBuf {
        self.downloads_dir.join(format!("{key}.{ext}"))
    }

    /// 播放在线音源：有缓存直接播放，否则后台下载（带进度事件）完成后自动播放。
    /// 下载完成后按缓存上限做 LRU 清理。
    pub fn play_url(self: &Arc<Self>, url: String, info: TrackInfo) -> Result<(), String> {
        let url = url.trim().to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("音源地址必须以 http:// 或 https:// 开头".into());
        }
        if url.contains(".m3u8") {
            return Err("暂不支持 m3u8/HLS 流，请使用音频文件直链".into());
        }
        let (key, ext) = self.cache_key_for(&url, &info);
        let cache = self.cache_path_for(&key, &ext);
        if cache.exists() && cache.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            // 命中缓存：更新访问时间（LRU 依据），当前曲目直接播放
            let _ = filetime::set_file_mtime(
                &cache,
                filetime::FileTime::from_system_time(std::time::SystemTime::now()),
            );
            let mut info = info;
            info.path = cache.to_string_lossy().into_owned();
            if info.duration_ms == 0 {
                info.duration_ms = probe_duration(&cache);
            }
            *self.want_url.write() = None;
            return self.play_file(info);
        }
        *self.want_url.write() = Some(url.clone());
        // 同一缓存键已有下载在进行：只登记意图后返回，复用进行中的下载，
        // 避免两个线程同时写同一个 .part 缓存文件导致内容损坏
        {
            let mut dl = self.downloading.write();
            if dl.contains(&key) {
                return Ok(());
            }
            dl.insert(key.clone());
        }
        // B 站 CDN 直链必须带 Referer/UA（部分边缘节点裸请求 403）；
        // 且 playurl 每次可能返回不同 CDN 主机，不能按主机名判断，按来源标记
        let is_bili = info.kind == "bilibili";
        let engine = Arc::clone(self);
        let app = self.app.clone();

        // 边下边播：下载照常落盘 .part，预缓冲达标（或整段完成）即起播，
        // 播放端读取器越过下载前沿时阻塞等待新数据，无需等整段下完。
        let shared = crate::streaming::StreamingDownload::new(
            cache.with_extension("part"),
            cache.clone(),
        );
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // 监视线程：预缓冲达标后以流式源起播（仅当用户仍在等这首）。
        // 与下载线程的"完成后播放"通过 started 原子交换保证只有一方起播。
        {
            let engine = Arc::clone(&engine);
            let shared = Arc::clone(&shared);
            let started = Arc::clone(&started);
            let url = url.clone();
            let info = info.clone();
            let cache = cache.clone();
            std::thread::spawn(move || loop {
                let (_ready, done, failed) = shared.wait_prebuffer(512 * 1024);
                if failed || done || started.load(Ordering::Relaxed) {
                    // 整段完成 → 由下载线程负责最终播放；失败 → 其已回报错误
                    break;
                }
                let armed = engine.want_url.read().as_deref() == Some(url.as_str());
                if !armed {
                    // 用户暂已切走：继续等下载完成（重新点播可在完成前再入本分支）
                    continue;
                }
                // 预缓冲达标：嗅探容器（此时文件头必然已落盘）
                if let Ok(mut f) = std::fs::File::open(&shared.part) {
                    let mut head = [0u8; 8];
                    use std::io::Read as _;
                    if f.read(&mut head).unwrap_or(0) == 8 && head[4..8] == *b"ftyp" {
                        shared.set_mp4(true);
                    }
                } else {
                    break;
                }
                // 先登记流式关联再置 started：open_current_source 与下载线程的
                // 兜底播放判断都依赖它。起播走 play_file 完整流程——
                // 独占模式开启时同样能协商独占会话（读取越界阻塞由流读取器承担）
                let mut i = info.clone();
                i.path = cache.to_string_lossy().into_owned();
                *engine.active_streaming.write() = Some((i.path.clone(), Arc::clone(&shared)));
                started.store(true, Ordering::SeqCst);
                *engine.want_url.write() = None;
                if let Err(e) = engine.play_file(i) {
                    let _ = engine.app.emit(
                        "download://progress",
                        serde_json::json!({
                            "url": url, "done": true,
                            "error": format!("播放失败：{e}")
                        }),
                    );
                }
                break;
            });
        }

        std::thread::spawn(move || {
            let result = download_to(&app, &url, &cache, is_bili, Some(&shared));
            engine.downloading.write().remove(&key);
            match result {
                Err(e) => {
                    let _ = app.emit(
                        "download://progress",
                        serde_json::json!({ "url": url, "done": true, "error": e }),
                    );
                    // 用户仍在等这首时清除意图，便于下次点击重新发起下载
                    if engine.want_url.read().as_deref() == Some(url.as_str()) {
                        *engine.want_url.write() = None;
                    }
                }
                Ok(()) => {
                    // 流式源仍在播这首 → 不重复起播；否则（小文件整段下完 /
                    // 总长未知 / 流式起播失败 / 跳歌后重播）在此播放完整缓存
                    let part_str = cache.to_string_lossy().into_owned();
                    let streaming_active = started.load(Ordering::SeqCst)
                        && engine
                            .active_streaming
                            .read()
                            .as_ref()
                            .map_or(false, |(p, _)| *p == part_str);
                    if !streaming_active {
                        let still_wanted =
                            engine.want_url.read().as_deref() == Some(url.as_str());
                        if still_wanted {
                            let mut info = info;
                            info.path = cache.to_string_lossy().into_owned();
                            if info.duration_ms == 0 {
                                info.duration_ms = probe_duration(&cache);
                            }
                            // 播放失败必须回报：此前静默吞噬，前端停在"播放中"
                            // 却没有任何声音，用户无从得知原因
                            if let Err(e) = engine.play_file(info) {
                                let _ = app.emit(
                                    "download://progress",
                                    serde_json::json!({
                                        "url": url, "done": true,
                                        "error": format!("播放失败：{e}")
                                    }),
                                );
                            }
                        }
                    }
                    // 下载成功后按上限清理（跳过正在播放/下载中的文件）
                    engine.evict_cache();
                }
            }
        });
        Ok(())
    }

    // ---------- 缓存管理 ----------

    /// 缓存目录内所有完整缓存文件（含大小与最后访问时间），按新旧降序
    fn cache_entries(&self) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.downloads_dir) {
            for e in rd.flatten() {
                let p = e.path();
                if !p.is_file() {
                    continue;
                }
                if p.extension().and_then(|x| x.to_str()) == Some("part") {
                    continue;
                }
                let Ok(meta) = e.metadata() else { continue };
                let mtime = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                out.push((p, meta.len(), mtime));
            }
        }
        out.sort_by(|a, b| b.2.cmp(&a.2)); // 新 -> 旧
        out
    }

    /// 当前缓存占用（字节）与文件数
    pub fn cache_usage(&self) -> (u64, u32) {
        let mut total = 0u64;
        let mut count = 0u32;
        for (_, len, _) in self.cache_entries() {
            total += len;
            count += 1;
        }
        (total, count)
    }

    /// 强制清理全部缓存；正在播放的文件跳过（Windows 上删除会失败）。
    /// 返回删除的文件数。
    pub fn clear_cache(&self) -> u32 {
        let playing = self
            .current
            .read()
            .as_ref()
            .map(|c| c.path.clone())
            .unwrap_or_default();
        let mut n = 0u32;
        for (p, _, _) in self.cache_entries() {
            if !playing.is_empty() && p == PathBuf::from(&playing) {
                continue;
            }
            if std::fs::remove_file(&p).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// 缓存上限（字节），0 = 不限制
    pub fn cache_limit(&self) -> u64 {
        self.cache_limit.load(Ordering::Relaxed)
    }

    pub fn set_cache_limit(&self, bytes: u64) {
        self.cache_limit.store(bytes, Ordering::Relaxed);
        self.evict_cache();
    }

    /// LRU 清理：超过上限时从最旧开始删，直到回到上限内。
    /// 跳过正在播放的文件和 .part 下载中间文件（后者不占上限，由下载流程自管）。
    fn evict_cache(&self) {
        let limit = self.cache_limit();
        if limit == 0 {
            return;
        }
        let playing = self
            .current
            .read()
            .as_ref()
            .map(|c| c.path.clone())
            .unwrap_or_default();
        let entries = self.cache_entries();
        let mut total = 0u64;
        for (_, len, _) in &entries {
            total += len;
        }
        if total <= limit {
            return;
        }
        for (p, len, _) in entries.iter().rev() {
            if total <= limit {
                break;
            }
            if !playing.is_empty() && *p == PathBuf::from(&playing) {
                continue;
            }
            if std::fs::remove_file(p).is_ok() {
                total = total.saturating_sub(*len);
            }
        }
    }
}

fn probe_duration(path: &Path) -> u64 {
    lofty::read_from_path(path)
        .ok()
        .map(|t| t.properties().duration().as_millis() as u64)
        .unwrap_or(0)
}

/// 嗅探文件是否为 MP4 容器（mp4/m4a/m4b、B 站 DASH 缓存均属此族）。
/// MP4 族一律走 symphonia 直连源（symdec）：rodio 的包装层对多轨 MP4
/// 会把视频轨帧数混进音轨时基算出错误总时长（0.22.2），且历史版本对
/// MP4 初始化直接 panic；symdec 选轨/时长/seek 均正确。
/// 不按扩展名预筛（缓存键扩展名取自 URL，可能任意）——直接看文件头，
/// MP4 规范要求首 box 即 ftyp（偏移 4..8）。
fn is_mp4_container(path: &str) -> bool {
    let Ok(mut f) = File::open(path) else {
        return false;
    };
    let mut head = [0u8; 8];
    let n = f.read(&mut head).unwrap_or(0);
    n == 8 && head[4..8] == *b"ftyp"
}

/// start() 尾部的公共播放准备（新 Sink 后设置音量/速度并开播）
fn sink_play_common(sink: &RwLock<Player>, volume_bits: u32, speed_bits: u32) {
    let s = sink.read();
    s.set_volume(f32::from_bits(volume_bits));
    s.set_speed(f32::from_bits(speed_bits));
    s.play();
}

/// 按文件类型打开可播放采样源：MP4 容器（m4a/mp4、B 站 DASH 缓存）走
/// symphonia 直连源（symdec），其余（mp3/flac/wav/ogg 等）走 rodio 解码。
fn open_playable_source(
    path: &str,
    skip_ms: u64,
) -> Result<Box<dyn rodio::Source + Send>, String> {
    if is_mp4_container(path) {
        return Ok(Box::new(crate::symdec::SymphoniaSource::open_at(
            path, skip_ms,
        )?));
    }
    let file = File::open(path).map_err(|e| format!("打开文件失败: {e}"))?;
    // MP4 族解码器初始化依赖文件总长 + 可寻址标记，缺失会直接失败；
    // 常规格式二者同样成立（本地文件皆可寻址），seek/时长计算亦受益
    let mut builder = Decoder::builder().with_seekable(true);
    if let Ok(md) = file.metadata() {
        builder = builder.with_byte_len(md.len());
    }
    let decoder = builder
        .with_data(BufReader::new(file))
        .build()
        .map_err(|e| format!("无法解码该音频文件: {e}"))?;
    Ok(Box::new(
        decoder.skip_duration(Duration::from_millis(skip_ms)),
    ))
}

/// 下载 URL 到本地缓存文件，通过 download://progress 事件回报进度
fn download_to(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    with_bili_headers: bool,
    stream: Option<&Arc<crate::streaming::StreamingDownload>>,
) -> Result<(), String> {
    let r = download_inner(app, url, dest, with_bili_headers, stream);
    if let Some(s) = stream {
        // 保证失败时读取端也能立即终止（成功路径已在 inner 内 finish）
        if r.is_err() {
            s.finish(false, 0);
        }
    }
    r
}

fn download_inner(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    with_bili_headers: bool,
    stream: Option<&Arc<crate::streaming::StreamingDownload>>,
) -> Result<(), String> {
    let part = dest.with_extension("part");
    // 连接与读取分段超时：整体超时会在大文件（FLAC 等几十 MB）下载中途掐断连接
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .build();
    let mut req = agent.get(url);
    // B 站 CDN 直链必须带 Referer 与浏览器 UA，否则部分边缘节点裸请求 403
    if with_bili_headers {
        req = req
            .set("Referer", "https://www.bilibili.com/")
            .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36");
    }
    let resp = req.call().map_err(|e| format!("下载音源失败: {e}"))?;
    let total: u64 = resp
        .header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if let Some(s) = stream {
        s.set_total(total);
    }

    let title_hint = resp
        .header("content-disposition")
        .and_then(|v| {
            v.split(';')
                .rev()
                .find_map(|seg| seg.trim().strip_prefix("filename="))
                .map(|s| s.trim_matches('"').to_string())
        })
        .unwrap_or_default();

    let mut file = File::create(&part).map_err(|e| format!("创建缓存文件失败: {e}"))?;
    let mut reader = resp.into_reader();
    let mut buf = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let io_result = (|| -> Result<(), String> {
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("下载数据流中断: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| format!("写入缓存失败: {e}"))?;
            received += n as u64;
            // 边下边播：推进数据前沿，唤醒阻塞中的播放端读取器
            if let Some(s) = stream {
                s.publish(received);
            }
            if last_emit.elapsed() >= Duration::from_millis(300) {
                // WebView 挂起（托盘隐藏）时跳过进度推送：避免反复唤醒渲染进程。
                // 恢复后前端 webview://resumed 兜底复位下载条
                let suspended = app
                    .try_state::<crate::AppState>()
                    .map(|st| st.webview_suspended.load(Ordering::SeqCst))
                    .unwrap_or(false);
                if !suspended {
                    last_emit = std::time::Instant::now();
                    let pct = if total > 0 {
                        (received as f64 / total as f64 * 100.0) as u64
                    } else {
                        0
                    };
                    let _ = app.emit(
                        "download://progress",
                        serde_json::json!({ "url": url, "received": received, "total": total, "pct": pct, "done": false }),
                    );
                }
            }
        }
        Ok(())
    })();
    if let Some(s) = stream {
        // 先终态后重命名：读取器收到 EOF 后释放文件，重命名成功率更高
        s.finish(io_result.is_ok(), received);
    }
    io_result?;
    drop(file);
    // 播放端可能仍持有 .part 句柄（流式播放中）：Windows 上重命名会失败，
    // 短暂重试，仍失败则留给读取器 Drop 时收尾
    let mut renamed = false;
    for _ in 0..30 {
        match std::fs::rename(&part, dest) {
            Ok(()) => {
                renamed = true;
                break;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    if !renamed {
        eprintln!("[engine] .part 暂无法重命名（播放中仍持有），由读取器收尾");
    }
    let _ = app.emit(
        "download://progress",
        serde_json::json!({ "url": url, "received": received, "total": if total == 0 { received } else { total }, "pct": 100, "done": true }),
    );
    if !title_hint.is_empty() {
        let st = app.state::<crate::AppState>();
        let conn = st.db.lock();
        if let Some(row) = db_lookup_source(&conn, url) {
            if row.1.is_empty() {
                crate::db::update_source_title(&conn, row.0, &title_hint);
            }
        }
    }
    Ok(())
}

fn db_lookup_source(conn: &rusqlite::Connection, url: &str) -> Option<(i64, String)> {
    conn.query_row(
        "SELECT id, title FROM sources WHERE url = ?1",
        rusqlite::params![url],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .ok()
}
