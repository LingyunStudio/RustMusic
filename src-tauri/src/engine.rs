use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use lofty::prelude::*;
use rodio::{Decoder, OutputStream, Sink, Source};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::eq::{EqShared, EqSource};
use crate::smtc::SmtcMsg;

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
}

pub struct Engine {
    sink: Sink,
    app: AppHandle,
    pub eq: Arc<EqShared>,
    pub pos_ms: Arc<AtomicU64>,
    pub dur_ms: Arc<AtomicU64>,
    pub user_paused: Arc<AtomicBool>,
    pub stopped: Arc<AtomicBool>,
    /// FLAC seek 重建播放链期间置位，避免 monitor 误判“播完”
    pub rebuilding: Arc<AtomicBool>,
    pub current: Arc<RwLock<Option<TrackInfo>>>,
    volume: AtomicU32,
    speed: AtomicU32,
    want_url: Arc<RwLock<Option<String>>>,
    downloads_dir: PathBuf,
    smtc: Sender<SmtcMsg>,
}

impl Engine {
    pub fn new(
        app: AppHandle,
        app_data: &Path,
        volume: f32,
        speed: f32,
        eq: Arc<EqShared>,
        smtc: Sender<SmtcMsg>,
    ) -> Result<Self, String> {
        let (_stream, handle) =
            OutputStream::try_default().map_err(|e| format!("无法初始化音频输出设备: {e}"))?;
        // OutputStream（cpal Stream）不是 Send/Sync，但设备需要存活整个进程周期：
        // 泄漏保活，Engine 只持有可跨线程的 Sink。
        let _leaked: &'static OutputStream = Box::leak(Box::new(_stream));
        let sink = Sink::try_new(&handle).map_err(|e| format!("无法创建播放通道: {e}"))?;
        sink.pause();
        let downloads_dir = app_data.join("downloads");
        let _ = std::fs::create_dir_all(&downloads_dir);
        Ok(Self {
            sink,
            app,
            eq,
            pos_ms: Arc::new(AtomicU64::new(0)),
            dur_ms: Arc::new(AtomicU64::new(0)),
            user_paused: Arc::new(AtomicBool::new(false)),
            stopped: Arc::new(AtomicBool::new(true)),
            rebuilding: Arc::new(AtomicBool::new(false)),
            current: Arc::new(RwLock::new(None)),
            volume: AtomicU32::new(volume.to_bits()),
            speed: AtomicU32::new(speed.to_bits()),
            want_url: Arc::new(RwLock::new(None)),
            downloads_dir,
            smtc,
        })
    }

    // ---------- 播放 ----------

