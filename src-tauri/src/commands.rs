use serde_json::json;
use lofty::prelude::*;
use tauri::{AppHandle, State};

use crate::db;
use crate::engine::TrackInfo;
use crate::library;
use crate::lyrics;
use crate::models::*;
use crate::AppState;

fn engine_clone(state: &State<AppState>) -> std::sync::Arc<crate::engine::Engine> {
    state.engine.lock().clone()
}

// ---------- 媒体库 ----------

#[tauri::command]
pub async fn list_tracks(state: State<'_, AppState>) -> Result<Vec<TrackMeta>, String> {
    let conn = state.db.lock();
    Ok(db::list_tracks(&conn))
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
pub async fn remove_folder(state: State<'_, AppState>, app: AppHandle, id: i64) -> Result<(), String> {
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

// ---------- 歌词 ----------

#[tauri::command]
pub async fn get_lyrics(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<LyricsPayload, String> {
    let (path, lrc) = {
        let conn = state.db.lock();
        let path = db::get_track_path(&conn, track_id).ok_or("曲目不存在")?;
        let lrc = db::get_lrc_path(&conn, track_id);
        (path, lrc)
    };

    if let Some(lrc_path) = lrc {
        if let Ok(text) = std::fs::read_to_string(&lrc_path) {
            let p = lyrics::parse(&text);
            if !p.lines.is_empty() {
                return Ok(LyricsPayload { synced: p.synced, lines: p.lines });
            }
        }
    }

    // 内嵌歌词
    if let Ok(tagged) = lofty::read_from_path(&path) {
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            if let Some(text) = tag.get_string(&lofty::tag::ItemKey::Lyrics) {
                let p = lyrics::parse(text);
                if !p.lines.is_empty() {
                    return Ok(LyricsPayload { synced: p.synced, lines: p.lines });
                }
            }
        }
    }
    Ok(LyricsPayload { synced: false, lines: vec![] })
}

// ---------- 喜欢 / 统计 ----------

#[tauri::command]
pub async fn like_track(state: State<'_, AppState>, id: i64, liked: bool) -> Result<(), String> {
    let conn = state.db.lock();
    db::like_track(&conn, id, liked);
    Ok(())
}

// ---------- 播放列表 ----------

#[tauri::command]
pub async fn list_playlists(state: State<'_, AppState>) -> Result<Vec<Playlist>, String> {
    let conn = state.db.lock();
    Ok(db::list_playlists(&conn))
}

#[tauri::command]
pub async fn create_playlist(
    state: State<'_, AppState>,
    name: String,
) -> Result<i64, String> {
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

// ---------- 在线音源 ----------

#[tauri::command]
pub async fn list_sources(state: State<'_, AppState>) -> Result<Vec<SourceItem>, String> {
    let conn = state.db.lock();
    Ok(db::list_sources(&conn))
}

#[tauri::command]
pub async fn add_source(
    state: State<'_, AppState>,
    url: String,
    title: Option<String>,
) -> Result<i64, String> {
    let url = url.trim().to_string();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("音源地址必须以 http:// 或 https:// 开头".into());
    }
    if url.contains(".m3u8") {
        return Err("暂不支持 m3u8/HLS，请使用音频文件直链".into());
    }
    let conn = state.db.lock();
    if let Some(item) = db::list_sources(&conn).into_iter().find(|s| s.url == url) {
        return Ok(item.id);
    }
    db::add_source(&conn, &url, title.as_deref().unwrap_or(""))
}

#[tauri::command]
pub async fn delete_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let conn = state.db.lock();
    db::delete_source(&conn, id);
    Ok(())
}

// ---------- 播放控制 ----------

#[tauri::command]
pub async fn play_track(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let meta = {
        let conn = state.db.lock();
        db::get_track(&conn, id).ok_or("曲目不存在")?
    };
    {
        let conn = state.db.lock();
        db::record_play(&conn, id);
    }
    let info = TrackInfo {
        id: Some(meta.id),
        kind: "track".into(),
        path: meta.path,
        title: meta.title,
        artist: meta.artist,
        album: meta.album,
        cover: meta.cover,
        duration_ms: (meta.duration * 1000.0) as u64,
        nid: None,
    };
    engine_clone(&state).play_file(info)
}

#[tauri::command]
pub async fn play_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let item = {
        let conn = state.db.lock();
        db::get_source(&conn, id).ok_or("音源不存在")?
    };
    let info = TrackInfo {
        id: None,
        kind: "url".into(),
        path: String::new(),
        title: if item.title.is_empty() {
            item.url.split('/').next_back().unwrap_or("在线音源").to_string()
        } else {
            item.title.clone()
        },
        artist: "在线音源".into(),
        album: String::new(),
        cover: String::new(),
        duration_ms: 0,
        nid: None,
    };
    engine_clone(&state).play_url(item.url, info)
}

