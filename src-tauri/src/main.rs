#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod db;
mod engine;
mod eq;
mod kugou;
mod library;
mod lyrics;
mod models;
mod netease;
mod qq;
mod qrc;
mod smtc;
mod updater;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub engine: Mutex<Arc<engine::Engine>>,
    pub app_data: std::path::PathBuf,
    /// 主窗口 WebView 是否处于挂起状态（托盘隐藏时 TrySuspend 回收渲染内存）
    pub webview_suspended: AtomicBool,
    /// 最近一次扫描进度快照（挂起期间 scan://progress 事件会丢，恢复后补发）
    pub scan_last: Mutex<serde_json::Value>,
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    resume_main_webview(app, true);
}

/// 挂起主窗口 WebView（WebView2 TrySuspend）：托盘隐藏时冻结并释放渲染进程内存。
/// 音频由后端 rodio 播放，不依赖 WebView，挂起不影响播放。
/// 桌面歌词开着时不挂起：悬浮窗的歌词/暂停状态依赖主 WebView 的推送。
fn suspend_main_webview(app: &AppHandle) {
    if app.get_webview_window("desktop-lyrics").is_some() {
        return;
    }
    let st = app.state::<AppState>();
    if st.webview_suspended.swap(true, Ordering::SeqCst) {
        return; // 已挂起
    }
    drop(st);
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    if let Err(e) = win.with_webview(|webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
        use windows::core::Interface as _;
        let controller = webview.controller();
        unsafe {
            let Ok(core) = controller.CoreWebView2() else {
                eprintln!("[webview] CoreWebView2() 失败");
                return;
            };
            let Ok(cwv3) = core.cast::<ICoreWebView2_3>() else {
                eprintln!("[webview] cast ICoreWebView2_3 失败");
                return;
            };
            // IsVisible=false 是 TrySuspend 的前置条件
            let _ = controller.SetIsVisible(false);
            let handler = webview2_com::TrySuspendCompletedHandler::create(Box::new(
                |hr, ok| {
                    eprintln!("[webview] TrySuspend 完成 hr={hr:?} ok={ok:?}");
                    Ok(())
                },
            ));
            if let Err(e) = cwv3.TrySuspend(&handler) {
                eprintln!("[webview] 挂起失败: {e}");
            }
        }
    }) {
        eprintln!("[webview] with_webview 失败: {e}");
    }
}

/// 恢复主窗口 WebView。notify=true 时补发播放状态/进度并通知前端
/// 刷新数据（挂起期间发往前端的事件都会被丢弃）。
fn resume_main_webview(app: &AppHandle, notify: bool) {
    {
        let st = app.state::<AppState>();
        if !st.webview_suspended.swap(false, Ordering::SeqCst) && !notify {
            return;
        }
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.with_webview(|webview| {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
            use windows::core::Interface as _;
            let controller = webview.controller();
            unsafe {
                let Ok(core) = controller.CoreWebView2() else {
                    return;
                };
                let Ok(cwv3) = core.cast::<ICoreWebView2_3>() else {
                    return;
                };
                let _ = cwv3.Resume();
                let _ = controller.SetIsVisible(true);
            }
        });
    }
    if notify {
        let app2 = app.clone();
        std::thread::spawn(move || {
            // 等 WebView 完全恢复后再补发，事件才不会被丢
            std::thread::sleep(Duration::from_millis(250));
            {
                let st = app2.state::<AppState>();
                st.engine.lock().resync_ui();
            }
            let _ = app2.emit("webview://resumed", serde_json::json!({}));
        });
    }
}

/// 统一的媒体控制转发（托盘/SMTC → 前端）。
/// WebView 挂起中：先唤醒再延迟转发，控制恢复可用；窗口仍隐藏时稍后重新挂起。
fn media_control(app: &AppHandle, action: &str, value: Option<f64>) {
    if action == "show" {
        show_main(app);
        return;
    }
    let suspended = app
        .state::<AppState>()
        .webview_suspended
        .load(Ordering::SeqCst);
    if !suspended {
        let _ = app.emit(
            "media://control",
            serde_json::json!({ "action": action, "value": value }),
        );
        return;
    }
    resume_main_webview(app, true);
    let app2 = app.clone();
    let action = action.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        let _ = app2.emit(
            "media://control",
            serde_json::json!({ "action": action, "value": value }),
        );
        schedule_resuspend(&app2);
    });
}

