//! 自动更新（GitHub Release）：检查 / 下载 / 安装

use tauri::{AppHandle, Emitter, Manager};

use crate::db;
use crate::updater;
use crate::AppState;
// ---------- 自动更新（GitHub Release） ----------

/// 手动检查更新：有新版本返回安装包信息，已是最新返回 null
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<updater::UpdateInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let current = app.package_info().version.to_string();
        updater::fetch_latest(&current)
    })
    .await
    .map_err(|e| format!("检查更新任务失败：{e}"))?
}

/// 前端就绪后触发一次启动自动检查（后台线程执行，结果通过
/// `update://available` 事件推送；调试构建不检查，避免开发时误装到 target 目录）
#[tauri::command]
pub async fn auto_check_update(app: AppHandle) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Ok(());
    }
    let enabled = {
        let st = app.state::<AppState>();
        let conn = st.db.lock();
        db::get_setting(&conn, "auto_update")
            .map(|s| s != "false")
            .unwrap_or(true)
    };
    if !enabled {
        return Ok(());
    }
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let current = app.package_info().version.to_string();
        match updater::fetch_latest(&current) {
            Ok(Some(info)) => {
                let _ = app.emit("update://available", info);
            }
            Ok(None) => eprintln!("[updater] 已是最新版本 {current}"),
            Err(e) => eprintln!("[updater] 自动检查更新失败：{e}"),
        }
    });
    Ok(())
}

/// 下载安装包到临时目录，进度通过 `update://progress` 事件上报，返回文件路径
#[tauri::command]
pub async fn download_update(
    app: AppHandle,
    url: String,
    name: String,
    size: u64,
    digest: Option<String>,
) -> Result<String, String> {
    // 下载可能持续数分钟：放到阻塞线程池，避免占用异步运行时
    tauri::async_runtime::spawn_blocking(move || {
        updater::download(&app, &url, &name, size, digest.as_deref())
    })
    .await
    .map_err(|e| format!("下载任务失败：{e}"))?
    .map(|p| p.to_string_lossy().to_string())
}

/// 取消进行中的下载
#[tauri::command]
pub async fn cancel_update_download() -> Result<(), String> {
    updater::cancel_download();
    Ok(())
}

/// 安装已下载的安装包并重启应用（分离助手接管后本进程自动退出）
#[tauri::command]
pub async fn install_update(app: AppHandle, path: String) -> Result<(), String> {
    updater::install_and_restart(&app, std::path::Path::new(&path))
}
