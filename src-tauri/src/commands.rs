use std::io::{Read, Write};
use serde_json::json;
use lofty::prelude::*;
use tauri::{AppHandle, Emitter, Manager, State};

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
    // 返回全量记录（含 missing 软删除），由前端按视图过滤：
    // 资料库隐藏 missing，“我喜欢/最近播放”保留记录（文件没了也显示，仅是引用）
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

// ---------- 输出设备 ----------

/// 枚举输出设备 + 当前生效的设备名
#[tauri::command]
pub async fn list_output_devices(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let engine = engine_clone(&state);
    let devices = engine.list_output_devices();
    Ok(json!({
        "devices": devices,
        "current": engine.current_device_name(),
        "preference": engine.device_preference(),
    }))
}

/// 切换输出设备；name 为空 = 跟随系统默认（并持久化偏好）
#[tauri::command]
pub async fn set_output_device(
    state: State<'_, AppState>,
    name: Option<String>,
) -> Result<(), String> {
    let engine = engine_clone(&state);
    let pref = name.as_deref().filter(|s| !s.is_empty());
    engine.switch_output_device(pref)?;
    let conn = state.db.lock();
    db::set_setting(
        &conn,
        "output_device",
        pref.unwrap_or(""),
    );
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
    let local_quality = if meta.bit_depth >= 16 && meta.sample_rate >= 44100 {
        format!(
            "{}kHz/{}bit",
            meta.sample_rate / 1000,
            meta.bit_depth.max(16)
        )
    } else if meta.bitrate > 0 {
        format!("{}kbps", meta.bitrate / 1000)
    } else {
        String::new()
    };
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
        qid: None,
        quality: (!local_quality.is_empty()).then_some(local_quality),
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
        qid: None,
        quality: None,
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
    offset: Option<i64>,
) -> Result<crate::netease::NetSearchResult, String> {
    let music_u = netease_cookie(&state);
    crate::netease::search(&keyword, 30, offset.unwrap_or(0), music_u.as_deref())
}

