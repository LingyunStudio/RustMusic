use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tauri::{AppHandle, Manager};

use crate::engine::TrackInfo;

pub enum SmtcMsg {
    Update {
        info: Option<TrackInfo>,
        playing: bool,
        pos_ms: u64,
    },
    /// 仅刷新系统浮窗播放进度（播放中由 monitor 周期发送，不重复设置元数据）
    Position { playing: bool, pos_ms: u64 },
    /// 远程封面后台下载完成：用最后一次曲目状态重刷元数据（带封面）
    RefreshCover,
}

/// 启动 SMTC（Windows 系统媒体传输控制）线程，返回消息发送端
pub fn spawn(app: AppHandle) -> Sender<SmtcMsg> {
    let (tx, rx) = channel::<SmtcMsg>();
    let run_tx = tx.clone();
    std::thread::spawn(move || run(app, rx, run_tx));
    tx
}

fn run(app: AppHandle, rx: Receiver<SmtcMsg>, tx: Sender<SmtcMsg>) {
    let hwnd = app
        .get_webview_window("main")
        .and_then(|w| w.hwnd().ok())
        .map(|h| h.0 as *mut std::ffi::c_void);

    let ev_app = app.clone();
    let handler = move |ev: MediaControlEvent| {
        let (action, value) = match ev {
            MediaControlEvent::Play => ("play", None),
            MediaControlEvent::Pause => ("pause", None),
            MediaControlEvent::Toggle => ("toggle", None),
            MediaControlEvent::Next => ("next", None),
            MediaControlEvent::Previous => ("prev", None),
            MediaControlEvent::Stop => ("stop", None),
            MediaControlEvent::Seek(dir) => (
                if matches!(dir, SeekDirection::Forward) {
                    "seek_fwd"
                } else {
                    "seek_back"
                },
                None,
            ),
            MediaControlEvent::SeekBy(dir, _) => (
                if matches!(dir, SeekDirection::Forward) {
                    "seek_fwd"
                } else {
                    "seek_back"
                },
                None,
            ),
            MediaControlEvent::SetPosition(p) => ("set_pos", Some(p.0.as_millis() as f64)),
            MediaControlEvent::Raise => ("show", None),
            _ => return,
        };
        // 统一走 media_control：WebView 挂起（托盘隐藏）时先唤醒再转发
        crate::tray::media_control(&ev_app, action, value);
    };

    let config = PlatformConfig {
        dbus_name: "rustmusic",
        display_name: "RustMusic",
        hwnd,
    };

    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("SMTC 初始化失败: {e:?}");
            return;
        }
    };
    if let Err(e) = controls.attach(handler) {
        eprintln!("SMTC 事件挂载失败: {e:?}");
        return;
    }
    let _ = controls.set_playback(MediaPlayback::Stopped);

    let mut has_track = false;
    // 最后一次曲目状态：RefreshCover（封面异步下载完成）重刷元数据时用
    let mut last_update: Option<(TrackInfo, bool, u64)> = None;
    for msg in rx {
        match msg {
            SmtcMsg::Update {
                info,
                playing,
                pos_ms,
            } => {
                has_track = info.is_some();
                match info {
                    Some(i) => {
                        last_update = Some((i.clone(), playing, pos_ms));
                        set_track(
                            &mut controls,
                            &i,
                            playing,
                            pos_ms,
                            cover_uri(&i.cover, &app, tx.clone()).as_deref(),
                        );
                    }
                    None => {
                        last_update = None;
                        let _ = controls.set_playback(MediaPlayback::Stopped);
                    }
                }
            }
            SmtcMsg::RefreshCover => {
                // 封面此刻应已入缓存：重发最后一次元数据，系统浮窗补上封面
                if let Some((i, playing, pos_ms)) = &last_update {
                    set_track(
                        &mut controls,
                        i,
                        *playing,
                        *pos_ms,
                        cover_uri(&i.cover, &app, tx.clone()).as_deref(),
                    );
                }
            }
            SmtcMsg::Position { playing, pos_ms } => {
                // 系统媒体浮窗（SMTC）不会自行走表，播放中需周期刷新进度
                if has_track {
                    let pos = MediaPosition(Duration::from_millis(pos_ms));
                    let _ = controls.set_playback(if playing {
                        MediaPlayback::Playing {
                            progress: Some(pos),
                        }
                    } else {
                        MediaPlayback::Paused {
                            progress: Some(pos),
                        }
                    });
                }
            }
        }
    }
}

