#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bilibili;
mod commands;
mod credentials;
mod db;
mod engine;
mod eq;
mod fx;
mod kugou;
mod library;
mod lyrics;
mod models;
mod monitor;
mod navidrome;
mod netease;
mod qq;
mod qrc;
mod smtc;
mod streaming;
mod symdec;
mod tray;
mod updater;
#[cfg(windows)]
mod wasapi_out;
#[cfg(not(windows))]
#[path = "wasapi_out_stub.rs"]
mod wasapi_out;
#[cfg(windows)]
mod taskbar_lyric;
#[cfg(not(windows))]
#[path = "taskbar_lyric_stub.rs"]
mod taskbar_lyric;
mod webview_suspend;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::{Emitter, Manager};

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
    /// 桌面歌词是否处于鼠标穿透锁定状态
    pub dlyrics_locked: AtomicBool,
}

/// 是否运行在虚拟机里（VMware/VirtualBox/QEMU/Hyper-V 等常见厂商）。
/// 虚拟机的软渲染栈 GPU 合成不可用，WebView 透明需要退到 Cairo 路径
#[cfg(target_os = "linux")]
fn is_virtual_machine() -> bool {
    let vendor = std::fs::read_to_string("/sys/class/dmi/id/sys_vendor")
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    ["vmware", "innotek", "oracle", "qemu", "microsoft corporation", "xen"]
        .iter()
        .any(|v| vendor.contains(v))
}

fn main() {
    // WebKitGTK 2.4x 默认的 DMABUF 渲染器在虚拟机（vmwgfx）等无 DMA-BUF
    // 支持的显卡驱动上整窗黑屏（界面与音频均正常，仅画面不绘制）。
    // 未显式设置时禁用它，回退旧渲染路径；真机有 DMABUF 支持时两种路径
    // 行为一致，无副作用。仅 Linux 生效，不影响 Windows
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // 虚拟机 GPU 合成不可用：禁用合成走 Cairo 路径后 WebView 才能输出
        // alpha，桌面歌词的"纯悬浮文字"形态（无实色黑底）依赖这一点
        if is_virtual_machine() && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
        // Wayland 会话改走 XWayland：Wayland 协议禁止应用自我置顶，GTK 的
        // set_keep_above 是无操作，桌面歌词的置顶静默失效；X11 语义下置顶
        // 与鼠标穿透都可用（XWayland 由 GNOME/KDE 等常规 Wayland 会话提供）。
        // 必须在 GTK 初始化前设置。GNOME 会话会全局导出 GDK_BACKEND=wayland
        //（会话默认而非用户选择），同样覆盖；用户显式指定的其它后端优先。
        // 无 XWayland（无 DISPLAY）时不干预，仍走原生 Wayland
        if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DISPLAY").is_some() {
            match std::env::var("GDK_BACKEND") {
                Ok(v) if !v.is_empty() && !v.contains("wayland") => {}
                _ => std::env::set_var("GDK_BACKEND", "x11"),
            }
        }
    }
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
            } else if window.label() == "desktop-lyrics" {
                match event {
                    // 拖动/缩放即时（防抖）落盘：托盘退出（app.exit）不走
                    // desktop_lyrics_close，不在这里存的话位置记忆会丢
                    tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {
                        commands::schedule_persist_dlyrics_geom(window.app_handle());
                    }
                    // 系统途径关闭（Alt+F4 等）：与"词"按钮同语义——存几何、
                    // 隐藏窗口、同步主窗口按钮状态与托盘。不真正销毁（销毁
                    // 与重开的创建循环有僵尸窗口竞态，见 desktop_lyrics_close）
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let app = window.app_handle();
                        commands::persist_dlyrics_geom(app);
                        app.state::<AppState>()
                            .dlyrics_locked
                            .store(false, Ordering::SeqCst);
                        let _ = window.hide();
                        let _ = app.emit("dlyrics://closed", serde_json::json!({}));
                        tray::update_dlyrics_tray(app);
                    }
                    _ => {}
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            // Linux：部分窗口管理器（KWin 等）对启动期映射的窗口按"启动时最小化"
            // 处理（焦窃取防护）；配置里窗口 visible=false，setup 完成后显式
            // 显示并聚焦，确保启动即可见
            #[cfg(target_os = "linux")]
            {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                }
            }
            // 无框窗口关掉 DWM 边框后 Win11 会失去圆角，这里单独设回圆角
            //（DWMWA_WINDOW_CORNER_PREFERENCE = ROUND；Win10 无此属性，静默忽略）
            #[cfg(windows)]
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

            // 存量明文凭据迁移：settings 表 → 系统凭据管理器（成功即清库内残留）
            credentials::migrate_legacy(&conn);

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
            // 任务栏歌词渲染线程（Windows 常驻控制窗口；其它平台桩为空）
            taskbar_lyric::spawn();
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

            // 恢复 WASAPI 独占模式设置（默认关闭）。独占仅 Windows：
            // 其它平台恒为关（数据库设置值不改，迁回 Windows 仍生效），
            // 避免旧库带 true 时每首歌都触发一次"独占不可用"回退提示
            #[cfg(windows)]
            let excl_pref = db::get_setting(&conn, "wasapi_exclusive").as_deref() == Some("true");
            #[cfg(not(windows))]
            let excl_pref = false;
            eng.set_exclusive_enabled(excl_pref);

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
                dlyrics_locked: AtomicBool::new(false),
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
            commands::like_tracks,
            commands::delete_track,
            commands::delete_tracks,
            commands::list_playlists,
            commands::create_playlist,
            commands::delete_playlist,
            commands::delete_playlists,
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
            commands::add_tracks_to_playlist,
            commands::export_playlist_m3u,
            commands::import_playlist_m3u,
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
            commands::qq_wx_qr_create,
            commands::qq_wx_qr_check,
            commands::qq_status,
            commands::qq_logout,
            commands::like_online,
            commands::download_online,
            commands::downloaded_online_list,
            commands::liked_online_list,
            commands::recent_online_list,
            commands::save_dir_get,
            commands::save_dir_set,
            commands::add_online_to_playlist,
            commands::remove_playlist_entry,
            commands::remove_playlist_entries,
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
            commands::fetch_cover,
            commands::desktop_lyrics_set_always_top,
            commands::dlyrics_menubar_line,
            commands::menubar_extension_install,
            commands::engine_preload_next,
            commands::dlyrics_taskbar_line,
            commands::taskbar_lyrics_set_enabled,
            commands::taskbar_lyrics_list_displays,
            commands::taskbar_lyrics_set_display,
            commands::dlyrics_taskbar_set_lines,
            commands::dlyrics_taskbar_color,
            commands::play_pause,
            commands::get_play_state,
            commands::pause,
            commands::resume,
            commands::stop,
            commands::seek,
            commands::set_volume,
            commands::fx_set_enabled,
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