// ---------- 网易云在线曲库 ----------

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeteasePlayReq {
    pub id: i64,
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

fn netease_cookie(state: &State<AppState>) -> Option<String> {
    let conn = state.db.lock();
    db::get_setting(&conn, "netease_music_u").filter(|s| !s.is_empty())
}

#[tauri::command]
pub async fn netease_search(
    state: State<'_, AppState>,
    keyword: String,
) -> Result<crate::netease::NetSearchResult, String> {
    let music_u = netease_cookie(&state);
    crate::netease::search(&keyword, 30, music_u.as_deref())
}

#[tauri::command]
pub async fn netease_play(
    app: AppHandle,
    state: State<'_, AppState>,
    track: NeteasePlayReq,
) -> Result<(), String> {
    let music_u = netease_cookie(&state);
    let (url, _br) = crate::netease::song_url(track.id, music_u.as_deref())?
        .ok_or_else(|| {
            "该歌曲暂无可播放链接（可能需要登录，或需要有效 VIP 权益）".to_string()
        })?;
    let info = TrackInfo {
        id: None,
        kind: "netease".into(),
        path: String::new(),
        title: track.title,
        artist: track.artist,
        album: track.album,
        cover: track.cover,
        duration_ms: track.duration_ms,
        nid: Some(track.id),
    };
    let _ = app; // 事件由引擎发出
    engine_clone(&state).play_url(url, info)
}

#[tauri::command]
pub async fn netease_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();
    let music_u = db::get_setting(&conn, "netease_music_u").unwrap_or_default();
    let nickname = db::get_setting(&conn, "netease_nickname").unwrap_or_default();
    Ok(json!({
        "loggedIn": !music_u.is_empty(),
        "nickname": nickname,
    }))
}

#[tauri::command]
pub async fn netease_qr_create() -> Result<serde_json::Value, String> {
    let (key, qr) = crate::netease::qr_create()?;
    Ok(json!({ "key": key, "qr": qr }))
}

#[tauri::command]
pub async fn netease_qr_check(
    state: State<'_, AppState>,
    key: String,
) -> Result<serde_json::Value, String> {
    let r = crate::netease::qr_check(&key)?;
    if r.status == "success" {
        if let Some(music_u) = &r.music_u {
            let conn = state.db.lock();
            db::set_setting(&conn, "netease_music_u", music_u);
            if let Some(nick) = &r.nickname {
                db::set_setting(&conn, "netease_nickname", nick);
            }
            if let Some(uid) = &r.user_id {
                db::set_setting(&conn, "netease_uid", &uid.to_string());
            }
        }
    }
    Ok(json!({
        "status": r.status,
        "nickname": r.nickname,
    }))
}

#[tauri::command]
pub async fn netease_like_list(state: State<'_, AppState>) -> Result<Vec<i64>, String> {
    let (uid, music_u) = {
        let conn = state.db.lock();
        (
            db::get_setting(&conn, "netease_uid").unwrap_or_default(),
            db::get_setting(&conn, "netease_music_u").unwrap_or_default(),
        )
    };
    if uid.is_empty() || music_u.is_empty() {
        return Ok(vec![]);
    }
    let uid: i64 = uid.parse().map_err(|_| "账号 ID 无效".to_string())?;
    crate::netease::like_list(uid, &music_u)
}