/// 设置一次完整的系统浮窗元数据 + 播放状态
fn set_track(
    controls: &mut MediaControls,
    i: &TrackInfo,
    playing: bool,
    pos_ms: u64,
    cover: Option<&str>,
) {
    let _ = controls.set_metadata(MediaMetadata {
        title: Some(&i.title),
        artist: Some(&i.artist),
        album: Some(&i.album),
        duration: Some(Duration::from_millis(i.duration_ms)),
        cover_url: cover,
    });
    let pos = MediaPosition(Duration::from_millis(pos_ms));
    let _ = controls.set_playback(if playing {
        MediaPlayback::Playing {
            progress: Some(pos),
        }
    } else {
        MediaPlayback::Paused {
            progress: Some(pos),
        }
    });
}

/// 已解析的远程封面缓存（url → 本地文件；None = 下载失败，避免反复重试）
fn cover_cache() -> &'static Mutex<HashMap<String, Option<std::path::PathBuf>>> {
    static C: OnceLock<Mutex<HashMap<String, Option<std::path::PathBuf>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 下载中的封面 URL 集合（去重，防止同一封面起多个下载线程）
fn cover_inflight() -> &'static Mutex<HashSet<String>> {
    static S: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 下载远程封面到本地缓存目录，返回落盘路径
fn download_cover(p: &str, app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = app
        .try_state::<crate::AppState>()
        .map(|s| s.app_data.join("covers").join("smtc"))?;
    let _ = std::fs::create_dir_all(&dir);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    p.hash(&mut h);
    let ext = if p.to_lowercase().contains(".png") {
        "png"
    } else {
        "jpg"
    };
    let dest = dir.join(format!("{:016x}.{ext}", h.finish()));
    let missing = !dest.exists() || dest.metadata().map(|m| m.len() == 0).unwrap_or(true);
    if missing {
        // 连接与读取分段超时，避免整体超时掐断大封面
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(10))
            .build();
        let mut data = Vec::new();
        agent
            .get(p)
            .set("User-Agent", "Mozilla/5.0")
            .call()
            .ok()?
            .into_reader()
            .take(2 * 1024 * 1024)
            .read_to_end(&mut data)
            .ok()?;
        if data.is_empty() {
            return None;
        }
        std::fs::write(&dest, data).ok()?;
    }
    Some(dest)
}

fn cover_uri(p: &str, app: &AppHandle, refresh: Sender<SmtcMsg>) -> Option<String> {
    if p.is_empty() {
        return None;
    }
    if !(p.starts_with("http://") || p.starts_with("https://")) {
        if !std::path::Path::new(p).exists() {
            return None;
        }
        return Some(file_uri(p));
    }
    // 远程封面：SMTC 线程内绝不联网（首次在线曲目会阻塞本线程最长 ~15s，
    // 期间进度刷新与媒体键事件全部卡死）。缓存命中直接用；未命中交给
    // 后台线程下载，完成后发 RefreshCover 重刷元数据补上封面
    if let Some(hit) = cover_cache().lock().unwrap().get(p) {
        return hit
            .as_ref()
            .and_then(|d| d.to_str())
            .map(file_uri);
    }
    if cover_inflight().lock().unwrap().insert(p.to_string()) {
        let key = p.to_string();
        let bg_app = app.clone();
        std::thread::spawn(move || {
            let dest = download_cover(&key, &bg_app);
            cover_cache().lock().unwrap().insert(key.clone(), dest);
            cover_inflight().lock().unwrap().remove(&key);
            let _ = refresh.send(SmtcMsg::RefreshCover);
        });
    }
    None
}

/// 本地路径 → file:// URI（% 必须编码：路径含 "100% Rock" 之类时
/// 不编码会让 SMTC 的 URI 解析直接失败）
fn file_uri(p: &str) -> String {
    let norm = p.replace('\\', "/");
    format!("file:///{}", utf8_percent_encode(&norm, FRAGMENT))
}

const FRAGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'#')
    .add(b'%')
    .add(b'?')
    .add(b'{')
    .add(b'}');
