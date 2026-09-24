use std::hash::{Hash, Hasher};
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
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
}

/// 启动 SMTC（Windows 系统媒体传输控制）线程，返回消息发送端
pub fn spawn(app: AppHandle) -> Sender<SmtcMsg> {
    let (tx, rx) = channel::<SmtcMsg>();
    std::thread::spawn(move || run(app, rx));
    tx
}

fn run(app: AppHandle, rx: Receiver<SmtcMsg>) {
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
        crate::media_control(&ev_app, action, value);
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
                        let _ = controls.set_metadata(MediaMetadata {
                            title: Some(&i.title),
                            artist: Some(&i.artist),
                            album: Some(&i.album),
                            duration: Some(Duration::from_millis(i.duration_ms)),
                            cover_url: cover_uri(&i.cover, &app).as_deref(),
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
                    None => {
                        let _ = controls.set_playback(MediaPlayback::Stopped);
                    }
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

fn cover_uri(p: &str, app: &AppHandle) -> Option<String> {
    if p.is_empty() {
        return None;
    }
    let path: std::path::PathBuf = if p.starts_with("http://") || p.starts_with("https://") {
        // 远程封面（网易云 / QQ 为 http 直链）：缓存到本地后以 file:// 提供给 SMTC
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
        dest
    } else {
        if !std::path::Path::new(p).exists() {
            return None;
        }
        std::path::PathBuf::from(p)
    };
    let norm = path.to_string_lossy().replace('\\', "/");
    Some(format!("file:///{}", utf8_percent_encode(&norm, FRAGMENT)))
}

const FRAGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'#')
    .add(b'?')
    .add(b'{')
    .add(b'}');