#[tauri::command]
pub async fn netease_like(
    state: State<'_, AppState>,
    id: i64,
    like: bool,
) -> Result<(), String> {
    let music_u = netease_cookie(&state).ok_or("未登录网易云账号")?;
    crate::netease::like(id, like, &music_u)
}

#[tauri::command]
pub async fn netease_lyric(
    state: State<'_, AppState>,
    id: i64,
) -> Result<LyricsPayload, String> {
    let music_u = netease_cookie(&state);
    let lrc = crate::netease::lyric(id, music_u.as_deref())?;
    let text = lrc.unwrap_or_default();
    if text.is_empty() {
        return Ok(LyricsPayload { synced: false, lines: vec![] });
    }
    let p = lyrics::parse(&text);
    Ok(LyricsPayload { synced: p.synced, lines: p.lines })
}

#[tauri::command]
pub async fn netease_logout(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    db::set_setting(&conn, "netease_music_u", "");
    db::set_setting(&conn, "netease_nickname", "");
    Ok(())
}

#[tauri::command]
pub async fn play_pause(state: State<'_, AppState>) -> Result<(), String> {
    engine_clone(&state).toggle();
    Ok(())
}

#[tauri::command]
pub async fn pause(state: State<'_, AppState>) -> Result<(), String> {
    engine_clone(&state).pause();
    Ok(())
}

#[tauri::command]
pub async fn resume(state: State<'_, AppState>) -> Result<(), String> {
    engine_clone(&state).resume();
    Ok(())
}

#[tauri::command]
pub async fn stop(state: State<'_, AppState>) -> Result<(), String> {
    engine_clone(&state).stop();
    Ok(())
}

#[tauri::command]
pub async fn seek(state: State<'_, AppState>, ms: u64) -> Result<(), String> {
    engine_clone(&state).seek(ms)
}

#[tauri::command]
pub async fn set_volume(state: State<'_, AppState>, v: f32) -> Result<(), String> {
    engine_clone(&state).set_volume(v);
    let conn = state.db.lock();
    db::set_setting(&conn, "volume", &format!("{}", v));
    Ok(())
}

#[tauri::command]
pub async fn set_speed(state: State<'_, AppState>, v: f32) -> Result<(), String> {
    engine_clone(&state).set_speed(v);
    let conn = state.db.lock();
    db::set_setting(&conn, "speed", &format!("{}", v));
    Ok(())
}

#[tauri::command]
pub async fn set_eq(
    state: State<'_, AppState>,
    gains: Vec<f32>,
    enabled: bool,
) -> Result<(), String> {
    if gains.len() != 10 {
        return Err("均衡器需要 10 个频段的增益".into());
    }
    let mut arr = [0f32; 10];
    arr.copy_from_slice(&gains);
    engine_clone(&state).eq.set(arr, enabled);
    let conn = state.db.lock();
    db::set_setting(&conn, "eq_gains", &serde_json::to_string(&gains).unwrap());
    db::set_setting(&conn, "eq_enabled", &enabled.to_string());
    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    let conn = state.db.lock();
    let volume: f32 = db::get_setting(&conn, "volume")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.8);
    let speed: f32 = db::get_setting(&conn, "speed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let eq_gains: Vec<f32> = db::get_setting(&conn, "eq_gains")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| vec![0.0; 10]);
    let eq_enabled = db::get_setting(&conn, "eq_enabled")
        .map(|s| s == "true")
        .unwrap_or(false);
    Ok(SettingsPayload { volume, speed, eq_gains, eq_enabled })
}

// ---------- 其他 ----------

#[tauri::command]
pub async fn clear_cache(state: State<'_, AppState>) -> Result<u32, String> {
    let dir = state.app_data.join("downloads");
    let mut n = 0u32;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.path().is_file() && std::fs::remove_file(e.path()).is_ok() {
                n += 1;
            }
        }
    }
    Ok(n)
}

#[tauri::command]
pub async fn get_app_info(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    Ok(json!({
        "version": app.package_info().version.to_string(),
        "dataDir": state.app_data.to_string_lossy(),
    }))
}
