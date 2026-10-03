//! Navidrome（Subsonic 兼容，密码存凭据管理器）

use serde_json::json;
use tauri::{AppHandle, Manager, State};

use crate::db;
use crate::engine::TrackInfo;
use crate::AppState;

use super::engine_clone;
// ---------- Navidrome（Subsonic 兼容，密码存凭据管理器） ----------

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NdSaveReq {
    pub server: String,
    pub username: String,
    pub password: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NdPlayReq {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub cover: String,
    #[serde(default)]
    pub duration_ms: u64,
}

#[tauri::command]
pub async fn navidrome_save(state: State<'_, AppState>, req: NdSaveReq) -> Result<(), String> {
    let server = crate::navidrome::norm_base(&req.server);
    let username = req.username.trim().to_string();
    if server.is_empty() || username.is_empty() || req.password.is_empty() {
        return Err("服务器地址、用户名、密码均不能为空".into());
    }
    // 先验证再落凭据（ping 为网络调用、keyring 为阻塞 IO，统一进阻塞线程池）
    let password = req.password.clone();
    let (server2, username2) = (server.clone(), username.clone());
    super::blocking(move || -> Result<(), String> {
        crate::navidrome::ping(&server2, &username2, &password)?;
        crate::navidrome::save_password(&server2, &username2, &password)
    })
    .await?;
    // 非秘密的连接信息存设置库（供播放/下载时读取）
    let conn = state.db.lock();
    db::set_setting(&conn, "navidrome_server", &server);
    db::set_setting(&conn, "navidrome_username", &username);
    Ok(())
}

#[tauri::command]
pub async fn navidrome_connect(
    state: State<'_, AppState>,
    server: String,
    username: String,
) -> Result<(), String> {
    let _ = state;
    super::blocking(move || crate::navidrome::ping(&crate::navidrome::norm_base(&server), &username, ""))
        .await
        .map_err(|e| format!("连接失败：{e}（请检查服务器地址或重新保存密码）"))
}

#[tauri::command]
pub async fn navidrome_forget(server: String, username: String) -> Result<(), String> {
    super::blocking(move || crate::navidrome::delete_password(&crate::navidrome::norm_base(&server), &username))
        .await
}

#[tauri::command]
pub async fn navidrome_search(
    state: State<'_, AppState>,
    server: String,
    username: String,
    query: String,
) -> Result<Vec<crate::navidrome::NdSong>, String> {
    let saved = {
        let conn = state.db.lock();
        db::get_setting(&conn, "navidrome_server").unwrap_or_default()
    };
    let server = if server.is_empty() { saved } else { server };
    super::blocking(move || crate::navidrome::search_songs(&server, &username, &query)).await
}

#[tauri::command]
pub async fn navidrome_albums(
    state: State<'_, AppState>,
    server: String,
    username: String,
) -> Result<Vec<crate::navidrome::NdAlbum>, String> {
    let saved = {
        let conn = state.db.lock();
        db::get_setting(&conn, "navidrome_server").unwrap_or_default()
    };
    let server = if server.is_empty() { saved } else { server };
    super::blocking(move || crate::navidrome::album_list(&server, &username)).await
}

#[tauri::command]
pub async fn navidrome_album_songs(
    state: State<'_, AppState>,
    server: String,
    username: String,
    id: String,
) -> Result<serde_json::Value, String> {
    let saved = {
        let conn = state.db.lock();
        db::get_setting(&conn, "navidrome_server").unwrap_or_default()
    };
    let server = if server.is_empty() { saved } else { server };
    let (name, artist, songs) =
        super::blocking(move || crate::navidrome::album_songs(&server, &username, &id)).await?;
    Ok(json!({ "name": name, "artist": artist, "songs": songs }))
}

#[tauri::command]
pub async fn navidrome_all_songs(
    state: State<'_, AppState>,
    server: String,
    username: String,
    offset: u64,
) -> Result<serde_json::Value, String> {
    let saved = {
        let conn = state.db.lock();
        db::get_setting(&conn, "navidrome_server").unwrap_or_default()
    };
    let server = if server.is_empty() { saved } else { server };
    // password（keyring）一并带出：封面 URL 也要用它签名
    let (server2, username2) = (server.clone(), username.clone());
    let (v, password) = super::blocking(move || -> Result<(serde_json::Value, String), String> {
        let password = crate::navidrome::get_password_pub(&server2, &username2)?;
        let v = crate::navidrome::search_all(&server2, &username2, &password, offset)?;
        Ok((v, password))
    })
    .await?;
    let empty = Vec::new();
    let songs = v
        .pointer("/subsonic-response/searchResult3/song")
        .and_then(|x| x.as_array())
        .unwrap_or(&empty);
    let total = v
        .pointer("/subsonic-response/searchResult3/total")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let list: Vec<crate::navidrome::NdSong> = songs
        .iter()
        .map(|s| crate::navidrome::song_from_pub(s, &server, &username, &password))
        .collect();
    Ok(json!({ "songs": list, "total": total }))
}

#[tauri::command]
pub async fn navidrome_play(
    app: AppHandle,
    server: String,
    username: String,
    track: NdPlayReq,
) -> Result<(), String> {
    let task_app = app.clone();
    super::blocking(move || navidrome_play_task(&task_app, server, username, track)).await
}

/// 同步任务体：stream_url 内部读凭据管理器（阻塞 IO）
fn navidrome_play_task(
    app: &AppHandle,
    server: String,
    username: String,
    track: NdPlayReq,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let saved = {
        let conn = state.db.lock();
        db::get_setting(&conn, "navidrome_server").unwrap_or_default()
    };
    let server = if server.is_empty() { saved } else { server };
    let url = crate::navidrome::stream_url(&server, &username, &track.id)?;
    let info = TrackInfo {
        id: None,
        kind: "navidrome".into(),
        path: String::new(),
        title: track.title,
        artist: track.artist,
        album: track.album,
        cover: track.cover,
        duration_ms: track.duration_ms,
        nid: None,
        qid: Some(track.id),
        kgid: None,
        quality: None,
    };
    super::record_online_play(&state, &info, "", false);
    engine_clone(&state).play_url(url, info)
}

#[tauri::command]
pub async fn navidrome_lyric(
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::models::LyricsPayload, String> {
    let server = crate::navidrome::norm_base(&{
        let conn = state.db.lock();
        db::get_setting(&conn, "navidrome_server").unwrap_or_default()
    });
    let username = {
        let conn = state.db.lock();
        db::get_setting(&conn, "navidrome_username").unwrap_or_default()
    };
    super::blocking(move || crate::navidrome::lyrics(&server, &username, &id)).await
}
