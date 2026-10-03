//! 酷狗音乐：搜索 / 播放取链 / 扫码登录 / 歌词 / 榜单歌单广场

use serde_json::json;
use tauri::{AppHandle, State};

use crate::db;
use crate::engine::TrackInfo;
use crate::lyrics;
use crate::models::LyricsPayload;
use crate::AppState;

use super::{engine_clone, notify_quality_fallback};
/// 酷狗存档行（最近播放/收藏/歌单）只落库了 128 hash：按标题反查各档
/// 质量 hash 补齐，否则 HQ/无损音质会静默塌缩到标准
pub(crate) fn kugou_hashes_enriched(
    hash: &str,
    title: &str,
    artist: &str,
    hq_hash: &str,
    sq_hash: &str,
    super_hash: &str,
) -> (String, String, String) {
    crate::kugou::enrich_hashes(hash, title, artist, hq_hash, sq_hash, super_hash)
}
// ---------- 酷狗音乐在线曲库（搜索/播放匿名；VIP 曲目需扫码登录） ----------

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KgPlayReq {
    pub hash: String,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub cover: String,
    #[serde(default)]
    pub duration_ms: u64,
    /// 搜索结果里的付费标志，用于播放失败分类
    #[serde(default)]
    pub vip: bool,
    /// 专辑音频 ID / 专辑 ID / 各音质 hash：登录后按音质取链接用，
    /// 旧收藏条目可缺省（serde default）
    #[serde(default)]
    pub album_audio_id: u64,
    #[serde(default)]
    pub album_id: u64,
    #[serde(default)]
    pub hq_hash: String,
    #[serde(default)]
    pub sq_hash: String,
    #[serde(default)]
    pub super_hash: String,
}

/// 酷狗登录凭证（token + userid），未登录为空串
fn kg_credential(state: &State<AppState>) -> (String, String) {
    let conn = state.db.lock();
    (
        db::get_setting(&conn, "kg_token").unwrap_or_default(),
        db::get_setting(&conn, "kg_userid").unwrap_or_default(),
    )
}

/// 调用酷狗接口前的统一准备：注入登录账号与持久化的注册 dfid。
/// dfid 缺失时（首次）触发设备注册并持久化——匿名随机 dfid 会被风控拦截。
pub(crate) fn kg_prepare(state: &State<AppState>) -> (String, String) {
    let (token, userid) = kg_credential(state);
    crate::kugou::set_account(&token, &userid);
    // mid 与 dfid 服务端绑定校验，必须成对持久化、成对注入
    let (mid, dfid) = {
        let conn = state.db.lock();
        (
            db::get_setting(&conn, "kg_mid").unwrap_or_default(),
            db::get_setting(&conn, "kg_dfid").unwrap_or_default(),
        )
    };
    if mid.is_empty() || dfid.is_empty() {
        // 进程内可能已有可用 pair（gateway 请求路径触发过注册）：同步落库
        let cached = crate::kugou::current_dfid();
        if !cached.is_empty() {
            let m = crate::kugou::current_mid();
            let conn = state.db.lock();
            db::set_setting(&conn, "kg_mid", &m);
            db::set_setting(&conn, "kg_dfid", &cached);
        } else {
            match crate::kugou::register_dev() {
                Ok(d) => {
                    let m = crate::kugou::current_mid();
                    let conn = state.db.lock();
                    db::set_setting(&conn, "kg_mid", &m);
                    db::set_setting(&conn, "kg_dfid", &d);
                }
                Err(e) => eprintln!("[kugou] register_dev 失败: {e}"),
            }
        }
    } else {
        crate::kugou::set_device(&mid, &dfid);
    }
    (token, userid)
}

#[tauri::command]
pub async fn kugou_search(keyword: String, page: Option<i64>) -> Result<serde_json::Value, String> {
    let songs = crate::kugou::search(&keyword, page.unwrap_or(1))?;
    Ok(json!({ "songs": songs }))
}