    pub fn play_file(&self, info: TrackInfo) -> Result<(), String> {
        *self.want_url.write() = None;
        let file = File::open(&info.path).map_err(|e| format!("打开文件失败: {e}"))?;
        let src = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("无法解码该音频文件: {e}"))?
            .convert_samples::<f32>();
        self.start(src, info)
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
        self.dur_ms
            .store(info.duration_ms, Ordering::Relaxed);
        self.sink.clear();
        self.sink.append(wrapped);
        self.sink
            .set_volume(f32::from_bits(self.volume.load(Ordering::Relaxed)));
        self.sink
            .set_speed(f32::from_bits(self.speed.load(Ordering::Relaxed)));
        self.sink.play();
        self.user_paused.store(false, Ordering::Relaxed);
        self.stopped.store(false, Ordering::Relaxed);
        #[cfg(debug_assertions)]
        eprintln!(
            "[engine] started: kind={} path={} total_duration={:?} sr={} ch={}",
            info.kind, info.path, diag_dur, diag_sr, diag_ch,
        );
        *self.current.write() = Some(info.clone());
        self.notify_smtc();
        let _ = self.app.emit(
            "player://state",
            PlayState { playing: true, info },
        );
        Ok(())
    }

    pub fn pause(&self) {
        if self.sink.empty() {
            return;
        }
        self.sink.pause();
        self.user_paused.store(true, Ordering::Relaxed);
        self.notify_smtc();
        self.emit_state(false);
    }

    pub fn resume(&self) {
        if self.sink.empty() {
            return;
        }
        self.sink.play();
        self.user_paused.store(false, Ordering::Relaxed);
        self.stopped.store(false, Ordering::Relaxed);
        self.notify_smtc();
        self.emit_state(true);
    }

    pub fn toggle(&self) {
        if self.user_paused.load(Ordering::Relaxed) {
            self.resume();
        } else {
            self.pause();
        }
    }

    pub fn stop(&self) {
        self.sink.stop();
        self.stopped.store(true, Ordering::Relaxed);
        self.user_paused.store(false, Ordering::Relaxed);
        self.pos_ms.store(0, Ordering::Relaxed);
        self.notify_smtc();
        self.emit_state(false);
    }

    pub fn seek(&self, ms: u64) -> Result<(), String> {
        if self.sink.empty() {
            return Err("当前没有正在播放的曲目".into());
        }
        // FLAC：解码器不支持 seek，失败的 try_seek 还会重置解码器状态
        // （表现为进度先跳回开头再跳目标），直接走重建路径
        let info_opt = self.current.read().clone();
        let is_flac = info_opt
            .as_ref()
            .map(|i| i.path.to_lowercase().ends_with(".flac"))
            .unwrap_or(false);
        if is_flac {
            let info = info_opt.ok_or("当前没有正在播放的曲目")?;
            self.rebuilding.store(true, Ordering::Relaxed);
            let r = self.rebuild_at(&info, ms);
            self.rebuilding.store(false, Ordering::Relaxed);
            return r;
        }
        drop(info_opt);
        // 常规 seek；MP3 边界位置偶发失败时回退 300ms 重试
        match self.sink.try_seek(Duration::from_millis(ms)) {
            Ok(()) => Ok(()),
            Err(first) => {
                let back = ms.saturating_sub(300);
                match self.sink.try_seek(Duration::from_millis(back)) {
                    Ok(()) => Ok(()),
                    Err(_) => Err(format!("定位失败: {first}")),
                }
            }
        }
    }

    /// FLAC 专用：重开文件并丢弃到目标时长，重建播放链
    fn rebuild_at(&self, info: &TrackInfo, ms: u64) -> Result<(), String> {
        let file = std::fs::File::open(&info.path).map_err(|e| format!("重开文件失败: {e}"))?;
        let src = Decoder::new(BufReader::new(file))
            .map_err(|e| format!("重新解码失败: {e}"))?
            .convert_samples::<f32>()
            .skip_duration(Duration::from_millis(ms));
        let wrapped =
            EqSource::with_base(src, self.eq.clone(), self.pos_ms.clone(), ms as f64);
        self.pos_ms.store(ms, Ordering::Relaxed);
        self.dur_ms.store(info.duration_ms, Ordering::Relaxed);
        self.sink.clear();
        self.sink.append(wrapped);
        self.sink
            .set_volume(f32::from_bits(self.volume.load(Ordering::Relaxed)));
        self.sink
            .set_speed(f32::from_bits(self.speed.load(Ordering::Relaxed)));
        let was_paused = self.user_paused.load(Ordering::Relaxed);
        if was_paused {
            self.sink.pause();
        } else {
            self.sink.play();
        }
        Ok(())
    }

    // ---------- 音量 / 速度 / 均衡器 ----------

    pub fn set_volume(&self, v: f32) {
        let v = v.clamp(0.0, 1.0);
        self.volume.store(v.to_bits(), Ordering::Relaxed);
        self.sink.set_volume(v);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    pub fn set_speed(&self, v: f32) {
        let v = v.clamp(0.5, 2.0);
        self.speed.store(v.to_bits(), Ordering::Relaxed);
        self.sink.set_speed(v);
    }

    pub fn speed(&self) -> f32 {
        f32::from_bits(self.speed.load(Ordering::Relaxed))
    }

    // ---------- 状态查询 ----------

    pub fn is_active(&self) -> bool {
        !self.sink.empty()
    }

    fn emit_state(&self, playing: bool) {
        if let Some(info) = self.current.read().clone() {
            let _ = self
                .app
                .emit("player://state", PlayState { playing, info });
        }
    }

    fn notify_smtc(&self) {
        let info = self.current.read().clone();
        let playing = !self.user_paused.load(Ordering::Relaxed) && !self.sink.empty();
        let _ = self.smtc.send(SmtcMsg::Update {
            info,
            playing,
            pos_ms: self.pos_ms.load(Ordering::Relaxed),
        });
    }

    // ---------- 在线音源 ----------

    fn cache_path_for(&self, url: &str) -> PathBuf {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut h);
        let name = h.finish();
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
                )
            })
            .unwrap_or_else(|| "bin".into());
        self.downloads_dir.join(format!("{name:016x}.{ext}"))
    }

    /// 播放在线音源：有缓存直接播放，否则后台下载（带进度事件）完成后自动播放
    pub fn play_url(self: &Arc<Self>, url: String, info: TrackInfo) -> Result<(), String> {
        let url = url.trim().to_string();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("音源地址必须以 http:// 或 https:// 开头".into());
        }
        if url.contains(".m3u8") {
            return Err("暂不支持 m3u8/HLS 流，请使用音频文件直链".into());
        }
        let cache = self.cache_path_for(&url);
        if cache.exists() && cache.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            let mut info = info;
            info.path = cache.to_string_lossy().into_owned();
            if info.duration_ms == 0 {
                info.duration_ms = probe_duration(&cache);
            }
            *self.want_url.write() = None;
            return self.play_file(info);
        }
        *self.want_url.write() = Some(url.clone());
        let engine = Arc::clone(self);
        let app = self.app.clone();
        std::thread::spawn(move || {
            if let Err(e) = download_to(&app, &url, &cache) {
                let _ = app.emit(
                    "download://progress",
                    serde_json::json!({ "url": url, "done": true, "error": e }),
                );
                return;
            }
            let still_wanted = engine.want_url.read().as_deref() == Some(url.as_str());
            if still_wanted {
                let mut info = info;
                info.path = cache.to_string_lossy().into_owned();
                if info.duration_ms == 0 {
                    info.duration_ms = probe_duration(&cache);
                }
                let _ = engine.play_file(info);
            }
        });
        Ok(())
    }
}

fn probe_duration(path: &Path) -> u64 {
    lofty::read_from_path(path)
        .ok()
        .map(|t| t.properties().duration().as_millis() as u64)
        .unwrap_or(0)
}

/// 下载 URL 到本地缓存文件，通过 download://progress 事件回报进度
fn download_to(app: &AppHandle, url: &str, dest: &Path) -> Result<(), String> {
    let part = dest.with_extension("part");
    let resp = ureq::get(url)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|e| format!("下载音源失败: {e}"))?;
    let total: u64 = resp
        .header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

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
        if last_emit.elapsed() >= Duration::from_millis(300) {
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
    drop(file);
    std::fs::rename(&part, dest).map_err(|e| format!("缓存文件重命名失败: {e}"))?;
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

fn db_lookup_source(
    conn: &rusqlite::Connection,
    url: &str,
) -> Option<(i64, String)> {
    conn.query_row(
        "SELECT id, title FROM sources WHERE url = ?1",
        rusqlite::params![url],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .ok()
}
