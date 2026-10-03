//! QQ 音乐：搜索 / 播放取链 / 扫码登录 / 歌词 / 榜单歌单

use serde_json::json;
use tauri::{AppHandle, Manager, State};

use crate::db;
use crate::engine::TrackInfo;
use crate::lyrics;
use crate::models::LyricsPayload;
use crate::AppState;

use super::{engine_clone, notify_quality_fallback};

// ---------- QQ 音乐在线曲库 ----------

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QqPlayReq {
    pub songmid: String,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub album_mid: String,
    #[serde(default)]
    pub media_mid: String,
    #[serde(default)]
    pub duration_ms: u64,
    /// 搜索结果里的 VIP 标志，用于播放失败分类（权益不足 vs 真无版权）
    #[serde(default)]
    pub vip: bool,
}

pub(crate) fn qq_credential(state: &State<AppState>) -> Result<(String, String), String> {
    let (musicid, musickey) = {
        let conn = state.db.lock();
        (
            db::get_setting(&conn, "qq_musicid").unwrap_or_default(),
            db::get_setting(&conn, "qq_musickey").unwrap_or_default(),
        )
    };
    if musicid.is_empty() || musickey.is_empty() {
        return Err("未登录 QQ 音乐账号，无法获取播放链接，请先扫码登录".into());
    }
    Ok((musicid, musickey))
}

#[tauri::command]
pub async fn qq_search(keyword: String, page: Option<i64>) -> Result<serde_json::Value, String> {
    let songs =
        super::blocking(move || crate::qq::search(&keyword, 30, page.unwrap_or(1))).await?;
    Ok(json!({ "songs": songs }))
}

#[tauri::command]
pub async fn qq_play(app: AppHandle, track: QqPlayReq) -> Result<(), String> {
    let task_app = app.clone();
    super::blocking(move || qq_play_task(&task_app, track)).await
}

/// 同步任务体：取链是阻塞网络调用（经 blocking 在专用线程池执行）
fn qq_play_task(app: &AppHandle, track: QqPlayReq) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (musicid, musickey) = qq_credential(&state)?;
    let quality = {
        let conn = state.db.lock();
        db::get_setting(&conn, "quality").unwrap_or_else(|| "high".to_string())
    };
    let (url, ext, quality_label) = crate::qq::song_url(
        &track.songmid,
        &track.media_mid,
        &musicid,
        &musickey,
        &quality,
        track.vip,
    )?;
    if quality == "lossless" && !ext.eq_ignore_ascii_case("flac") {
        notify_quality_fallback(app, "QQ 音乐", &quality, quality_label, "账号权益未含无损或该曲无 flac");
    }
    // 封面优先用数据库存的完整 URL（歌单导入时已写入），缺失再拼 album_mid
    let cover = {
        let conn = state.db.lock();
        db::get_online_cover(&conn, "qq", &track.songmid).unwrap_or_else(|| {
            if track.album_mid.is_empty() {
                String::new()
            } else {
                format!(
                    "https://y.gtimg.cn/music/photo_new/T002R300x300M000{}.jpg",
                    track.album_mid
                )
            }
        })
    };
    let info = TrackInfo {
        id: None,
        kind: "qq".into(),
        path: String::new(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        cover: cover.clone(),
        duration_ms: track.duration_ms,
        nid: None,
        qid: Some(track.songmid.clone()),
        kgid: None,
        quality: Some(quality_label.to_string()),
    };
    // 记录到“最近播放”（在线曲目元数据轻量入库）
    super::record_online_play(&state, &info, &track.media_mid, false);
    engine_clone(&state).play_url(url, info)
}

#[tauri::command]
pub async fn qq_lyric(
    state: State<'_, AppState>,
    songmid: String,
) -> Result<LyricsPayload, String> {
    let cached = {
        let conn = state.db.lock();
        db::get_lyrics_cache(&conn, "qq", &songmid)
    };
    if let Some(json) = cached {
        if let Ok(p) = serde_json::from_str::<LyricsPayload>(&json) {
            return Ok(p);
        }
    }
    // 优先逐字歌词（QRC，需登录）；失败回落匿名行级接口
    // 凭据先取好再进阻塞线程（State 不能跨线程移动）；songmid 闭包内外各用一次，克隆入闭包
    let cred = qq_credential(&state).ok();
    let songmid_for_task = songmid.clone();
    let payload = super::blocking(move || -> Result<LyricsPayload, String> {
        let songmid = songmid_for_task;
        if let Some((musicid, musickey)) = &cred {
            if let Ok(Some(enhanced)) = crate::qq::lyric_qrc(&songmid, musicid, musickey) {
                let p = lyrics::parse(&enhanced);
                if p.synced {
                    let mut payload = LyricsPayload::new(p.synced, p.lines);
                    payload.source = Some("逐字 QRC".into());
                    return Ok(payload);
                }
            }
            eprintln!("[qq] {songmid} QRC 不可用（无权益/解密失败），回落行级 LRC");
        }
        let text = crate::qq::lyric(&songmid)?.unwrap_or_default();
        if text.is_empty() {
            return Ok(LyricsPayload::new(false, vec![]));
        }
        let p = lyrics::parse(&text);
        let mut payload = LyricsPayload::new(p.synced, p.lines);
        if p.synced {
            payload.source = Some("行级 LRC".into());
        }
        Ok(payload)
    })
    .await?;
    if !payload.lines.is_empty() {
        if let Ok(json) = serde_json::to_string(&payload) {
            let conn = state.db.lock();
            db::set_lyrics_cache(&conn, "qq", &songmid, &json);
        }
    }
    Ok(payload)
}
#[tauri::command]
pub async fn qq_qr_create() -> Result<serde_json::Value, String> {
    let (qrsig, qr) = super::blocking(crate::qq::qr_create).await?;
    Ok(json!({ "qrsig": qrsig, "qr": qr }))
}