#[tauri::command]
pub async fn netease_play(
    app: AppHandle,
    state: State<'_, AppState>,
    track: NeteasePlayReq,
) -> Result<(), String> {
    let music_u = netease_cookie(&state);
    let quality = {
        let conn = state.db.lock();
        db::get_setting(&conn, "quality").unwrap_or_else(|| "high".to_string())
    };
    let (url, br, ext) = crate::netease::song_url(track.id, music_u.as_deref(), &quality)?
        .ok_or_else(|| {
            "该歌曲暂无可播放链接（可能需要登录，或需要有效 VIP 权益）".to_string()
        })?;
    let quality_label = if ext.eq_ignore_ascii_case("flac") {
        "FLAC".to_string()
    } else {
        format!("{br}kbps")
    };
    // 记录到“最近播放”（在线曲目元数据轻量入库）
    {
        let conn = state.db.lock();
        db::record_play_online(
            &conn,
            "netease",
            &track.id.to_string(),
            &track.title,
            &track.artist,
            &track.album,
            &track.cover,
            track.duration_ms as i64,
            "",
            false,
        );
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
        quality: Some(quality_label),
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
    // 优先逐字歌词（yrc）：染色推进贴合实际演唱节奏；无则回落行级
    if let Ok(Some(enhanced)) = crate::netease::lyric_yrc(id, music_u.as_deref()) {
        let p = lyrics::parse(&enhanced);
        if p.synced {
            return Ok(LyricsPayload { synced: p.synced, lines: p.lines });
        }
    }
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

fn qq_credential(state: &State<AppState>) -> Result<(String, String), String> {
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
pub async fn qq_search(
    keyword: String,
    page: Option<i64>,
) -> Result<serde_json::Value, String> {
    let songs = crate::qq::search(&keyword, 30, page.unwrap_or(1))?;
    Ok(json!({ "songs": songs }))
}

#[tauri::command]
pub async fn qq_play(
    state: State<'_, AppState>,
    track: QqPlayReq,
) -> Result<(), String> {
    let (musicid, musickey) = qq_credential(&state)?;
    let quality = {
        let conn = state.db.lock();
        db::get_setting(&conn, "quality").unwrap_or_else(|| "high".to_string())
    };
    let (url, ext) =
        crate::qq::song_url(&track.songmid, &track.media_mid, &musicid, &musickey, &quality, track.vip)?;
    let quality_label = if ext.eq_ignore_ascii_case("flac") {
        "FLAC".to_string()
    } else if ext.eq_ignore_ascii_case("mp3") && quality == "standard" {
        "128kbps".to_string()
    } else if ext.eq_ignore_ascii_case("mp3") {
        "320kbps".to_string()
    } else {
        ext.to_uppercase()
    };
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
        quality: Some(quality_label),
    };
    // 记录到“最近播放”（在线曲目元数据轻量入库）
    {
        let conn = state.db.lock();
        db::record_play_online(
            &conn,
            "qq",
            &track.songmid,
            &track.title,
            &track.artist,
            &track.album,
            &cover,
            track.duration_ms as i64,
            &track.media_mid,
            false,
        );
    }
    engine_clone(&state).play_url(url, info)
}

#[tauri::command]
pub async fn qq_lyric(state: State<'_, AppState>, songmid: String) -> Result<LyricsPayload, String> {
    // 优先逐字歌词（QRC，需登录）；失败回落匿名行级接口
    if let Ok((musicid, musickey)) = qq_credential(&state) {
        if let Ok(Some(enhanced)) = crate::qq::lyric_qrc(&songmid, &musicid, &musickey) {
            let p = lyrics::parse(&enhanced);
            if p.synced {
                return Ok(LyricsPayload { synced: p.synced, lines: p.lines });
            }
        }
    }
    let text = crate::qq::lyric(&songmid)?.unwrap_or_default();
    if text.is_empty() {
        return Ok(LyricsPayload { synced: false, lines: vec![] });
    }
    let p = lyrics::parse(&text);
    Ok(LyricsPayload { synced: p.synced, lines: p.lines })
}

#[tauri::command]
pub async fn qq_qr_create() -> Result<serde_json::Value, String> {
    let (qrsig, qr) = crate::qq::qr_create()?;
    Ok(json!({ "qrsig": qrsig, "qr": qr }))
}

#[tauri::command]
pub async fn qq_qr_check(
    state: State<'_, AppState>,
    qrsig: String,
) -> Result<serde_json::Value, String> {
    let r = crate::qq::qr_check(&qrsig)?;
    if r.status == "success" {
        if let (Some(musicid), Some(musickey)) = (&r.musicid, &r.musickey) {
            let conn = state.db.lock();
            db::set_setting(&conn, "qq_musicid", musicid);
            db::set_setting(&conn, "qq_musickey", musickey);
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
    db::set_setting(&conn, "qq_nickname", "");
    Ok(())
}

// ---------- 在线歌曲收藏到本地 / 歌单导入 ----------

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineSaveReq {
    pub kind: String, // netease | qq
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub cover_url: String,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub media_mid: String,
}

fn save_dir(state: &State<AppState>) -> std::path::PathBuf {
    let conn = state.db.lock();
    let custom = db::get_setting(&conn, "save_dir").unwrap_or_default();
    if custom.is_empty() {
        state.app_data.join("下载音乐")
    } else {
        std::path::PathBuf::from(custom)
    }
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// 收藏在线歌曲到“我喜欢”（轻量引用，不下载；播放时按权益取链接）
#[tauri::command]
pub async fn like_online(
    state: State<'_, AppState>,
    kind: String,
    rid: String,
    title: String,
    artist: Option<String>,
    album: Option<String>,
    cover: Option<String>,
    duration_ms: Option<i64>,
    media_mid: Option<String>,
    vip: Option<bool>,
    like: Option<bool>,
) -> Result<(), String> {
    let conn = state.db.lock();
    let like = like.unwrap_or(true);
    if like {
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
        db::like_online_track(&conn, &kind, &rid);
    } else {
        db::unlike_online_track(&conn, &kind, &rid);
    }
    Ok(())
}

/// 下载在线歌曲到保存目录（写标签入库，资料库可见）
#[tauri::command]
pub async fn download_online(
    app: AppHandle,
    state: State<'_, AppState>,
    req: OnlineSaveReq,
) -> Result<String, String> {
    let title = req.title.trim().to_string();
    if title.is_empty() {
        return Err("歌曲标题为空".into());
    }

    // 1) 按当前音质取播放链接
    let quality = {
        let conn = state.db.lock();
        db::get_setting(&conn, "quality").unwrap_or_else(|| "high".to_string())
    };
    let (url, ext) = match req.kind.as_str() {
        "netease" => {
            let id: i64 = req.id.parse().map_err(|_| "网易云歌曲 ID 无效".to_string())?;
            let music_u = netease_cookie(&state);
            let (u, _br, ext) = crate::netease::song_url(id, music_u.as_deref(), &quality)?
                .ok_or("该歌曲暂无可播放链接")?;
            (u, ext)
        }
        "qq" => {
            let (musicid, musickey) = qq_credential(&state)?;
            let (u, ext) = crate::qq::song_url(
                &req.id,
                &req.media_mid,
                &musicid,
                &musickey,
                &quality,
                true,
            )?;
            (u, ext)
        }
        _ => return Err("未知音源类型".into()),
    };

    // 2) 下载到保存目录
    let dir = save_dir(&state);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建保存目录失败: {e}"))?;
    let artist = sanitize_filename(&req.artist);
    let name = format!(
        "{} - {}.{}",
        if artist.is_empty() { "Unknown" } else { &artist },
        sanitize_filename(&title),
        ext
    );
    let dest = dir.join(&name);
    let mut file = std::fs::File::create(&dest).map_err(|e| format!("创建文件失败: {e}"))?;
    let (total, reader) = http_get_for(&req.kind, &url)?;
    let mut reader = reader.take(128 * 1024 * 1024);
    let mut buf = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let mut emitted = false;
    // 分块读取并回报进度（download://progress 驱动进度条；无 content-length 时 pct 为 0）
    let dl = (|| -> Result<(), String> {
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("下载失败: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| format!("写入文件失败: {e}"))?;
            received += n as u64;
            if last_emit.elapsed() >= std::time::Duration::from_millis(300) {
                last_emit = std::time::Instant::now();
                emitted = true;
                let pct = if total > 0 {
                    ((received as f64 / total as f64) * 100.0) as u64
                } else {
                    0
                };
                let _ = app.emit(
                    "download://progress",
                    json!({ "url": &title, "received": received, "total": total, "pct": pct.min(99), "done": false }),
                );
            }
        }
        Ok(())
    })();
    drop(file);
    if let Err(e) = dl {
        // 已发过进度则先清掉进度条；错误提示由命令返回值统一 toast，避免重复弹窗
        if emitted {
            let _ = app.emit("download://progress", json!({ "url": &title, "done": true }));
        }
        return Err(e);
    }
    let _ = app.emit(
        "download://progress",
        json!({ "url": &title, "received": received, "total": if total == 0 { received } else { total }, "pct": 100, "done": true }),
    );

    // 3) 取歌词并写标签（失败静默）
    let lyrics = match req.kind.as_str() {
        "netease" => crate::netease::lyric(
            req.id.parse::<i64>().unwrap_or(0),
            netease_cookie(&state).as_deref(),
        )
        .ok()
        .flatten(),
        "qq" => crate::qq::lyric(&req.id).ok().flatten(),
        _ => None,
    };
    write_tags(&dest, &title, &req.artist, &req.album, &req.cover_url, lyrics.as_deref());

    // 4) 解析入库 + 标记已下载 + 保存目录纳入扫描
    let mut track = crate::library::parse_track(&dest, &state.app_data).ok_or("解析歌曲失败")?;
    // 文件标签缺失时长时，用前端传入的元数据时长兜底
    if track.duration == 0.0 && req.duration_ms > 0 {
        track.duration = req.duration_ms as f64 / 1000.0;
    }
    {
        let conn = state.db.lock();
        db::upsert_track(&conn, &track);
        db::mark_online_downloaded(&conn, &req.kind, &req.id);
        let _ = db::add_folder(&conn, &dir.to_string_lossy());
    }
    Ok(name)
}

/// “我喜欢”列表：在线条目部分
#[tauri::command]
pub async fn liked_online_list(
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::PlaylistEntryMeta>, String> {
    let conn = state.db.lock();
    Ok(db::liked_online_list(&conn)
        .into_iter()
        .map(|e| crate::models::PlaylistEntryMeta {
            rowid: 0,
            kind: e.kind,
            track_id: None,
            online_id: Some(e.online_id),
            title: e.title,
            artist: e.artist,
            album: e.album,
            cover: e.cover,
            duration: e.duration,
            media_mid: e.media_mid,
            vip: e.vip,
            last_played: 0,
            liked_at: e.liked_at,
        })
        .collect())
}

/// “最近播放”的在线曲目部分（最近播放过的，按时间倒序）
#[tauri::command]
pub async fn recent_online_list(
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::PlaylistEntryMeta>, String> {
    let conn = state.db.lock();
    Ok(db::recent_online_list(&conn, 100)
        .into_iter()
        .map(|e| crate::models::PlaylistEntryMeta {
            rowid: 0,
            kind: e.kind,
            track_id: None,
            online_id: Some(e.online_id),
            title: e.title,
            artist: e.artist,
            album: e.album,
            cover: e.cover,
            duration: e.duration,
            media_mid: e.media_mid,
            vip: e.vip,
            last_played: e.last_played,
            liked_at: 0,
        })
        .collect())
}

/// 获取下载保存目录（custom 为空时用 default）
#[tauri::command]
pub async fn save_dir_get(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();
    let custom = db::get_setting(&conn, "save_dir").unwrap_or_default();
    Ok(json!({
        "dir": custom,
        "default": state.app_data.join("下载音乐").to_string_lossy(),
    }))
}

#[tauri::command]
pub async fn save_dir_set(state: State<'_, AppState>, dir: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&dir);
    if !p.is_dir() {
        return Err("该路径不是文件夹".into());
    }
    let conn = state.db.lock();
    db::set_setting(&conn, "save_dir", &dir);
    Ok(())
}

fn http_get_for(kind: &str, url: &str) -> Result<(u64, impl std::io::Read), String> {
    // 连接与读取分段超时：整体超时会在大文件下载中途掐断连接
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30));
    if kind == "qq" {
        if let Some(p) = crate::qq::system_proxy() {
            builder = builder.proxy(p);
        }
    }
    let agent = builder.build();
    let mut req = agent.get(url).set("User-Agent", "Mozilla/5.0");
    if kind == "qq" {
        req = req
            .set("Referer", "https://y.qq.com/")
            .set("User-Agent", crate::qq::UA);
    }
    let resp = req.call().map_err(|e| format!("下载失败: {e}"))?;
    let total: u64 = resp
        .header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    Ok((total, resp.into_reader()))
}

/// 给下载的音频写标签（标题/艺术家/专辑/封面/歌词）
fn write_tags(
    path: &std::path::Path,
    title: &str,
    artist: &str,
    album: &str,
    cover_url: &str,
    lyrics: Option<&str>,
) {
    let _ = (|| -> Result<(), String> {
        use lofty::prelude::*;
        let mut tagged = lofty::read_from_path(path).map_err(|e| e.to_string())?;
        let tag_type = tagged.file_type().primary_tag_type();
        {
            let tag = match tagged.primary_tag_mut() {
                Some(t) => t,
                None => {
                    tagged.insert_tag(lofty::tag::Tag::new(tag_type));
                    tagged.primary_tag_mut().ok_or("无主标签")?
                }
            };
            tag.insert_text(lofty::tag::ItemKey::TrackTitle, title.to_string());
            if !artist.is_empty() {
                tag.insert_text(lofty::tag::ItemKey::TrackArtist, artist.to_string());
            }
            if !album.is_empty() {
                tag.insert_text(lofty::tag::ItemKey::AlbumTitle, album.to_string());
            }
            if let Some(lrc) = lyrics.filter(|l| !l.trim().is_empty()) {
                tag.insert_text(lofty::tag::ItemKey::Lyrics, lrc.to_string());
            }
            if !cover_url.is_empty() {
                if let Ok(resp) = ureq::get(cover_url)
                    .set("User-Agent", "Mozilla/5.0")
                    .timeout(std::time::Duration::from_secs(15))
                    .call()
                {
                    let mut data = Vec::new();
                    if resp.into_reader().read_to_end(&mut data).is_ok() && !data.is_empty() {
                        let mime = if cover_url.contains(".png") {
                            lofty::picture::MimeType::Png
                        } else {
                            lofty::picture::MimeType::Jpeg
                        };
                        let pic = lofty::picture::Picture::new_unchecked(
                            lofty::picture::PictureType::CoverFront,
                            Some(mime),
                            None,
                            data,
                        );
                        tag.push_picture(pic);
                    }
                }
            }
        }
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        tagged
            .save_to(&mut file, lofty::config::WriteOptions::default())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
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

#[tauri::command]
pub async fn netease_user_playlists(
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::UserPlaylistMeta>, String> {
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
    state: State<'_, AppState>,
    remote_pid: i64, // 网易云歌单 ID
    name: String, // 歌单名（前端传入；新建/同名匹配用）
) -> Result<(i64, i64), String> {
    let music_u = {
        let conn = state.db.lock();
        db::get_setting(&conn, "netease_music_u").unwrap_or_default()
    };
    if music_u.is_empty() {
        return Err("未登录网易云账号".into());
    }
    let songs = crate::netease::playlist_tracks(remote_pid, &music_u)?;
    let (list_id, added) = {
        let conn = state.db.lock();
        let pid = match db::find_playlist_by_remote(
            &conn,
            "netease",
            &remote_pid.to_string(),
            &name,
        ) {
            Some(id) => id,
            None => db::create_playlist(&conn, &name)?,
        };
        db::set_playlist_remote(&conn, pid, "netease", &remote_pid.to_string());
        let mut added = 0i64;
        for t in &songs {
            db::upsert_online_track(
                &conn,
                "netease",
                &t.id.to_string(),
                &t.name,
                &t.artist_str(),
                &t.album_name(),
                &t.cover_url().unwrap_or_default(),
                t.duration_ms(),
                "",
                t.fee == 1,
            );
            if db::add_online_to_playlist(&conn, pid, "netease", &t.id.to_string())? {
                added += 1;
            }
        }
        (pid, added)
    };
    Ok((list_id, added))
}

#[tauri::command]
pub async fn qq_user_playlists(
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::UserPlaylistMeta>, String> {
    let (musicid, musickey) = qq_credential(&state)?;
    crate::qq::user_playlists(&musicid, &musickey)
}

/// 导入 QQ 音乐歌单：远程 dissid 拉曲目 → 查找/合并/创建本地播放列表。
/// 语义与网易云版一致：已有列表去重合并、顺序不动，新歌追加末尾。
/// 返回 (本地播放列表 id, 本次实际新增条数)
#[tauri::command]
pub async fn qq_import_playlist(
    state: State<'_, AppState>,
    remote_pid: i64,
    name: String,
) -> Result<(i64, i64), String> {
    let (musicid, musickey) = qq_credential(&state)?;
    let songs = crate::qq::playlist_tracks(remote_pid, &musicid, &musickey)?;
    let (list_id, added) = {
        let conn = state.db.lock();
        let pid = match db::find_playlist_by_remote(
            &conn,
            "qq",
            &remote_pid.to_string(),
            &name,
        ) {
            Some(id) => id,
            None => db::create_playlist(&conn, &name)?,
        };
        db::set_playlist_remote(&conn, pid, "qq", &remote_pid.to_string());
        let mut added = 0i64;
        for t in &songs {
            db::upsert_online_track(
                &conn,
                "qq",
                &t.id,
                &t.name,
                &t.singer,
                &t.album,
                &format!(
                    "https://y.gtimg.cn/music/photo_new/T002R300x300M000{}.jpg",
                    t.album_mid
                ),
                t.duration_ms as i64,
                &t.media_mid,
                t.vip,
            );
            if db::add_online_to_playlist(&conn, pid, "qq", &t.id)? {
                added += 1;
            }
        }
        (pid, added)
    };
    Ok((list_id, added))
}

#[tauri::command]
pub async fn set_play_quality(state: State<'_, AppState>, quality: String) -> Result<(), String> {
    if !matches!(quality.as_str(), "standard" | "high" | "lossless") {
        return Err("无效的音质选项".into());
    }
    let conn = state.db.lock();
    db::set_setting(&conn, "quality", &quality);
    Ok(())
}

/// 关闭主窗口行为：tray = 最小化到托盘（默认）；exit = 直接退出应用
#[tauri::command]
pub async fn set_close_action(state: State<'_, AppState>, action: String) -> Result<(), String> {
    if !matches!(action.as_str(), "tray" | "exit") {
        return Err("无效的关闭行为".into());
    }
    let conn = state.db.lock();
    db::set_setting(&conn, "close_action", &action);
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
    let quality = db::get_setting(&conn, "quality").unwrap_or_else(|| "high".to_string());
    let cache_limit: u64 = db::get_setting(&conn, "cache_limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(2 * 1024 * 1024 * 1024);
    let close_action =
        db::get_setting(&conn, "close_action").unwrap_or_else(|| "tray".to_string());
    Ok(SettingsPayload {
        volume,
        speed,
        eq_gains,
        eq_enabled,
        quality,
        cache_limit,
        close_action,
    })
}

// ---------- 其他 ----------

#[tauri::command]
pub async fn clear_cache(state: State<'_, AppState>) -> Result<u32, String> {
    Ok(engine_clone(&state).clear_cache())
}

/// 当前缓存占用（bytes）与文件数
#[tauri::command]
pub async fn cache_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (bytes, files) = engine_clone(&state).cache_usage();
    Ok(json!({ "bytes": bytes, "files": files }))
}

/// 设置缓存上限（bytes，0 = 不限制）；超限立即 LRU 清理
#[tauri::command]
pub async fn set_cache_limit(
    state: State<'_, AppState>,
    bytes: u64,
) -> Result<(), String> {
    {
        let conn = state.db.lock();
        db::set_setting(&conn, "cache_limit", &bytes.to_string());
    }
    engine_clone(&state).set_cache_limit(bytes);
    Ok(())
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

// ---------- 封面取色（服务端，绕过 QQ 封面域无 CORS 的限制） ----------

#[tauri::command]
pub async fn extract_cover_palette(
    url: String,
) -> Result<Vec<String>, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Ok(vec![]);
    }
    let bytes = match ureq::get(&url)
        .set("User-Agent", "Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(10))
        .call()
    {
        Ok(resp) => {
            let mut buf = Vec::new();
            resp.into_reader()
                .take(2 * 1024 * 1024)
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            buf
        }
        Err(_) => return Ok(vec![]),
    };
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let rgb = img.thumbnail(36, 36).to_rgb8();
    // 色相分桶（与前端算法一致），输出 hsl
    let mut buckets: std::collections::BTreeMap<i64, (f64, f64, f64, f64)> =
        std::collections::BTreeMap::new();
    for p in rgb.pixels() {
        let (r, g, b) = (p[0] as f64 / 255.0, p[1] as f64 / 255.0, p[2] as f64 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lum = r * 0.3 + g * 0.6 + b * 0.1;
        if lum < 0.06 || lum > 0.97 {
            continue;
        }
        let sat = if max == 0.0 { 0.0 } else { (max - min) / max };
        let d = max - min;
        let mut hue = 0.0;
        if d != 0.0 {
            if max == r {
                hue = ((g - b) / d) % 6.0;
            } else if max == g {
                hue = (b - r) / d + 2.0;
            } else {
                hue = (r - g) / d + 4.0;
            }
            hue *= 60.0;
            if hue < 0.0 {
                hue += 360.0;
            }
        }
        let bucket = (hue / 45.0).floor() as i64;
        let w = 0.4 + sat;
        let e = buckets.entry(bucket).or_insert((0.0, 0.0, 0.0, 0.0));
        e.0 += r * w;
        e.1 += g * w;
        e.2 += b * w;
        e.3 += w;
    }
    let mut colors: Vec<(f64, f64, f64, f64)> = buckets.into_values().collect();
    colors.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    for (r, g, b, _) in colors.iter().take(4) {
        let (r, g, b) = (*r, *g, *b);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let d = max - min;
        let mut h = 0.0;
        let mut s = 0.0;
        if d != 0.0 {
            s = d / (1.0 - (2.0 * l - 1.0).abs());
            if max == r {
                h = ((g - b) / d) % 6.0;
            } else if max == g {
                h = (b - r) / d + 2.0;
            } else {
                h = (r - g) / d + 4.0;
            }
            h *= 60.0;
            if h < 0.0 {
                h += 360.0;
            }
        }
        let s2 = (s.max(0.55) * 1.1).min(1.0);
        let l_out = (l.max(0.66)).min(0.85);
        out.push(format!(
            "hsl({}, {:.0}%, {:.0}%)",
            h.round() as i64,
            s2 * 100.0,
            l_out * 100.0
        ));
    }
    Ok(out)
}

// ---------- 桌面歌词窗口 ----------

/// 打开桌面歌词窗口（透明、无边框、置顶、跳过任务栏）。
/// 已存在则仅显示与聚焦。窗口加载 desktop-lyrics.html（dev 下走 Vite 端口）。
#[tauri::command]
pub async fn desktop_lyrics_open(app: AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        let _ = w.show();
        return Ok(());
    }
    let url = {
        // dev：Vite 服务 desktop-lyrics.html；release：dist 内多页产物
        let dev = cfg!(debug_assertions);
        let base = if dev {
            "http://localhost:1420/desktop-lyrics.html".to_string()
        } else {
            // tauri build 时 frontendDist 已包含 desktop-lyrics.html
            "desktop-lyrics.html".to_string()
        };
        base
    };
    let (w, h) = (800.0f64, 110.0f64);
    // 位置记忆：上次关闭时保存的 geometry（逻辑像素），无记录则默认主屏
    // 水平居中、垂直 82% 处（不挡任务栏）
    let default_pos = || {
        app.primary_monitor()
            .ok()
            .flatten()
            .map(|m| {
                let s = m.size();
                let sc = m.scale_factor();
                let (sw, sh) = (s.width as f64 / sc, s.height as f64 / sc);
                ((sw - w) / 2.0, sh * 0.82 - h / 2.0)
            })
            .unwrap_or((120.0, 640.0))
    };
    // "x,y,w,h" 四元组（逻辑像素）
    let saved = {
        let st = app.state::<AppState>();
        let conn = st.db.lock();
        db::get_setting(&conn, "dlyrics_geom")
    };
    let (x, y, ww, hh) = saved
        .and_then(|s| {
            let p: Vec<f64> = s.split(',').filter_map(|v| v.parse().ok()).collect();
            (p.len() == 4).then_some((p[0], p[1], p[2], p[3]))
        })
        .map(|(x, y, ww, hh)| (x, y, ww.max(320.0), hh.max(70.0)))
        .unwrap_or_else(|| {
            let (x, y) = default_pos();
            (x, y, w, h)
        });
    let builder = WebviewWindowBuilder::new(&app, "desktop-lyrics", WebviewUrl::App(url.into()))
        .title("桌面歌词")
        .inner_size(ww, hh)
        .position(x, y)
        .decorations(false)
        .transparent(true)
        // WebView2 透明：alpha=0 的背景色是 Windows 下真正穿透的关键
        .background_color(tauri::utils::config::Color(0, 0, 0, 0))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .shadow(false);
    builder
        .build()
        .map_err(|e| format!("创建桌面歌词窗口失败: {e}"))?;
    Ok(())
}

/// 关闭桌面歌词窗口（无窗口时静默成功）；关闭前把位置尺寸存进设置表
#[tauri::command]
pub async fn desktop_lyrics_close(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        // 几何持久化（逻辑像素四元组）
        let scale = w.scale_factor().unwrap_or(1.0);
        if let (Ok(pos), Ok(size)) = (w.outer_position(), w.inner_size()) {
            let geom = format!(
                "{},{},{},{}",
                pos.x as f64 / scale,
                pos.y as f64 / scale,
                size.width as f64 / scale,
                size.height as f64 / scale
            );
            let st = app.state::<AppState>();
            let conn = st.db.lock();
            let _ = db::set_setting(&conn, "dlyrics_geom", &geom);
        }
        let _ = w.close();
    }
    Ok(())
}

/// 解锁桌面歌词（锁定 = set_ignore_cursor_events，穿透后窗口收不到点击，
/// 由主窗口的全局快捷键/设置开关调此命令恢复交互）
#[tauri::command]
pub async fn desktop_lyrics_unlock(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        let _ = w.set_ignore_cursor_events(false);
    }
    Ok(())
}
