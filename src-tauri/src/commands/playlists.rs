//! 播放列表：增删改 / 排序 / 条目管理（本地 + 在线条目）

use tauri::State;

use crate::db;
use crate::models::Playlist;
use crate::AppState;
// ---------- 播放列表 ----------

#[tauri::command]
pub async fn list_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    let conn = state.db.lock();
    Ok(db::list_playlists(&conn))
}

#[tauri::command]
pub async fn create_playlist(state: State<'_, AppState>, name: String) -> Result<i64, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("播放列表名称不能为空".into());
    }
    let conn = state.db.lock();
    db::create_playlist(&conn, &name)
}

#[tauri::command]
pub async fn delete_playlist(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    db::delete_playlist(&conn, id);
    Ok(())
}

#[tauri::command]
pub async fn rename_playlist(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::rename_playlist(&conn, id, &name);
    Ok(())
}

/// 播放列表手动排序（侧边栏长按拖动）：按 id 序列重写 sort_pos
#[tauri::command]
pub async fn reorder_playlists(state: State<'_, AppState>, ids: Vec<i64>) -> Result<(), String> {
    let conn = state.db.lock();
    db::reorder_playlists(&conn, &ids);
    Ok(())
}

#[tauri::command]
pub async fn add_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::add_to_playlist(&conn, playlist_id, track_id)
}

#[tauri::command]
pub async fn remove_from_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::remove_from_playlist(&conn, playlist_id, track_id);
    Ok(())
}
#[tauri::command]
pub async fn add_online_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    kind: String,
    rid: String,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    cover: Option<String>,
    duration_ms: Option<i64>,
    media_mid: Option<String>,
    vip: Option<bool>,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::upsert_online_track(
        &conn,
        &kind,
        &rid,
        &title,
        &artist.unwrap_or_default(),
        &album.unwrap_or_default(),
        &cover.unwrap_or_default(),
        duration_ms.unwrap_or(0),
        &media_mid.unwrap_or_default(),
        vip.unwrap_or(false),
    );
    db::add_online_to_playlist(&conn, playlist_id, &kind, &rid)?;
    Ok(())
}

#[tauri::command]
pub async fn remove_playlist_entry(state: State<'_, AppState>, rowid: i64) -> Result<(), String> {
    let conn = state.db.lock();
    db::remove_playlist_entry(&conn, rowid);
    Ok(())
}

/// 手动排序持久化：“资料库 / 我喜欢”整份顺序（全量覆盖）。
/// list: "library" | "liked"；keys 为行标识序列：
/// 本地 "track:<id>"、网易云 "netease:<rid>"、QQ "qq:<rid>"
#[tauri::command]
pub async fn save_manual_order(
    state: State<'_, AppState>,
    list: String,
    keys: Vec<String>,
) -> Result<(), String> {
    if !matches!(list.as_str(), "library" | "liked") {
        return Err("未知排序列表".into());
    }
    let conn = state.db.lock();
    db::save_manual_order(&conn, &list, &keys);
    Ok(())
}

/// 读取“资料库 / 我喜欢”的手动排序（row key → 序号；无记录的行序号为 0）
#[tauri::command]
pub async fn get_manual_order(
    state: State<'_, AppState>,
    list: String,
) -> Result<std::collections::HashMap<String, i64>, String> {
    if !matches!(list.as_str(), "library" | "liked") {
        return Err("未知排序列表".into());
    }
    let conn = state.db.lock();
    Ok(db::manual_order_map(&conn, &list))
}

/// 播放列表条目手动排序：按 rowid 序列重写 position（全量覆盖）
#[tauri::command]
pub async fn reorder_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    rowids: Vec<i64>,
) -> Result<(), String> {
    let conn = state.db.lock();
    db::reorder_playlist(&conn, playlist_id, &rowids);
    Ok(())
}