#[tauri::command]
pub async fn kugou_play(app: AppHandle, state: State<'_, AppState>, track: KgPlayReq) -> Result<(), String> {
    let quality = {
        let conn = state.db.lock();
        db::get_setting(&conn, "quality").unwrap_or_else(|| "high".to_string())
    };
    let (token, userid) = kg_prepare(&state);
    // 存档行（最近播放/收藏/歌单）只落库了 128 hash：按标题反查补齐各档
    // hash，否则 HQ/无损音质设置会静默塌缩到标准
    let (hq_hash, sq_hash, super_hash) = kugou_hashes_enriched(
        &track.hash,
        &track.title,
        &track.artist,
        &track.hq_hash,
        &track.sq_hash,
        &track.super_hash,
    );
    let (url, _ext, quality_label) = crate::kugou::song_url(
        &track.hash,
        track.album_audio_id,
        track.album_id,
        &hq_hash,
        &sq_hash,
        &super_hash,
        track.vip,
        &token,
        &userid,
        &quality,
    )?;
    notify_quality_fallback(
        &app,
        "酷狗",
        &quality,
        &quality_label,
        if token.is_empty() {
            "酷狗未登录，匿名通道仅提供标准音质"
        } else {
            "账号权益或接口风控限制了更高音质"
        },
    );
    // 封面：数据库存的完整 URL 优先（收藏/最近播放已入库），缺失用搜索带的
    let cover = {
        let conn = state.db.lock();
        db::get_online_cover(&conn, "kugou", &track.hash).unwrap_or_default()
    };
    let cover = if cover.is_empty() {
        track.cover.clone()
    } else {
        cover
    };
    let info = TrackInfo {
        id: None,
        kind: "kugou".into(),
        path: String::new(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        cover: cover.clone(),
        duration_ms: track.duration_ms,
        nid: None,
        qid: None,
        kgid: Some(track.hash.clone()),
        quality: Some(quality_label.to_string()),
    };
    // 记录到“最近播放”（在线曲目元数据轻量入库；album_audio_id 存入
    // media_mid 列，恢复播放时能带全取链接参数）
    {
        let conn = state.db.lock();
        db::record_play_online(
            &conn,
            "kugou",
            &track.hash,
            &track.title,
            &track.artist,
            &track.album,
            &cover,
            track.duration_ms as i64,
            &track.album_audio_id.to_string(),
            track.vip,
        );
    }
    engine_clone(&state).play_url(url, info)
}

#[tauri::command]
pub async fn kugou_lyric(
    state: State<'_, AppState>,
    hash: String,
) -> Result<LyricsPayload, String> {
    // 歌词近乎静态（含逐字）：命中缓存免全部网络往返
    let cached = {
        let conn = state.db.lock();
        db::get_lyrics_cache(&conn, "kugou", &hash)
    };
    if let Some(json) = cached {
        if let Ok(p) = serde_json::from_str::<LyricsPayload>(&json) {
            return Ok(p);
        }
    }
    let text = crate::kugou::lyric(&hash)?;
    let text = match text {
        Some(t) if !t.is_empty() => t,
        _ => {
            return Ok(LyricsPayload::new(false, vec![]));
        }
    };
    let p = lyrics::parse(&text);
    // KRC 通道成功与否由 kugou::lyric 内部落 stderr 日志；来源按解析结果区分
    let word_level = p.lines.iter().any(|l| l.words.is_some());
    let mut payload = LyricsPayload::new(p.synced, p.lines);
    if p.synced {
        payload.source = Some(if word_level { "逐字 KRC" } else { "行级 LRC" }.into());
    }
    if !payload.lines.is_empty() {
        if let Ok(json) = serde_json::to_string(&payload) {
            let conn = state.db.lock();
            db::set_lyrics_cache(&conn, "kugou", &hash, &json);
        }
    }
    Ok(payload)
}

// ---------- 酷狗扫码登录 ----------

#[tauri::command]
pub async fn kugou_qr_create() -> Result<serde_json::Value, String> {
    let (key, qr) = crate::kugou::qr_create()?;
    Ok(json!({ "key": key, "qr": qr }))
}

#[tauri::command]
pub async fn kugou_qr_check(
    state: State<'_, AppState>,
    key: String,
) -> Result<serde_json::Value, String> {
    let r = crate::kugou::qr_check(&key)?;
    if r.status == "success" {
        if let (Some(token), Some(userid)) = (&r.token, &r.userid) {
            crate::kugou::set_account(token, userid);
            let conn = state.db.lock();
            db::set_setting(&conn, "kg_token", token);
            db::set_setting(&conn, "kg_userid", userid);
            if let Some(nick) = &r.nickname {
                db::set_setting(&conn, "kg_nickname", nick);
            }
            if let Some(vt) = r.vip_type {
                db::set_setting(&conn, "kg_vip_type", &vt.to_string());
            }
        }
    }
    Ok(json!({ "status": r.status, "nickname": r.nickname }))
}

#[tauri::command]
pub async fn kugou_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();
    let token = db::get_setting(&conn, "kg_token").unwrap_or_default();
    let nickname = db::get_setting(&conn, "kg_nickname").unwrap_or_default();
    Ok(json!({ "loggedIn": !token.is_empty(), "nickname": nickname }))
}

