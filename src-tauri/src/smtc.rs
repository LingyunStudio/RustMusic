use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use tauri::{AppHandle, Emitter, Manager};

use crate::engine::TrackInfo;

pub enum SmtcMsg {
    Update {
        info: Option<TrackInfo>,
        playing: bool,
        pos_ms: u64,
    },
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
        let _ = ev_app.emit(
            "media://control",
            serde_json::json!({ "action": action, "value": value }),
        );
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

    for msg in rx {
        let SmtcMsg::Update { info, playing, pos_ms } = msg;
        match info {
            Some(i) => {
                let _ = controls.set_metadata(MediaMetadata {
                    title: Some(&i.title),
                    artist: Some(&i.artist),
                    album: Some(&i.album),
                    duration: Some(Duration::from_millis(i.duration_ms)),
                    cover_url: cover_uri(&i.cover).as_deref(),
                });
                let pos = MediaPosition(Duration::from_millis(pos_ms));
                let _ = controls.set_playback(if playing {
                    MediaPlayback::Playing { progress: Some(pos) }
                } else {
                    MediaPlayback::Paused { progress: Some(pos) }
                });
            }
            None => {
                let _ = controls.set_playback(MediaPlayback::Stopped);
            }
        }
    }
}

fn cover_uri(p: &str) -> Option<String> {
    if p.is_empty() || !std::path::Path::new(p).exists() {
        return None;
    }
    let norm = p.replace('\\', "/");
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
