//! 音频引擎：播放核心（共享模式播放链、seek 重建、音量/倍速、状态事件与 SMTC 同步）。
//! 输出设备管理见 output.rs，WASAPI 独占会话见 exclusive.rs，在线缓存/边下边播见 cache.rs。
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use rodio::{Decoder, MixerDeviceSink, Player, Source};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::eq::{EqShared, EqSource};
use crate::smtc::SmtcMsg;
use crate::wasapi_out;

mod cache;
mod exclusive;
mod output;

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