#[tauri::command]
pub async fn kugou_logout(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    db::set_setting(&conn, "kg_token", "");
    db::set_setting(&conn, "kg_userid", "");
    db::set_setting(&conn, "kg_nickname", "");
    db::set_setting(&conn, "kg_vip_type", "");
    Ok(())
}

// ---------- 酷狗榜单 / 歌单广场（匿名） ----------

#[tauri::command]
pub async fn kugou_toplists() -> Result<serde_json::Value, String> {
    let toplists = crate::kugou::toplists()?;
    Ok(json!({ "toplists": toplists }))
}

#[tauri::command]
pub async fn kugou_toplist_tracks(
    state: State<'_, AppState>,
    top_id: i64,
    page: Option<i64>,
) -> Result<serde_json::Value, String> {
    let _ = kg_prepare(&state);
    let page = page.unwrap_or(1).max(1);
    let songs = crate::kugou::toplist_tracks(top_id, page)?;
    Ok(json!({ "songs": songs, "hasMore": songs.len() as i64 >= 90 }))
}

#[tauri::command]
pub async fn kugou_random_playlist(
    state: State<'_, AppState>,
) -> Result<crate::kugou::KgPublicPlaylist, String> {
    let _ = kg_prepare(&state);
    crate::kugou::random_playlist()
}

#[tauri::command]
pub async fn kugou_playlist_tracks(
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::kugou::KgPublicPlaylist, String> {
    let _ = kg_prepare(&state);
    crate::kugou::playlist_tracks(&id)
}

/// 酷狗账号下的自建/收藏歌单（需扫码登录）
#[tauri::command]
pub async fn kugou_user_playlists(
    state: State<'_, AppState>,
) -> Result<Vec<crate::kugou::KgUserPlaylist>, String> {
    let (token, userid) = kg_prepare(&state);
    crate::kugou::user_playlists(&token, &userid)
}

/// 酷狗公开歌单导入为本地播放列表（匿名可拉，无需登录）
#[tauri::command]
pub async fn kugou_import_playlist(
    state: State<'_, AppState>,
    remote_pid: String,
    name: String,
) -> Result<(i64, i64), String> {
    let _ = kg_prepare(&state);
    let songs = crate::kugou::playlist_tracks(&remote_pid)?;
    let (list_id, added) = {
        let conn = state.db.lock();
        let pid = match db::find_playlist_by_remote(&conn, "kugou", &remote_pid, &name) {
            Some(id) => id,
            None => db::create_playlist(&conn, &name)?,
        };
        db::set_playlist_remote(&conn, pid, "kugou", &remote_pid);
        db::set_playlist_origin(&conn, pid, &name);
        let mut added = 0i64;
        for t in &songs.songs {
            db::upsert_online_track(
                &conn,
                "kugou",
                &t.id,
                &t.name,
                &t.singer,
                &t.album,
                &t.cover,
                t.duration_ms as i64,
                &t.album_audio_id.to_string(),
                t.vip,
            );
            if db::add_online_to_playlist(&conn, pid, "kugou", &t.id)? {
                added += 1;
            }
        }
        (pid, added)
    };
    Ok((list_id, added))
}
