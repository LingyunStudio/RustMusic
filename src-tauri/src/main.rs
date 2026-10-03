#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bilibili;
mod commands;
mod db;
mod engine;
mod eq;
mod kugou;
mod monitor;
mod navidrome;
mod library;
mod lyrics;
mod models;
mod netease;
mod qq;
mod qrc;
mod smtc;
mod streaming;
mod symdec;
mod tray;
mod updater;
mod wasapi_out;
mod webview_suspend;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::Manager;

use monitor::{device_watcher, monitor};
use tray::setup_tray;
use webview_suspend::{resume_main_webview, suspend_main_webview};

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    /// 音频引擎句柄。Engine 内部以细粒度 RwLock / 原子量管理状态（sink、进度等），
    /// 命令层克隆 Arc 后在锁外调用——不存在全局串行点
    pub engine: Arc<engine::Engine>,
    pub app_data: std::path::PathBuf,
    /// 主窗口 WebView 是否处于挂起状态（托盘隐藏时 TrySuspend 回收渲染内存）
    pub webview_suspended: AtomicBool,
    /// 最近一次扫描进度快照（挂起期间 scan://progress 事件会丢，恢复后补发）
    pub scan_last: Mutex<serde_json::Value>,
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
            // 无框窗口关掉 DWM 边框后 Win11 会失去圆角，这里单独设回圆角
            //（DWMWA_WINDOW_CORNER_PREFERENCE = ROUND；Win10 无此属性，静默忽略）
            if let Some(main) = app.get_webview_window("main") {
                use windows::Win32::Foundation::HWND;
                use windows::Win32::Graphics::Dwm::{
                    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
                    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
                };
                if let Ok(hwnd) = main.hwnd() {
                    // 圆角
                    let pref = DWMWCP_ROUND;
                    let res = unsafe {
                        DwmSetWindowAttribute(
                            HWND(hwnd.0 as *mut _),
                            DWMWA_WINDOW_CORNER_PREFERENCE,
                            &pref as *const _ as *const std::ffi::c_void,
                            std::mem::size_of_val(&pref) as u32,
                        )
                    };
                    if let Err(e) = res {
                        eprintln!("[win] 圆角设置失败（Win10 不支持，可忽略）: {e}");
                    }
                    // Win11 对活动窗口会画一圈系统强调色描边，置 NONE 去掉
                    let none = DWMWA_COLOR_NONE;
                    let res2 = unsafe {
                        DwmSetWindowAttribute(
                            HWND(hwnd.0 as *mut _),
                            DWMWA_BORDER_COLOR,
                            &none as *const _ as *const std::ffi::c_void,
                            std::mem::size_of_val(&none) as u32,
                        )
                    };
                    if let Err(e) = res2 {
                        eprintln!("[win] 边框颜色设置失败（Win10 不支持，可忽略）: {e}");
                    }
                }
            }
            let app_data = handle.path().app_data_dir().map_err(|e| e.to_string())?;
            std::fs::create_dir_all(app_data.join("covers")).map_err(|e| e.to_string())?;
            std::fs::create_dir_all(app_data.join("downloads")).map_err(|e| e.to_string())?;

            // 旧库存量封面补 160px 列表缩略图（后台线程，不阻塞启动）
            library::migrate_cover_thumbs(&app_data);

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

            // 恢复 WASAPI 独占模式设置（默认关闭）
            eng.set_exclusive_enabled(
                db::get_setting(&conn, "wasapi_exclusive").as_deref() == Some("true"),
            );

            // 恢复保存的输出设备偏好（空 = 跟随系统默认）
            {
                let pref = db::get_setting(&conn, "output_device").filter(|s| !s.is_empty());
                if let Some(name) = pref {
                    if let Err(e) = eng.switch_output_device(Some(&name)) {
                        // 设备已不存在：回落默认并在日志说明
                        eprintln!("[engine] 恢复输出设备「{name}」失败: {e}");
                    }
                }
            }

            app.manage(AppState {
                db: Mutex::new(conn),
                engine: eng,
                app_data: app_data.clone(),
                webview_suspended: AtomicBool::new(false),
                scan_last: Mutex::new(
                    serde_json::json!({ "active": false, "done": 0, "total": 0 }),
                ),
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
            commands::asset_scope_allow,
            commands::prepare_skin_image,
            commands::navidrome_save,
            commands::navidrome_connect,
            commands::navidrome_forget,
            commands::navidrome_search,
            commands::navidrome_albums,
            commands::navidrome_album_songs,
            commands::navidrome_all_songs,
            commands::navidrome_play,
            commands::navidrome_lyric,
            commands::kugou_search,
            commands::kugou_play,
            commands::kugou_lyric,
            commands::kugou_qr_create,
            commands::kugou_qr_check,
            commands::kugou_status,
            commands::kugou_logout,
            commands::kugou_toplists,
            commands::kugou_toplist_tracks,
            commands::kugou_random_playlist,
            commands::kugou_playlist_tracks,
            commands::kugou_user_playlists,
            commands::kugou_import_playlist,
            commands::add_to_playlist,
            commands::remove_from_playlist,
            commands::list_sources,
            commands::add_source,
            commands::delete_source,
            commands::play_track,
            commands::bilibili_add,
            commands::bilibili_play,
            commands::bilibili_qr_create,
            commands::bilibili_qr_check,
            commands::bilibili_status,
            commands::bilibili_logout,
            commands::bilibili_lyric,
            commands::bilibili_space,
            commands::bilibili_space_more,
            commands::bilibili_space_collection,
            commands::bilibili_space_collection_more,
            commands::bilibili_fav_folders,
            commands::bilibili_fav_list,
            commands::bilibili_video_info,
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
            commands::qq_toplists,
            commands::qq_toplist_tracks,
            commands::qq_random_playlist,
            commands::netease_toplists,
            commands::netease_toplist_tracks,
            commands::netease_random_playlist,
            commands::netease_daily_recommend,
            commands::netease_personal_fm,
            commands::set_wasapi_exclusive,
            commands::set_play_quality,
            commands::set_close_action,
            commands::extract_cover_palette,
            commands::play_pause,
            commands::get_play_state,
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
            commands::desktop_lyrics_is_open,
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
