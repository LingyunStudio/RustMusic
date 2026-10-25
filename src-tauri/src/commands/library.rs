//! 媒体库：曲目列表 / 文件夹管理 / 扫描进度 / 拖拽导入 / 喜欢

use tauri::{AppHandle, Manager, State};

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
    let added = {
        let conn = state.db.lock();
        db::add_folder(&conn, &norm).map(|(_, added)| added)?
    };
    if added {
        library::spawn_scan(&app);
    }
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

/// 在系统文件管理器中打开文件夹
#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        let _ = std::fs::create_dir_all(&p);
    }
    if !p.is_dir() {
        return Err("该路径不是文件夹".into());
    }
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        #[cfg(target_os = "windows")]
        let mut cmd = {
            // explorer 已打开该目录时聚焦，否则新开窗口
            std::process::Command::new("explorer")
        };
        #[cfg(target_os = "linux")]
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(&path)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {e}"))?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        return Err("当前平台不支持".into());
    }
    Ok(())
}
#[derive(serde::Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DropResult {
    pub folders_added: u32,
    pub tracks_added: Vec<crate::models::TrackMeta>,
}

/// 拖拽导入：文件夹加入媒体库，音频文件直接入库
#[tauri::command]
pub async fn drop_paths(app: AppHandle, paths: Vec<String>) -> Result<DropResult, String> {
    let task_app = app.clone();
    super::blocking(move || drop_paths_task(&task_app, paths)).await
}

/// 同步任务体：parse_track 会解码内嵌封面（大图可达数十 MB），多文件拖拽明显耗时
fn drop_paths_task(app: &AppHandle, paths: Vec<String>) -> Result<DropResult, String> {
    let state = app.state::<AppState>();
    let mut folders_added = 0u32;
    let mut tracks_added = Vec::new();
    let mut need_scan = false;
    for p in paths {
        let pb = std::path::PathBuf::from(&p);
        if !pb.exists() {
            continue;
        }
        if pb.is_dir() {
            let norm = library::norm_path(&pb);
            let conn = state.db.lock();
            if let Ok((_, added)) = db::add_folder(&conn, &norm) {
                if added {
                    folders_added += 1;
                    need_scan = true;
                }
            }
        } else if library::is_audio(&pb) {
            let app_data = state.app_data.clone();
            if let Some(t) = library::parse_track(&pb, &app_data) {
                let conn = state.db.lock();
                db::upsert_track(&conn, &t);
                if let Some(meta) = db::get_track_by_path(&conn, &t.path) {
                    tracks_added.push(meta);
                }
            }
        }
    }
    if need_scan {
        library::spawn_scan(app);
    }
    Ok(DropResult {
        folders_added,
        tracks_added,
    })
}
// ---------- 喜欢 / 统计 ----------

#[tauri::command]
pub async fn like_track(state: State<'_, AppState>, id: i64, liked: bool) -> Result<(), String> {
    let conn = state.db.lock();
    db::like_track(&conn, id, liked);
    Ok(())
}

/// 批量喜欢/取消喜欢（多选场景）：单事务提交
#[tauri::command]
pub async fn like_tracks(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    liked: bool,
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    let conn = state.db.lock();
    match conn.unchecked_transaction() {
        Ok(tx) => {
            for id in &ids {
                db::like_track(&tx, *id, liked);
            }
            let _ = tx.commit();
        }
        Err(_) => {
            for id in &ids {
                db::like_track(&conn, *id, liked);
            }
        }
    }
    Ok(())
}

// ---------- 删除曲目 ----------

#[tauri::command]
pub async fn delete_track(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    delete_tracks(app, state, vec![id]).await.map(|_| ())
}

#[tauri::command]
pub async fn delete_tracks(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> Result<u32, String> {
    let _ = state;
    let task_app = app.clone();
    super::blocking(move || delete_tracks_task(&task_app, ids)).await
}

fn delete_tracks_task(app: &AppHandle, ids: Vec<i64>) -> Result<u32, String> {
    if ids.is_empty() {
        return Ok(0);
    }
    let state = app.state::<AppState>();
    let engine = super::engine_clone(&state);

    // 如果正在播放其中某首，先停止播放释放文件句柄
    let current_id =
        engine.current.read().as_ref().and_then(
            |info| {
                if info.kind == "track" {
                    info.id
                } else {
                    None
                }
            },
        );
    if let Some(cid) = current_id {
        if ids.contains(&cid) {
            engine.stop();
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
    }

    let mut successful_ids = Vec::with_capacity(ids.len());
    let mut last_err = None;

    for id in &ids {
        let meta = {
            let conn = state.db.lock();
            db::get_track(&conn, *id)
        };
        let Some(t) = meta else {
            continue;
        };

        let pb = std::path::PathBuf::from(&t.path);
        if pb.exists() {
            // 清理只读属性
            if let Ok(m) = std::fs::metadata(&pb) {
                let mut perms = m.permissions();
                if perms.readonly() {
                    perms.set_readonly(false);
                    let _ = std::fs::set_permissions(&pb, perms);
                }
            }
            if let Err(e) = std::fs::remove_file(&pb) {
                last_err = Some(format!("删除文件失败 ({}): {e}", t.path));
                continue;
            }
            let lrc = pb.with_extension("lrc");
            if lrc.exists() {
                let _ = std::fs::remove_file(lrc);
            }
        }

        // 清理缓存目录中对应的封面小图与原图（限定在 covers 缓存目录下）
        if !t.cover.is_empty() {
            let cov = std::path::PathBuf::from(&t.cover);
            let covers_dir = state.app_data.join("covers");
            let is_cache = if let (Ok(p1), Ok(p2)) = (cov.canonicalize(), covers_dir.canonicalize())
            {
                p1.starts_with(p2)
            } else {
                cov.starts_with(&covers_dir)
            };
            if is_cache && cov.exists() {
                let _ = std::fs::remove_file(&cov);
                // 小图路径按文件名推导（不能对全路径 replace(".jpg")：
                // 上级目录名里含 ".jpg" 时会替换到目录段）
                if let (Some(stem), Some(ext)) = (
                    cov.file_stem().and_then(|s| s.to_str()),
                    cov.extension().and_then(|e| e.to_str()),
                ) {
                    let small = cov.with_file_name(format!("{stem}_s.{ext}"));
                    let _ = std::fs::remove_file(small);
                }
            }
        }

        successful_ids.push(*id);
    }

    if successful_ids.is_empty() && last_err.is_some() {
        return Err(last_err.unwrap());
    }

    if !successful_ids.is_empty() {
        let conn = state.db.lock();
        db::delete_tracks(&conn, &successful_ids);
    }

    Ok(successful_ids.len() as u32)
}
