//! 媒体库：曲目列表 / 文件夹管理 / 扫描进度 / 拖拽导入 / 喜欢

use tauri::{AppHandle, State};

use crate::db;
use crate::library;
use crate::models::{Folder, TrackMeta};
use crate::AppState;
// ---------- 媒体库 ----------

#[tauri::command]
pub async fn list_tracks(state: State<'_, AppState>) -> Result<Vec<TrackMeta>, String> {
    let conn = state.db.lock();
    // 返回全量记录（含 missing 软删除），由前端按视图过滤：
    // 资料库隐藏 missing，“我喜欢/最近播放”保留记录（文件没了也显示，仅是引用）
    Ok(db::list_tracks(&conn))
}

/// 最近一次扫描进度快照（WebView 挂起期间 scan://progress 事件丢失，恢复后补发）
#[tauri::command]
pub async fn get_scan_state(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    Ok(state.scan_last.lock().clone())
}

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> Result<Vec<Folder>, String> {
    let conn = state.db.lock();
    Ok(db::list_folders(&conn))
}

#[tauri::command]
pub async fn add_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.is_dir() {
        return Err("该路径不是文件夹".into());
    }
    let norm = library::norm_path(&p);
    {
        let conn = state.db.lock();
        db::add_folder(&conn, &norm)?;
    }
    library::spawn_scan(&app);
    Ok(())
}

#[tauri::command]
pub async fn remove_folder(
    state: State<'_, AppState>,
    app: AppHandle,
    id: i64,
) -> Result<(), String> {
    {
        let conn = state.db.lock();
        db::remove_folder(&conn, id);
    }
    library::spawn_scan(&app);
    Ok(())
}

#[tauri::command]
pub async fn rescan(app: AppHandle) -> Result<(), String> {
    library::spawn_scan(&app);
    Ok(())
}

/// 在资源管理器中打开文件夹
#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.is_dir() {
        return Err("该路径不是文件夹".into());
    }
    #[cfg(target_os = "windows")]
    {
        // explorer 已打开该目录时聚焦，否则新开窗口
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("打开资源管理器失败: {e}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = p;
        return Err("当前平台不支持".into());
    }
    Ok(())
}
/// 拖拽导入：文件夹加入媒体库，音频文件直接入库
#[tauri::command]
pub async fn drop_paths(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<u32, String> {
    let mut added = 0u32;
    let mut need_scan = false;
    for p in paths {
        let pb = std::path::PathBuf::from(&p);
        if !pb.exists() {
            continue;
        }
        if pb.is_dir() {
            let norm = library::norm_path(&pb);
            let conn = state.db.lock();
            if db::add_folder(&conn, &norm).is_ok() {
                added += 1;
                need_scan = true;
            }
        } else if library::is_audio(&pb) {
            let app_data = state.app_data.clone();
            if let Some(t) = library::parse_track(&pb, &app_data) {
                let conn = state.db.lock();
                db::upsert_track(&conn, &t);
                added += 1;
            }
        }
    }
    if need_scan {
        library::spawn_scan(&app);
    }
    Ok(added)
}
// ---------- 喜欢 / 统计 ----------

#[tauri::command]
pub async fn like_track(state: State<'_, AppState>, id: i64, liked: bool) -> Result<(), String> {
    let conn = state.db.lock();
    db::like_track(&conn, id, liked);
    Ok(())
}