#[tauri::command]
pub async fn qq_qr_check(
    state: State<'_, AppState>,
    qrsig: String,
) -> Result<serde_json::Value, String> {
    let r = super::blocking(move || crate::qq::qr_check(&qrsig)).await?;
    if r.status == "success" {
        if let (Some(musicid), Some(musickey)) = (&r.musicid, &r.musickey) {
            let conn = state.db.lock();
            db::set_setting(&conn, "qq_musicid", musicid);
            db::set_setting(&conn, "qq_musickey", musickey);
            // 拉取“我喜欢/收藏”夹需要加密 uin（登录响应里带，错过就没了）
            if let Some(euin) = &r.encrypt_uin {
                db::set_setting(&conn, "qq_encrypt_uin", euin);
            }
            if let Some(nick) = &r.nickname {
                db::set_setting(&conn, "qq_nickname", nick);
            }
        }
    }
    Ok(json!({ "status": r.status, "nickname": r.nickname }))
}

#[tauri::command]
pub async fn qq_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();
    let musicid = db::get_setting(&conn, "qq_musicid").unwrap_or_default();
    let nickname = db::get_setting(&conn, "qq_nickname").unwrap_or_default();
    Ok(json!({ "loggedIn": !musicid.is_empty(), "nickname": nickname }))
}

#[tauri::command]
pub async fn qq_logout(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    db::set_setting(&conn, "qq_musicid", "");
    db::set_setting(&conn, "qq_musickey", "");
    db::set_setting(&conn, "qq_encrypt_uin", "");
    db::set_setting(&conn, "qq_nickname", "");
    Ok(())
}

// ---------- 榜单 / 随机推荐（网易云、QQ，匿名可用） ----------

#[tauri::command]
pub async fn qq_toplists() -> Result<serde_json::Value, String> {
    let toplists = super::blocking(crate::qq::toplists).await?;
    Ok(json!({ "toplists": toplists }))
}

#[tauri::command]
pub async fn qq_toplist_tracks(top_id: i64, page: Option<i64>) -> Result<serde_json::Value, String> {
    let page = page.unwrap_or(1).max(1);
    let songs = super::blocking(move || crate::qq::toplist_tracks(top_id, page)).await?;
    Ok(json!({ "songs": songs, "hasMore": songs.len() as i64 >= 100 }))
}

#[tauri::command]
pub async fn qq_random_playlist() -> Result<crate::qq::QqPublicPlaylist, String> {
    super::blocking(crate::qq::random_playlist).await
}

#[tauri::command]
pub async fn qq_user_playlists(
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::UserPlaylistMeta>, String> {
    let (musicid, musickey) = qq_credential(&state)?;
    super::blocking(move || crate::qq::user_playlists(&musicid, &musickey)).await
}

/// 导入 QQ 音乐歌单：远程 dissid 拉曲目 → 查找/合并/创建本地播放列表。
/// 语义与网易云版一致：已有列表去重合并、顺序不动，新歌追加末尾。
/// 返回 (本地播放列表 id, 本次实际新增条数)
#[tauri::command]
pub async fn qq_import_playlist(
    app: AppHandle,
    remote_pid: i64,
    name: String,
) -> Result<(i64, i64), String> {
    let task_app = app.clone();
    super::blocking(move || qq_import_playlist_task(&task_app, remote_pid, name)).await
}

/// 同步任务体：歌单拉取是阻塞网络调用（经 blocking 在专用线程池执行）
fn qq_import_playlist_task(
    app: &AppHandle,
    remote_pid: i64,
    name: String,
) -> Result<(i64, i64), String> {
    let state = app.state::<AppState>();
    let (musicid, musickey) = qq_credential(&state)?;
    let stored_euin = {
        let conn = state.db.lock();
        db::get_setting(&conn, "qq_encrypt_uin").unwrap_or_default()
    };
    let songs = crate::qq::playlist_tracks(
        remote_pid,
        &musicid,
        &musickey,
        &crate::qq::encrypt_uin_of(&musicid, &stored_euin),
    )?;
    let rows: Vec<super::playlists::OnlineTrackRow> = songs
        .iter()
        .map(|t| super::playlists::OnlineTrackRow {
            rid: t.id.clone(),
            title: t.name.clone(),
            artist: t.singer.clone(),
            album: t.album.clone(),
            // 封面按 album_mid 拼 CDN 规则 URL（导入时即写入完整地址）
            cover: format!(
                "https://y.gtimg.cn/music/photo_new/T002R300x300M000{}.jpg",
                t.album_mid
            ),
            duration_ms: t.duration_ms as i64,
            media_mid: t.media_mid.clone(),
            vip: t.vip,
        })
        .collect();
    let (list_id, added) = {
        let conn = state.db.lock();
        super::playlists::import_online_tracks(&conn, "qq", &remote_pid.to_string(), &name, &rows)?
    };
    Ok((list_id, added))
}
