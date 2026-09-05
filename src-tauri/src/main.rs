#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod engine;
mod eq;
mod library;
mod lyrics;
mod models;
mod netease;
mod qq;
mod smtc;

use std::sync::atomic::Ordering;
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub engine: Mutex<Arc<engine::Engine>>,
    pub app_data: std::path::PathBuf,
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let pp = MenuItem::with_id(app, "pp", "播放 / 暂停", true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "prev", "上一首", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "下一首", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &sep1, &pp, &prev, &next, &sep2, &quit])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().expect("missing app icon").clone())
        .tooltip("RustMusic")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "show" => show_main(app),
            "pp" => {
                let _ = app.emit("media://control", serde_json::json!({ "action": "toggle" }));
            }
            "prev" => {
                let _ = app.emit("media://control", serde_json::json!({ "action": "prev" }));
            }
            "next" => {
                let _ = app.emit("media://control", serde_json::json!({ "action": "next" }));
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = ev
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn monitor(app: AppHandle) {
    let eng = {
        let st = app.state::<AppState>();
        let e = st.engine.lock().clone();
        e
    };
    let mut was_active = false;
    let mut last_pos_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);

    let mut diag = 0u32;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(120));
        let active = eng.is_active();
        let paused = eng.user_paused.load(Ordering::Relaxed);

        diag += 1;
        if cfg!(debug_assertions) && diag % 40 == 0 {
            eprintln!(
                "[monitor] active={} paused={} stopped={} pos={} dur={}",
                active,
                paused,
                eng.stopped.load(Ordering::Relaxed),
                eng.pos_ms.load(Ordering::Relaxed),
                eng.dur_ms.load(Ordering::Relaxed),
            );
        }

        if active && !paused && last_pos_emit.elapsed() >= std::time::Duration::from_millis(250) {
            last_pos_emit = std::time::Instant::now();
            let _ = app.emit(
                "player://pos",
                serde_json::json!({
                    "pos": eng.pos_ms.load(Ordering::Relaxed),
                    "dur": eng.dur_ms.load(Ordering::Relaxed),
                }),
            );
        }

        // 一首曲目自然播完（非暂停、非手动停止）
        if was_active && !active && !paused && !eng.stopped.load(Ordering::Relaxed) {
            let _ = app.emit("player://ended", serde_json::json!({}));
        }
        was_active = active;
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let app_data = handle
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?;
            std::fs::create_dir_all(app_data.join("covers")).map_err(|e| e.to_string())?;
            std::fs::create_dir_all(app_data.join("downloads")).map_err(|e| e.to_string())?;

            let conn = db::init(&app_data.join("library.db"))?;

            // 读取用户设置
            let volume: f32 = db::get_setting(&conn, "volume")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.8);
            let speed: f32 = db::get_setting(&conn, "speed")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0);
            let eq_gains: [f32; 10] = db::get_setting(&conn, "eq_gains")
                .and_then(|s| serde_json::from_str::<Vec<f32>>(&s).ok())
                .and_then(|v| {
                    let mut a = [0f32; 10];
                    if v.len() == 10 {
                        a.copy_from_slice(&v);
                        Some(a)
                    } else {
                        None
                    }
                })
                .unwrap_or([0.0; 10]);
            let eq_enabled = db::get_setting(&conn, "eq_enabled")
                .map(|s| s == "true")
                .unwrap_or(false);
            let eq = Arc::new(eq::EqShared::new(eq_gains, eq_enabled));

            let smtc_tx = smtc::spawn(handle.clone());
            let eng = engine::Engine::new(
                handle.clone(),
                &app_data,
                volume,
                speed,
                eq,
                smtc_tx,
            )?;

            app.manage(AppState {
                db: Mutex::new(conn),
                engine: Mutex::new(Arc::new(eng)),
                app_data: app_data.clone(),
            });

            let mhandle = handle.clone();
            std::thread::spawn(move || monitor(mhandle));

            setup_tray(&handle)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_tracks,
            commands::list_folders,
            commands::add_folder,
            commands::remove_folder,
            commands::rescan,
            commands::drop_paths,
            commands::get_lyrics,
            commands::like_track,
            commands::list_playlists,
            commands::create_playlist,
            commands::delete_playlist,
            commands::rename_playlist,
            commands::add_to_playlist,
            commands::remove_from_playlist,
            commands::list_sources,
            commands::add_source,
            commands::delete_source,
            commands::play_track,
            commands::play_source,
            commands::netease_search,
            commands::netease_play,
            commands::netease_status,
            commands::netease_qr_create,
            commands::netease_qr_check,
            commands::netease_lyric,
            commands::netease_like_list,
            commands::netease_like,
            commands::netease_logout,
            commands::qq_search,
            commands::qq_play,
            commands::qq_lyric,
            commands::qq_qr_create,
            commands::qq_qr_check,
            commands::qq_status,
            commands::qq_logout,
            commands::play_pause,
            commands::pause,
            commands::resume,
            commands::stop,
            commands::seek,
            commands::set_volume,
            commands::set_speed,
            commands::set_eq,
            commands::get_settings,
            commands::clear_cache,
            commands::get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
