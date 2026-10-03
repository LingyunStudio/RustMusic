//! 网易云音乐：搜索 / 播放取链 / 扫码登录 / 喜欢 / 歌词 / 榜单歌单

use serde_json::json;
use tauri::{AppHandle, Manager, State};

use crate::db;
use crate::engine::TrackInfo;
use crate::lyrics;
use crate::models::LyricsPayload;
use crate::AppState;

use super::{engine_clone, notify_quality_fallback, quality_tag};
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
pub(crate) fn netease_cookie(state: &State<AppState>) -> Option<String> {
    let conn = state.db.lock();
    db::get_setting(&conn, "netease_music_u").filter(|s| !s.is_empty())
}

#[tauri::command]
pub async fn netease_search(
    state: State<'_, AppState>,
    keyword: String,
    offset: Option<i64>,
) -> Result<crate::netease::NetSearchResult, String> {
    let music_u = netease_cookie(&state);
    super::blocking(move || {
        crate::netease::search(&keyword, 30, offset.unwrap_or(0), music_u.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn netease_play(
    app: AppHandle,
    track: NeteasePlayReq,
) -> Result<(), String> {
    let task_app = app.clone();
    super::blocking(move || netease_play_task(&task_app, track)).await
}

/// 同步任务体：取链是阻塞网络调用（经 blocking 在专用线程池执行）
fn netease_play_task(app: &AppHandle, track: NeteasePlayReq) -> Result<(), String> {
    let state = app.state::<AppState>();
    let music_u = netease_cookie(&state);
    let quality = {
        let conn = state.db.lock();
        db::get_setting(&conn, "quality").unwrap_or_else(|| "high".to_string())
    };
    let (url, br, ext) = crate::netease::song_url(track.id, music_u.as_deref(), &quality)?
        .ok_or_else(|| "该歌曲暂无可播放链接（可能需要登录，或需要有效 VIP 权益）".to_string())?;
    let quality_label = quality_tag(&ext, br);
    if quality == "lossless" && !ext.eq_ignore_ascii_case("flac") {
        notify_quality_fallback(app, "网易云", &quality, &quality_label, "账号权益未含无损或该曲无更高音质");
    }
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
        qid: None,
        kgid: None,
        quality: Some(quality_label.to_string()),
    };
    let _ = app; // 事件由引擎发出
    super::record_online_play(&state, &info, "", false);
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
    let (key, qr) = super::blocking(crate::netease::qr_create).await?;
    Ok(json!({ "key": key, "qr": qr }))
}

#[tauri::command]
pub async fn netease_qr_check(
    state: State<'_, AppState>,
    key: String,
) -> Result<serde_json::Value, String> {
    let r = super::blocking(move || crate::netease::qr_check(&key)).await?;
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
    super::blocking(move || crate::netease::like_list(uid, &music_u)).await
}

#[tauri::command]
pub async fn netease_like(state: State<'_, AppState>, id: i64, like: bool) -> Result<(), String> {
    let music_u = netease_cookie(&state).ok_or("未登录网易云账号")?;
    super::blocking(move || crate::netease::like(id, like, &music_u)).await
}

#[tauri::command]
pub async fn netease_lyric(state: State<'_, AppState>, id: i64) -> Result<LyricsPayload, String> {
    let music_u = netease_cookie(&state);
    // 歌词近乎静态（含逐字/翻译）：命中缓存免全部网络往返
    let cached = {
        let conn = state.db.lock();
        db::get_lyrics_cache(&conn, "netease", &id.to_string())
    };
    if let Some(json) = cached {
        if let Ok(p) = serde_json::from_str::<LyricsPayload>(&json) {
            return Ok(p);
        }
    }
    // 优先逐字歌词（yrc）：染色推进贴合实际演唱节奏；无则回落行级。
    // 翻译（tlyric）按时间就近合并进行，两种路径都带
    let payload = super::blocking(move || -> Result<LyricsPayload, String> {
        if let Ok(Some(t)) = crate::netease::lyric_yrc(id, music_u.as_deref()) {
            let mut p = lyrics::parse(&t.lrc);
            if p.synced {
                if let Some(tr) = &t.trans {
                    lyrics::attach_translations(&mut p, tr);
                }
                let mut payload = LyricsPayload::new(true, p.lines);
                payload.source = Some("逐字 YRC".into());
                return Ok(payload);
            }
        }
        eprintln!("[netease] id {id} yrc 不可用，回落行级 LRC");
        let t = crate::netease::lyric(id, music_u.as_deref())?;
        let Some(t) = t else {
            return Ok(LyricsPayload::new(false, vec![]));
        };
        let mut p = lyrics::parse(&t.lrc);
        if let Some(tr) = &t.trans {
            lyrics::attach_translations(&mut p, tr);
        }
        let mut payload = LyricsPayload::new(p.synced, p.lines);
        if p.synced {
            payload.source = Some("行级 LRC".into());
        }
        Ok(payload)
    })
    .await?;
    if !payload.lines.is_empty() {
        // 空结果不缓存：瞬时网络失败不至于把“无歌词”固化
        if let Ok(json) = serde_json::to_string(&payload) {
            let conn = state.db.lock();
            db::set_lyrics_cache(&conn, "netease", &id.to_string(), &json);
        }
    }
    Ok(payload)
}

#[tauri::command]
pub async fn netease_logout(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    db::set_setting(&conn, "netease_music_u", "");
    db::set_setting(&conn, "netease_nickname", "");
    Ok(())
}
#[tauri::command]
pub async fn netease_toplists() -> Result<serde_json::Value, String> {
    let toplists = super::blocking(crate::netease::toplists).await?;
    Ok(json!({ "toplists": toplists }))
}

/// 榜单曲目（榜单 ID 即歌单 ID，明文 v6 匿名可拉；登录后按权益取播放链接）
#[tauri::command]
pub async fn netease_toplist_tracks(
    state: State<'_, AppState>,
    top_id: i64,
    page: Option<i64>,
) -> Result<serde_json::Value, String> {
    let music_u = netease_cookie(&state).unwrap_or_default();
    let page = page.unwrap_or(1).max(1);
    let pagesize = 100i64;
    let all = super::blocking(move || crate::netease::playlist_tracks(top_id, &music_u)).await?;
    let start = ((page - 1) * pagesize) as usize;
    let songs = if start < all.len() {
        all[start..((start + pagesize as usize).min(all.len()))].to_vec()
    } else {
        Vec::new()
    };
    Ok(json!({ "songs": songs, "hasMore": (start + songs.len()) < all.len() }))
}

#[tauri::command]
pub async fn netease_random_playlist(
    state: State<'_, AppState>,
) -> Result<crate::netease::NetRandomPlaylist, String> {
    let music_u = netease_cookie(&state).unwrap_or_default();
    super::blocking(move || crate::netease::random_playlist(&music_u)).await
}

#[tauri::command]
pub async fn netease_daily_recommend(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let music_u = netease_cookie(&state).ok_or("请先登录网易云账号")?;
    let songs = super::blocking(move || crate::netease::daily_recommend(&music_u)).await?;
    Ok(json!({ "songs": songs }))
}

#[tauri::command]
pub async fn netease_personal_fm(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let music_u = netease_cookie(&state).ok_or("请先登录网易云账号")?;
    let songs = super::blocking(move || crate::netease::personal_fm(&music_u)).await?;
    Ok(json!({ "songs": songs }))
}
#[tauri::command]
pub async fn netease_user_playlists(
    app: AppHandle,
) -> Result<Vec<crate::models::UserPlaylistMeta>, String> {
    let task_app = app.clone();
    super::blocking(move || netease_user_playlists_task(&task_app)).await
}

/// 同步任务体：uid 解析与歌单拉取都是阻塞网络调用
fn netease_user_playlists_task(
    app: &AppHandle,
) -> Result<Vec<crate::models::UserPlaylistMeta>, String> {
    let state = app.state::<AppState>();
    let (uid, music_u) = {
        let conn = state.db.lock();
        (
            db::get_setting(&conn, "netease_uid").unwrap_or_default(),
            db::get_setting(&conn, "netease_music_u").unwrap_or_default(),
        )
    };
    if music_u.is_empty() {
        return Err("未登录网易云账号".into());
    }
    let uid: i64 = if uid.is_empty() {
        let resolved = crate::netease::resolve_uid(&music_u)?;
        {
            let conn = state.db.lock();
            db::set_setting(&conn, "netease_uid", &resolved.to_string());
        }
        resolved
    } else {
        uid.parse().map_err(|_| "账号 ID 无效".to_string())?
    };
    crate::netease::user_playlists(uid, &music_u)
}

/// 导入网易云歌单：查找/合并/创建本地播放列表并写入在线条目（播放时按权益取链接）。
/// 已导入过的（按远程歌单 id 匹配，旧数据退化同名匹配）合并进已有列表：
/// 已有条目去重跳过、本地顺序与手动加的歌不动，新歌追加到末尾。
/// 返回 (本地播放列表 id, 本次实际新增条数)
#[tauri::command]
pub async fn netease_import_playlist(
    app: AppHandle,
    remote_pid: i64, // 网易云歌单 ID
    name: String,    // 歌单名（前端传入；新建/同名匹配用）
) -> Result<(i64, i64), String> {
    let task_app = app.clone();
    super::blocking(move || netease_import_playlist_task(&task_app, remote_pid, name)).await
}

/// 同步任务体：歌单拉取是阻塞网络调用（经 blocking 在专用线程池执行）
fn netease_import_playlist_task(
    app: &AppHandle,
    remote_pid: i64,
    name: String,
) -> Result<(i64, i64), String> {
    let state = app.state::<AppState>();
    let music_u = {
        let conn = state.db.lock();
        db::get_setting(&conn, "netease_music_u").unwrap_or_default()
    };
    if music_u.is_empty() {
        return Err("未登录网易云账号".into());
    }
    let songs = crate::netease::playlist_tracks(remote_pid, &music_u)?;
    let rows: Vec<super::playlists::OnlineTrackRow> = songs
        .iter()
        .map(|t| super::playlists::OnlineTrackRow {
            rid: t.id.to_string(),
            title: t.name.clone(),
            artist: t.artist_str(),
            album: t.album_name(),
            cover: t.cover_url().unwrap_or_default(),
            duration_ms: t.duration_ms(),
            media_mid: String::new(),
            vip: t.fee == 1,
        })
        .collect();
    let (list_id, added) = {
        let conn = state.db.lock();
        super::playlists::import_online_tracks(&conn, "netease", &remote_pid.to_string(), &name, &rows)?
    };
    Ok((list_id, added))
}