/// 窗口仍隐藏时延迟重新挂起（短暂唤醒处理完托盘操作/换曲后回收内存）
fn schedule_resuspend(app: &AppHandle) {
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(8));
        let hidden = app2
            .get_webview_window("main")
            .map(|w| !w.is_visible().unwrap_or(false))
            .unwrap_or(false);
        if hidden {
            suspend_main_webview(&app2);
        }
    });
}

/// 发送需要前端处理的事件。挂起中可丢的事件（如进度帧）直接跳过；
/// 必须处理的事件（如“播完自动切歌”）先唤醒 WebView 再延迟投递。
fn emit_to_frontend(app: &AppHandle, event: &str, payload: serde_json::Value, wake: bool) {
    let suspended = app
        .state::<AppState>()
        .webview_suspended
        .load(Ordering::SeqCst);
    if !suspended {
        let _ = app.emit(event, payload);
        return;
    }
    if !wake {
        return;
    }
    resume_main_webview(app, true);
    let app2 = app.clone();
    let event = event.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        let _ = app2.emit(&event, payload);
        schedule_resuspend(&app2);
    });
}

/// 监听系统默认输出设备变化（耳机插入/拔出、切换默认设备）：
/// 用户未固定设备时自动重建输出流跟到新默认设备，并通知前端刷新设置页。
/// cpal/Windows 无设备变更回调，用轮询实现（2s 间隔，仅查名字开销可忽略）。
fn device_watcher(app: AppHandle) {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    let eng = {
        let st = app.state::<AppState>();
        let e = st.engine.lock().clone();
        e
    };
    let mut last_default: String = rodio::cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let now_default: String = rodio::cpal::default_host()
            .default_output_device()
            .and_then(|d| d.name().ok())
            .unwrap_or_default();
        if now_default == last_default || now_default.is_empty() {
            continue;
        }
        last_default = now_default.clone();
        // 用户固定了设备（且该设备仍存在）时不打扰；跟随系统则自动切换
        if let Some(pref) = eng.device_preference() {
            let still_there = rodio::cpal::default_host()
                .output_devices()
                .map(|mut ds| {
                    ds.any(|d| d.name().ok().as_deref() == Some(pref.as_str()))
                })
                .unwrap_or(false);
            if still_there {
                continue;
            }
            eprintln!("[engine] 固定设备「{pref}」已不存在，跟随系统默认");
        }
        eprintln!("[engine] 默认输出设备变更 → 切到 {now_default}");
        if eng.switch_output_device(None).is_ok() {
            let _ = app.emit(
                "device://changed",
                serde_json::json!({ "current": eng.current_device_name() }),
            );
        }
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
            "pp" => media_control(app, "toggle", None),
            "prev" => media_control(app, "prev", None),
            "next" => media_control(app, "next", None),
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
    let mut last_smtc = std::time::Instant::now() - std::time::Duration::from_secs(1);

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

        if active && !paused {
            if last_pos_emit.elapsed() >= std::time::Duration::from_millis(250) {
                last_pos_emit = std::time::Instant::now();
                let payload = serde_json::json!({
                    "pos": eng.pos_ms.load(Ordering::Relaxed),
                    "dur": eng.dur_ms.load(Ordering::Relaxed),
                });
                // WebView 挂起时进度帧可丢（恢复后由 resync 补发），不值得唤醒
                emit_to_frontend(&app, "player://pos", payload, false);
            }
            // 同步系统媒体浮窗（SMTC）进度，每秒刷新一次
            if last_smtc.elapsed() >= std::time::Duration::from_secs(1) {
                last_smtc = std::time::Instant::now();
                eng.notify_smtc_pos();
            }
        }

        // 一首曲目自然播完（非暂停、非手动停止；FLAC 重建 / 换曲瞬间 sink 短暂为空，跳过）
        let rebuilding = eng.rebuilding.load(Ordering::Relaxed);
        let switching = eng.switching.load(Ordering::Relaxed);
        if was_active
            && !active
            && !paused
            && !eng.stopped.load(Ordering::Relaxed)
            && !rebuilding
            && !switching
        {
            // 队列/循环逻辑在前端：挂起中也要唤醒投递，否则托盘播放不连续
            emit_to_frontend(&app, "player://ended", serde_json::json!({}), true);
        }
        was_active = active || rebuilding || switching;
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            // 主窗口点关闭：按用户设置决定隐藏到托盘（默认）或退出应用。
            // 只拦主窗口——桌面歌词窗口的关闭是正常功能（先存几何再关），
            // 退出路径（托盘退出）走的 app.exit，不触发 CloseRequested。
            if window.label() == "main" {
                // 安全网：窗口获得焦点时若仍挂起（外部 ShowWindow 等绕过托盘
                // 的显示路径），恢复 WebView 并补发状态，避免白屏/冻结界面
                if let tauri::WindowEvent::Focused(true) = event {
                    let app = window.app_handle();
                    let suspended = app
                        .state::<AppState>()
                        .webview_suspended
                        .load(Ordering::SeqCst);
                    if suspended {
                        resume_main_webview(app, true);
                    }
                }
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let app = window.app_handle();
                    let action = {
                        let st = app.state::<AppState>();
                        let conn = st.db.lock();
                        db::get_setting(&conn, "close_action").unwrap_or_else(|| "tray".into())
                    };
                    if action == "tray" {
                        api.prevent_close();
                        let _ = window.hide();
                        // 隐藏到托盘：挂起 WebView 回收渲染内存（音频不受影响）
                        suspend_main_webview(window.app_handle());
                    } else {
                        // exit：显式退出——托盘图标存在时默认关闭可能仅移除窗口，
                        // 进程会以无窗口状态残留在托盘
                        api.prevent_close();
                        app.exit(0);
                    }
                }
            }
        })
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
            let cache_limit: u64 = db::get_setting(&conn, "cache_limit")
                .and_then(|v| v.parse().ok())
                // 默认 2GB：无损音质单曲可达几十 MB，无上限会无限膨胀
                .unwrap_or(2 * 1024 * 1024 * 1024);
            let eng = engine::Engine::new(
                handle.clone(),
                &app_data,
                volume,
                speed,
                eq,
                cache_limit,
                smtc_tx,
            )?;
            let eng = Arc::new(eng);

            // 恢复保存的输出设备偏好（空 = 跟随系统默认）
            {
                let pref = db::get_setting(&conn, "output_device")
                    .filter(|s| !s.is_empty());
                if let Some(name) = pref {
                    if let Err(e) = eng.switch_output_device(Some(&name)) {
                        // 设备已不存在：回落默认并在日志说明
                        eprintln!("[engine] 恢复输出设备「{name}」失败: {e}");
                    }
                }
            }

            app.manage(AppState {
                db: Mutex::new(conn),
                engine: Mutex::new(eng),
                app_data: app_data.clone(),
                webview_suspended: AtomicBool::new(false),
                scan_last: Mutex::new(serde_json::json!({ "active": false, "done": 0, "total": 0 })),
            });

            let mhandle = handle.clone();
            std::thread::spawn(move || monitor(mhandle));

            let dwhandle = handle.clone();
            std::thread::spawn(move || device_watcher(dwhandle));

            setup_tray(&handle)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_tracks,
            commands::list_folders,
            commands::add_folder,
            commands::remove_folder,
            commands::rescan,
            commands::open_folder,
            commands::list_output_devices,
            commands::set_output_device,
            commands::drop_paths,
            commands::get_lyrics,
            commands::like_track,
            commands::list_playlists,
            commands::create_playlist,
            commands::delete_playlist,
            commands::rename_playlist,
            commands::reorder_playlists,
            commands::kugou_search,
            commands::kugou_play,
            commands::kugou_lyric,
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
            commands::like_online,
            commands::download_online,
            commands::liked_online_list,
            commands::recent_online_list,
            commands::save_dir_get,
            commands::save_dir_set,
            commands::add_online_to_playlist,
            commands::remove_playlist_entry,
            commands::save_manual_order,
            commands::get_manual_order,
            commands::reorder_playlist,
            commands::netease_user_playlists,
            commands::netease_import_playlist,
            commands::qq_user_playlists,
            commands::qq_import_playlist,
            commands::set_play_quality,
            commands::set_close_action,
            commands::extract_cover_palette,
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
            commands::cache_stats,
            commands::set_cache_limit,
            commands::get_app_info,
            commands::desktop_lyrics_open,
            commands::desktop_lyrics_close,
            commands::desktop_lyrics_unlock,
            commands::get_scan_state,
            commands::auto_check_update,
            commands::check_update,
            commands::download_update,
            commands::cancel_update_download,
            commands::install_update,
            commands::set_auto_update,
            commands::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
