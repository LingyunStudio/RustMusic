use lofty::prelude::*;
use serde_json::json;
use std::io::{Read, Write};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db;
use crate::engine::TrackInfo;
use crate::library;
use crate::lyrics;
use crate::models::*;
use crate::updater;
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

// ---------- 输出设备 ----------

/// 枚举输出设备 + 当前生效的设备名
#[tauri::command]
pub async fn list_output_devices(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
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
    db::set_setting(&conn, "output_device", pref.unwrap_or(""));
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
                return Ok(LyricsPayload {
                    synced: p.synced,
                    lines: p.lines,
                });
            }
        }
    }

    // 内嵌歌词
    if let Ok(tagged) = lofty::read_from_path(&path) {
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            if let Some(text) = tag.get_string(&lofty::tag::ItemKey::Lyrics) {
                let p = lyrics::parse(text);
                if !p.lines.is_empty() {
                    return Ok(LyricsPayload {
                        synced: p.synced,
                        lines: p.lines,
                    });
                }
            }
        }
    }
    Ok(LyricsPayload {
        synced: false,
        lines: vec![],
    })
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
        kgid: None,
        quality: (!local_quality.is_empty()).then_some(local_quality),
    };
    let r = engine_clone(&state).play_file(info);
    // 开播成功才计入播放次数/最近播放：文件损坏等播放失败不计
    if r.is_ok() {
        let conn = state.db.lock();
        db::record_play(&conn, id);
    }
    r
}

/// 解析 B 站视频（链接 / BV 号 / av 号 / b23.tv 短链）并加入在线音源列表：
/// 多P视频每个分P各加一条，已存在的自动跳过。返回本次实际新增条数。
#[tauri::command]
pub async fn bilibili_add(state: State<'_, AppState>, input: String) -> Result<usize, String> {
    let rows = crate::bilibili::resolve_pages(&input)?;
    let mut added = 0usize;
    {
        let conn = state.db.lock();
        for (url, title) in &rows {
            let dup = conn
                .query_row(
                    "SELECT 1 FROM sources WHERE url = ?1",
                    rusqlite::params![url],
                    |_| Ok(()),
                )
                .is_ok();
            if dup {
                continue;
            }
            db::add_source(&conn, url, title)?;
            added += 1;
        }
    }
    Ok(added)
}

#[tauri::command]
pub async fn play_source(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    // B 站视频源（bili:// 存储链接 / 原始视频链接）：解析音频直链后走在线缓存播放
    let bili_input = {
        let conn = state.db.lock();
        db::get_source(&conn, id)
            .map(|item| item.url)
            .filter(|u| crate::bilibili::looks_like_bili(u))
    };
    if let Some(url) = bili_input {
        if let Some((audio_url, info)) = crate::bilibili::resolve(&url)? {
            // 字幕与音频并行预取：开播时歌词缓存已就绪
            if let Some(rid) = info.qid.clone() {
                spawn_bili_lyric_warmup(&state, rid);
            }
            // 记录最近播放（rid 与我喜欢/播放列表条目一致：BVxxx-cid）
            {
                let conn = state.db.lock();
                db::record_play_online(
                    &conn,
                    "bilibili",
                    info.qid.as_deref().unwrap_or(url.as_str()),
                    &info.title,
                    &info.artist,
                    &info.album,
                    &info.cover,
                    info.duration_ms as i64,
                    "",
                    false,
                );
            }
            return engine_clone(&state).play_url(audio_url, info);
        }
    }
    let item = {
        let conn = state.db.lock();
        db::get_source(&conn, id).ok_or("音源不存在")?
    };
    let info = TrackInfo {
        id: None,
        kind: "url".into(),
        path: String::new(),
        title: if item.title.is_empty() {
            item.url
                .split('/')
                .next_back()
                .unwrap_or("在线音源")
                .to_string()
        } else {
            item.title.clone()
        },
        artist: "在线音源".into(),
        album: String::new(),
        cover: String::new(),
        duration_ms: 0,
        nid: None,
        qid: None,
        kgid: None,
        quality: None,
    };
    engine_clone(&state).play_url(item.url, info)
}

/// 运行时放行 asset 协议路径：自定义皮肤图片可能在用户目录任意位置
#[tauri::command]
pub async fn asset_scope_allow(app: AppHandle, path: String) -> Result<(), String> {
    app.asset_protocol_scope()
        .allow_file(path)
        .map_err(|e| e.to_string())
}

/// 自定义皮肤图的展示用压缩副本：整图解码后按屏幕级尺寸（≤1920px）重存，
/// 避免原始分辨率（4K/8K 壁纸解码可达数十至上百 MB）常驻渲染进程图片缓存。
/// 动图（gif）与已 ≤1920px 的图原样返回；任何失败都回退原路径，不影响选图
#[tauri::command]
pub async fn prepare_skin_image(
    state: State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    let lower = path.to_lowercase();
    if lower.ends_with(".gif") {
        return Ok(path);
    }
    let src = std::path::Path::new(&path);
    let (mtime, size) = {
        let md = std::fs::metadata(src).map_err(|e| e.to_string())?;
        (
            md.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            md.len(),
        )
    };
    // 目标名绑定来源路径 + 内容版本：换图/改图后不会读到旧压缩副本
    let mut h = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    path.hash(&mut h);
    mtime.hash(&mut h);
    size.hash(&mut h);
    let stem = format!("{:016x}", h.finish());

    let dir = state.app_data.join("skins");
    let _ = std::fs::create_dir_all(&dir);

    // 已有副本直接复用（重复选同一张图不重复解码）
    let hit = dir.join(format!("{stem}.png"));
    if hit.exists() {
        return Ok(hit.to_string_lossy().into_owned());
    }
    let hit_jpg = dir.join(format!("{stem}.jpg"));
    if hit_jpg.exists() {
        return Ok(hit_jpg.to_string_lossy().into_owned());
    }

    // 先廉价读尺寸：≤1920 的图不做整图解码，直接原样返回
    let (w, h) = image::ImageReader::open(src)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .and_then(|r| r.into_dimensions().ok())
        .ok_or_else(|| "读取图片信息失败".to_string())?;
    if w <= 1920 && h <= 1920 {
        return Ok(path);
    }

    let img = image::open(src).map_err(|e| e.to_string())?;
    let small = img.thumbnail(1920, 1920);
    // 带透明通道的图存 PNG 保持透明；照片存 JPEG 体积小
    let dest = if small.color().has_alpha() {
        hit
    } else {
        hit_jpg
    };
    small
        .save_with_format(&dest, if dest.extension().and_then(|e| e.to_str()) == Some("png") {
            image::ImageFormat::Png
        } else {
            image::ImageFormat::Jpeg
        })
        .map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

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
    // 先验证再落凭据
    crate::navidrome::ping(&server, &username, &req.password)?;
    crate::navidrome::save_password(&server, &username, &req.password)?;
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
    crate::navidrome::ping(&crate::navidrome::norm_base(&server), &username, "")
        .map_err(|e| format!("连接失败：{e}（请检查服务器地址或重新保存密码）"))
}

#[tauri::command]
pub async fn navidrome_forget(server: String, username: String) -> Result<(), String> {
    crate::navidrome::delete_password(&crate::navidrome::norm_base(&server), &username)
}

#[tauri::command]
pub async fn navidrome_search(
    state: State<'_, AppState>,
    server: String,
    username: String,
    query: String,
) -> Result<Vec<crate::navidrome::NdSong>, String> {
    let conn = state.db.lock();
    let saved = db::get_setting(&conn, "navidrome_server").unwrap_or_default();
    drop(conn);
    let server = if server.is_empty() { saved } else { server };
    crate::navidrome::search_songs(&server, &username, &query)
}

#[tauri::command]
pub async fn navidrome_albums(
    state: State<'_, AppState>,
    server: String,
    username: String,
) -> Result<Vec<crate::navidrome::NdAlbum>, String> {
    let conn = state.db.lock();
    let saved = db::get_setting(&conn, "navidrome_server").unwrap_or_default();
    drop(conn);
    let server = if server.is_empty() { saved } else { server };
    crate::navidrome::album_list(&server, &username)
}

#[tauri::command]
pub async fn navidrome_album_songs(
    state: State<'_, AppState>,
    server: String,
    username: String,
    id: String,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();
    let saved = db::get_setting(&conn, "navidrome_server").unwrap_or_default();
    drop(conn);
    let server = if server.is_empty() { saved } else { server };
    let (name, artist, songs) = crate::navidrome::album_songs(&server, &username, &id)?;
    Ok(json!({ "name": name, "artist": artist, "songs": songs }))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NdStreamReq {
    pub server: String,
    pub username: String,
    pub id: String,
}

#[tauri::command]
pub async fn navidrome_stream_url(req: NdStreamReq) -> Result<String, String> {
    crate::navidrome::stream_url(&crate::navidrome::norm_base(&req.server), &req.username, &req.id)
}

#[tauri::command]
pub async fn navidrome_all_songs(
    state: State<'_, AppState>,
    server: String,
    username: String,
    offset: u64,
) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();
    let saved = db::get_setting(&conn, "navidrome_server").unwrap_or_default();
    drop(conn);
    let server = if server.is_empty() { saved } else { server };
    let password = crate::navidrome::get_password_pub(&server, &username)?;
    let v = crate::navidrome::search_all(&server, &username, &password, offset)?;
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
    state: State<'_, AppState>,
    server: String,
    username: String,
    track: NdPlayReq,
) -> Result<(), String> {
    let conn = state.db.lock();
    let saved = db::get_setting(&conn, "navidrome_server").unwrap_or_default();
    drop(conn);
    let server = if server.is_empty() { saved } else { server };
    let url = crate::navidrome::stream_url(&server, &username, &track.id)?;
    {
        let conn = state.db.lock();
        db::record_play_online(
            &conn,
            "navidrome",
            &track.id,
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
    crate::navidrome::lyrics(&server, &username, &id)
}

// ---------- B 站登录（扫码） ----------
// ---------- B 站登录（扫码） ----------

#[tauri::command]
pub async fn bilibili_qr_create() -> Result<serde_json::Value, String> {
    let (key, qr) = crate::bilibili::login_qr_create()?;
    Ok(json!({ "key": key, "qr": qr }))
}

#[tauri::command]
pub async fn bilibili_qr_check(
    state: State<'_, AppState>,
    key: String,
) -> Result<serde_json::Value, String> {
    let r = crate::bilibili::login_qr_check(&key)?;
    if r.status == "success" {
        if let Some(c) = &r.cookies {
            let conn = state.db.lock();
            db::set_setting(&conn, "bili_sessdata", &c.sessdata);
            db::set_setting(&conn, "bili_buvid3", &c.buvid3);
            if let Some(nick) = &r.nickname {
                db::set_setting(&conn, "bili_nickname", nick);
            }
        }
    }
    Ok(json!({ "status": r.status, "nickname": r.nickname }))
}

#[tauri::command]
pub async fn bilibili_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let conn = state.db.lock();
    let sessdata = db::get_setting(&conn, "bili_sessdata").unwrap_or_default();
    let nickname = db::get_setting(&conn, "bili_nickname").unwrap_or_default();
    Ok(json!({
        "loggedIn": !sessdata.is_empty(),
        "nickname": nickname,
    }))
}

#[tauri::command]
pub async fn bilibili_logout(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    db::set_setting(&conn, "bili_sessdata", "");
    db::set_setting(&conn, "bili_buvid3", "");
    db::set_setting(&conn, "bili_nickname", "");
    Ok(())
}

/// B 站字幕歌词：rid = "BVxxx-cid"。字幕列表需登录（SESSDATA），
/// 未登录返回无歌词（字幕接口匿名一律为空列表）。
#[tauri::command]
/// 拉取并缓存 B 站字幕歌词（bilibili_lyric 命令与预热线程共用）。
/// 字幕内容稳定：进程内缓存，避免每次播放重复拉取。
fn bili_lyric_fetch(
    rid: &str,
    sessdata: &str,
    buvid3: &str,
) -> Result<crate::models::LyricsPayload, String> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, crate::models::LyricsPayload>>,
    > = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(hit) = cache.lock().ok().and_then(|m| m.get(rid).cloned()) {
        return Ok(hit);
    }
    let (bvid, cid) = crate::bilibili::parse_rid(rid)?;
    let c = if sessdata.is_empty() {
        None
    } else {
        Some(crate::bilibili::BiliCookies {
            sessdata: sessdata.to_string(),
            buvid3: buvid3.to_string(),
        })
    };
    let payload = crate::bilibili::subtitle_lyrics(&bvid, &cid.to_string(), c.as_ref())?
        .unwrap_or(crate::models::LyricsPayload {
            synced: false,
            lines: vec![],
        });
    if !payload.lines.is_empty() {
        if let Ok(mut m) = cache.lock() {
            m.insert(rid.to_string(), payload.clone());
        }
    }
    Ok(payload)
}

/// 播放前后台预取字幕（与音频下载并行）：开播时歌词缓存已就绪，秒出。
/// 失败静默——播放不受影响，前端随后请求时再重试。
fn spawn_bili_lyric_warmup(state: &State<'_, AppState>, rid: String) {
    let (sessdata, buvid3) = {
        let conn = state.db.lock();
        (
            db::get_setting(&conn, "bili_sessdata").unwrap_or_default(),
            db::get_setting(&conn, "bili_buvid3").unwrap_or_default(),
        )
    };
    std::thread::spawn(move || {
        let _ = bili_lyric_fetch(&rid, &sessdata, &buvid3);
    });
}

#[tauri::command]
pub async fn bilibili_lyric(
    state: State<'_, AppState>,
    rid: String,
) -> Result<crate::models::LyricsPayload, String> {
    let (sessdata, buvid3) = {
        let conn = state.db.lock();
        (
            db::get_setting(&conn, "bili_sessdata").unwrap_or_default(),
            db::get_setting(&conn, "bili_buvid3").unwrap_or_default(),
        )
    };
    bili_lyric_fetch(&rid, &sessdata, &buvid3)
}

// ---------- B 站 UP 主空间 ----------

/// 空间登录态 cookie：有 SESSDATA 用登录态；未登录也带 buvid3 指纹降风控
fn bili_cookies_opt(state: &State<'_, AppState>) -> Option<crate::bilibili::BiliCookies> {
    let conn = state.db.lock();
    let sessdata = db::get_setting(&conn, "bili_sessdata").unwrap_or_default();
    let buvid3 = db::get_setting(&conn, "bili_buvid3").unwrap_or_default();
    drop(conn);
    if sessdata.is_empty() && buvid3.is_empty() {
        return Some(crate::bilibili::BiliCookies {
            sessdata: String::new(),
            buvid3: crate::bilibili::login_buvid3(),
        });
    }
    Some(crate::bilibili::BiliCookies { sessdata, buvid3 })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BiliSpaceItem {
    rid: String,
    title: String,
    artist: String,
    cover: String,
    duration_ms: u64,
    play: u64,
    created: i64,
}

fn space_items(rows: Vec<crate::bilibili::SpaceVideo>, up: &str) -> Vec<BiliSpaceItem> {
    rows.into_iter()
        .map(|v| BiliSpaceItem {
            rid: v.bvid.clone(),
            title: v.title,
            artist: if up.is_empty() { "哔哩哔哩".into() } else { up.to_string() },
            cover: v.cover,
            duration_ms: v.duration_ms,
            play: v.play,
            created: v.created,
        })
        .collect()
}

/// UP 主信息 + 投稿列表第一页（order: pubdate|click|stow，服务端全量排序）+ 合集列表
#[tauri::command]
pub async fn bilibili_space(
    state: State<'_, AppState>,
    input: String,
    order: String,
) -> Result<serde_json::Value, String> {
    let mid = crate::bilibili::parse_space(&input)?;
    let c = bili_cookies_opt(&state);
    let (name, face, fans, card_total) = crate::bilibili::space_card(&mid)?;
    let (rows, list_total, has_more) = crate::bilibili::space_videos(&mid, &order, 1, c.as_ref())?;
    let seasons = crate::bilibili::space_seasons(&mid, c.as_ref()).unwrap_or_default();
    let total = if list_total > 0 { list_total } else { card_total };
    Ok(json!({
        "mid": mid,
        "name": name,
        "face": face,
        "fans": fans,
        "total": total,
        "hasMore": has_more,
        "items": space_items(rows, &name),
        "collections": seasons,
    }))
}

/// UP 主投稿列表翻页
#[tauri::command]
pub async fn bilibili_space_more(
    state: State<'_, AppState>,
    mid: String,
    order: String,
    pn: i64,
) -> Result<serde_json::Value, String> {
    let c = bili_cookies_opt(&state);
    let pn = pn.max(1) as u32;
    let (rows, list_total, has_more) = crate::bilibili::space_videos(&mid, &order, pn, c.as_ref())?;
    let (name, _face, _fans, card_total) = crate::bilibili::space_card(&mid)?;
    let total = if list_total > 0 { list_total } else { card_total };
    Ok(json!({
        "total": total,
        "hasMore": has_more,
        "items": space_items(rows, &name),
    }))
}

/// 需要登录态的 B 站调用：取出 SESSDATA cookie，未登录报错
fn bili_cookies_req(state: &State<'_, AppState>) -> Result<crate::bilibili::BiliCookies, String> {
    let c = bili_cookies_opt(state).unwrap_or(crate::bilibili::BiliCookies {
        sessdata: String::new(),
        buvid3: String::new(),
    });
    if c.sessdata.is_empty() {
        return Err("请先扫码登录 B 站账号".into());
    }
    Ok(c)
}

/// 登录用户创建的收藏夹列表（需 B 站登录；私密收藏夹一并可见）
#[tauri::command]
pub async fn bilibili_fav_folders(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let c = bili_cookies_req(&state)?;
    let (user, folders) = tauri::async_runtime::spawn_blocking(move || {
        crate::bilibili::fav_folders(&c)
    })
    .await
    .map_err(|e| format!("收藏夹任务失败：{e}"))??;
    Ok(json!({
        "folders": folders,
        "name": user.uname,
        "face": user.face,
    }))
}

/// 收藏夹内容列表（media_id + 页码；分页逻辑同合集）
#[tauri::command]
pub async fn bilibili_fav_list(
    state: State<'_, AppState>,
    media_id: i64,
    pn: i64,
) -> Result<serde_json::Value, String> {
    let c = bili_cookies_req(&state)?;
    let pn = pn.max(1) as u32;
    let (rows, total, has_more) =
        tauri::async_runtime::spawn_blocking(move || {
            crate::bilibili::fav_folder_videos(media_id, pn, &c)
        })
        .await
        .map_err(|e| format!("收藏夹任务失败：{e}"))??;
    Ok(json!({
        "total": total,
        "hasMore": has_more,
        "items": space_items(rows, ""),
    }))
}

/// 合集 / 系列视频列表第一页
#[tauri::command]
pub async fn bilibili_space_collection(
    state: State<'_, AppState>,
    mid: String,
    id: i64,
    kind: String,
) -> Result<serde_json::Value, String> {
    let c = bili_cookies_opt(&state);
    let (rows, total, has_more) = match kind.as_str() {
        "season" => crate::bilibili::space_season_archives(&mid, id, 1, c.as_ref())?,
        // "fav"：登录用户收藏夹（复用结果区 UI，id = media_id）
        "fav" => {
            let sess = bili_cookies_req(&state)?;
            crate::bilibili::fav_folder_videos(id, 1, &sess)?
        }
        _ => crate::bilibili::space_series_archives(&mid, id, 1, c.as_ref())?,
    };
    Ok(json!({
        "total": total,
        "hasMore": has_more,
        "items": space_items(rows, ""),
    }))
}

/// 合集 / 系列视频翻页
#[tauri::command]
pub async fn bilibili_space_collection_more(
    state: State<'_, AppState>,
    mid: String,
    id: i64,
    kind: String,
    pn: i64,
) -> Result<serde_json::Value, String> {
    let c = bili_cookies_opt(&state);
    let (rows, total, has_more) = match kind.as_str() {
        "season" => crate::bilibili::space_season_archives(&mid, id, pn.max(1) as u32, c.as_ref())?,
        "fav" => {
            let sess = bili_cookies_req(&state)?;
            crate::bilibili::fav_folder_videos(id, pn.max(1) as u32, &sess)?
        }
        _ => crate::bilibili::space_series_archives(&mid, id, pn.max(1) as u32, c.as_ref())?,
    };
    Ok(json!({
        "total": total,
        "hasMore": has_more,
        "items": space_items(rows, ""),
    }))
}

/// 解析单个 B 站视频（当前结果展示，不落库）：每分P一条，rid = "BVxxx-cid"
#[tauri::command]
pub async fn bilibili_video_info(input: String) -> Result<Vec<BiliSpaceItem>, String> {
    let rows = crate::bilibili::video_rows(&input)?;
    Ok(rows
        .into_iter()
        .map(|(rid, title, artist, cover, duration_ms)| BiliSpaceItem {
            rid,
            title,
            artist,
            cover,
            duration_ms,
            play: 0,
            created: 0,
        })
        .collect())
}

// ---------- 网易云在线曲库 ----------

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BiliPlayReq {
    /// "BVxxx-cid"（与喜欢/播放列表条目的 onlineId 一致）
    pub rid: String,
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

/// 播放 B 站曲目（我喜欢/播放列表/最近播放入口）：元数据由前端条目提供，
/// 后端只负责解析音频直链并记录最近播放。
#[tauri::command]
pub async fn bilibili_play(state: State<'_, AppState>, track: BiliPlayReq) -> Result<(), String> {
    let (bvid, cid) = crate::bilibili::parse_rid(&track.rid)?;
    let (audio_url, quality) = crate::bilibili::audio_stream(&bvid, cid)?;
    // 字幕与音频并行预取：开播时歌词缓存已就绪
    spawn_bili_lyric_warmup(&state, track.rid.clone());
    {
        let conn = state.db.lock();
        db::record_play_online(
            &conn,
            "bilibili",
            &track.rid,
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
        kind: "bilibili".into(),
        path: String::new(),
        title: track.title,
        artist: track.artist,
        album: track.album,
        cover: track.cover,
        duration_ms: track.duration_ms,
        nid: None,
        qid: Some(track.rid),
        kgid: None,
        quality: Some(quality),
    };
    engine_clone(&state).play_url(audio_url, info)
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

/// 音质标签（在线曲目播放栏徽标）：FLAC → 无损；≥320kbps → HQ；其余 → 标准
fn quality_tag(ext: &str, br_kbps: i64) -> String {
    if ext.eq_ignore_ascii_case("flac") {
        "无损".to_string()
    } else if br_kbps >= 320 {
        "HQ".to_string()
    } else {
        "标准".to_string()
    }
}

/// 音质回退提示：设置的档位未被账号权益满足、实际下发更低档时告知用户
/// （静默降级曾让"无损"设置实际播放 128k 而用户无从得知）
fn notify_quality_fallback(
    app: &AppHandle,
    source: &str,
    quality: &str,
    actual_label: &str,
    reason: &str,
) {
    if quality == "standard" {
        return;
    }
    let wanted = match quality {
        "lossless" => "无损",
        "high" => "HQ",
        _ => return,
    };
    if actual_label == wanted {
        return;
    }
    let _ = app.emit(
        "player://quality-fallback",
        serde_json::json!({ "message": format!("{source}：当前以「{actual_label}」音质播放（{reason}）") }),
    );
}

/// 酷狗存档行（最近播放/收藏/歌单）只落库了 128 hash：按标题反查各档
/// 质量 hash 补齐，否则 HQ/无损音质会静默塌缩到标准
fn kugou_hashes_enriched(
    hash: &str,
    title: &str,
    hq_hash: &str,
    sq_hash: &str,
    super_hash: &str,
) -> (String, String, String) {
    crate::kugou::enrich_hashes(hash, title, hq_hash, sq_hash, super_hash)
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
        .ok_or_else(|| "该歌曲暂无可播放链接（可能需要登录，或需要有效 VIP 权益）".to_string())?;
    let quality_label = quality_tag(&ext, br);
    if quality == "lossless" && !ext.eq_ignore_ascii_case("flac") {
        notify_quality_fallback(&app, "网易云", &quality, &quality_label, "账号权益未含无损或该曲无更高音质");
    }
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
        kgid: None,
        quality: Some(quality_label.to_string()),
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
pub async fn netease_like(state: State<'_, AppState>, id: i64, like: bool) -> Result<(), String> {
    let music_u = netease_cookie(&state).ok_or("未登录网易云账号")?;
    crate::netease::like(id, like, &music_u)
}

#[tauri::command]
pub async fn netease_lyric(state: State<'_, AppState>, id: i64) -> Result<LyricsPayload, String> {
    let music_u = netease_cookie(&state);
    // 优先逐字歌词（yrc）：染色推进贴合实际演唱节奏；无则回落行级
    if let Ok(Some(enhanced)) = crate::netease::lyric_yrc(id, music_u.as_deref()) {
        let p = lyrics::parse(&enhanced);
        if p.synced {
            return Ok(LyricsPayload {
                synced: p.synced,
                lines: p.lines,
            });
        }
    }
    let lrc = crate::netease::lyric(id, music_u.as_deref())?;
    let text = lrc.unwrap_or_default();
    if text.is_empty() {
        return Ok(LyricsPayload {
            synced: false,
            lines: vec![],
        });
    }
    let p = lyrics::parse(&text);
    Ok(LyricsPayload {
        synced: p.synced,
        lines: p.lines,
    })
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
pub async fn qq_search(keyword: String, page: Option<i64>) -> Result<serde_json::Value, String> {
    let songs = crate::qq::search(&keyword, 30, page.unwrap_or(1))?;
    Ok(json!({ "songs": songs }))
}

#[tauri::command]
pub async fn qq_play(app: AppHandle, state: State<'_, AppState>, track: QqPlayReq) -> Result<(), String> {
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
        notify_quality_fallback(&app, "QQ 音乐", &quality, quality_label, "账号权益未含无损或该曲无 flac");
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
pub async fn qq_lyric(
    state: State<'_, AppState>,
    songmid: String,
) -> Result<LyricsPayload, String> {
    // 优先逐字歌词（QRC，需登录）；失败回落匿名行级接口
    if let Ok((musicid, musickey)) = qq_credential(&state) {
        if let Ok(Some(enhanced)) = crate::qq::lyric_qrc(&songmid, &musicid, &musickey) {
            let p = lyrics::parse(&enhanced);
            if p.synced {
                return Ok(LyricsPayload {
                    synced: p.synced,
                    lines: p.lines,
                });
            }
        }
    }
    let text = crate::qq::lyric(&songmid)?.unwrap_or_default();
    if text.is_empty() {
        return Ok(LyricsPayload {
            synced: false,
            lines: vec![],
        });
    }
    let p = lyrics::parse(&text);
    Ok(LyricsPayload {
        synced: p.synced,
        lines: p.lines,
    })
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
fn kg_prepare(state: &State<AppState>) -> (String, String) {
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
    // 入库时已记录的酷狗歌词直接走远端；本地缓存散布在播放缓存之外，简化为直取
    let _ = &state;
    let text = crate::kugou::lyric(&hash)?;
    let text = match text {
        Some(t) if !t.is_empty() => t,
        _ => {
            return Ok(LyricsPayload {
                synced: false,
                lines: vec![],
            })
        }
    };
    let p = lyrics::parse(&text);
    Ok(LyricsPayload {
        synced: p.synced,
        lines: p.lines,
    })
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
    let songs = crate::kugou::toplist_tracks(top_id, page.unwrap_or(1))?;
    Ok(json!({ "songs": songs }))
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
    let toplists = crate::qq::toplists()?;
    Ok(json!({ "toplists": toplists }))
}

#[tauri::command]
pub async fn qq_toplist_tracks(top_id: i64) -> Result<serde_json::Value, String> {
    let songs = crate::qq::toplist_tracks(top_id)?;
    Ok(json!({ "songs": songs }))
}

#[tauri::command]
pub async fn qq_random_playlist() -> Result<crate::qq::QqPublicPlaylist, String> {
    crate::qq::random_playlist()
}

#[tauri::command]
pub async fn netease_toplists() -> Result<serde_json::Value, String> {
    let toplists = crate::netease::toplists()?;
    Ok(json!({ "toplists": toplists }))
}

/// 榜单曲目（榜单 ID 即歌单 ID，明文 v6 匿名可拉；登录后按权益取播放链接）
#[tauri::command]
pub async fn netease_toplist_tracks(
    state: State<'_, AppState>,
    top_id: i64,
) -> Result<serde_json::Value, String> {
    let music_u = netease_cookie(&state).unwrap_or_default();
    let songs = crate::netease::playlist_tracks(top_id, &music_u)?;
    Ok(json!({ "songs": songs }))
}

#[tauri::command]
pub async fn netease_random_playlist(
    state: State<'_, AppState>,
) -> Result<crate::netease::NetRandomPlaylist, String> {
    let music_u = netease_cookie(&state).unwrap_or_default();
    crate::netease::random_playlist(&music_u)
}

#[tauri::command]
pub async fn netease_daily_recommend(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let music_u = netease_cookie(&state).ok_or("请先登录网易云账号")?;
    let songs = crate::netease::daily_recommend(&music_u)?;
    Ok(json!({ "songs": songs }))
}

#[tauri::command]
pub async fn netease_personal_fm(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let music_u = netease_cookie(&state).ok_or("请先登录网易云账号")?;
    let songs = crate::netease::personal_fm(&music_u)?;
    Ok(json!({ "songs": songs }))
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
            let id: i64 = req
                .id
                .parse()
                .map_err(|_| "网易云歌曲 ID 无效".to_string())?;
            let music_u = netease_cookie(&state);
            let (u, _br, ext) = crate::netease::song_url(id, music_u.as_deref(), &quality)?
                .ok_or("该歌曲暂无可播放链接")?;
            (u, ext)
        }
        "qq" => {
            let (musicid, musickey) = qq_credential(&state)?;
            let (u, ext, _label) =
                crate::qq::song_url(&req.id, &req.media_mid, &musicid, &musickey, &quality, true)?;
            (u, ext)
        }
        "kugou" => {
            let (token, userid) = kg_prepare(&state);
            // media_mid 列存的是专辑音频 ID（最近播放/歌单导入时写入）
            let album_audio_id = req.media_mid.parse().unwrap_or(0);
            // 存档行只有 128 hash：按标题反查各档 hash，下载音质才能生效
            let (hq, sq, sup) = kugou_hashes_enriched(&req.id, &title, "", "", "");
            let (u, ext, _label) =
                crate::kugou::song_url(&req.id, album_audio_id, 0, &hq, &sq, &sup, true, &token, &userid, &quality)?;
            (u, ext)
        }
        "bilibili" => {
            // rid = "BVxxx-cid"；存为 .m4a（内容是 fragmented MP4，symphonia 可解）
            let (bvid, cid) = crate::bilibili::parse_rid(&req.id)?;
            let (u, _q) = crate::bilibili::audio_stream(&bvid, cid)?;
            (u, "m4a".to_string())
        }
        "navidrome" => {
            let conn = state.db.lock();
            let server = db::get_setting(&conn, "navidrome_server").unwrap_or_default();
            let username = db::get_setting(&conn, "navidrome_username").unwrap_or_default();
            drop(conn);
            if server.is_empty() {
                return Err("尚未连接 Navidrome 服务器".into());
            }
            let url = crate::navidrome::stream_url(&server, &username, &req.id)?;
            // 扩展名从流 URL 路径推断，推断不出按 mp3
            let path = url.split('?').next().unwrap_or("");
            let ext = path
                .rsplit('.')
                .next()
                .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
                .unwrap_or("mp3")
                .to_string();
            (url, ext)
        }
        _ => return Err("未知音源类型".into()),
    };

    // 2) 下载到保存目录
    let dir = save_dir(&state);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建保存目录失败: {e}"))?;
    let artist = sanitize_filename(&req.artist);
    let name = format!(
        "{} - {}.{}",
        if artist.is_empty() {
            "Unknown"
        } else {
            &artist
        },
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
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("下载失败: {e}"))?;
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
            let _ = app.emit(
                "download://progress",
                json!({ "url": &title, "done": true }),
            );
        }
        return Err(e);
    }
    let _ = app.emit(
        "download://progress",
        json!({ "url": &title, "received": received, "total": if total == 0 { received } else { total }, "pct": 100, "done": true }),
    );

    // 3) 取歌词并写标签（失败静默）。B 站缓存是 fragmented MP4：
    //    lofty 无法安全写标签（可能损坏流），跳过——标题/时长由入库元数据兜底
    let lyrics = match req.kind.as_str() {
        "netease" => crate::netease::lyric(
            req.id.parse::<i64>().unwrap_or(0),
            netease_cookie(&state).as_deref(),
        )
        .ok()
        .flatten(),
        "qq" => crate::qq::lyric(&req.id).ok().flatten(),
        "kugou" => crate::kugou::lyric(&req.id).ok().flatten(),
        _ => None,
    };
    if req.kind != "bilibili" {
        write_tags(
            &dest,
            &title,
            &req.artist,
            &req.album,
            &req.cover_url,
            lyrics.as_deref(),
        );
    }

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
    if kind == "bilibili" {
        // B 站 CDN 部分边缘节点裸请求 403
        req = req
            .set("Referer", "https://www.bilibili.com/")
            .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36");
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
    name: String,    // 歌单名（前端传入；新建/同名匹配用）
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
        let pid =
            match db::find_playlist_by_remote(&conn, "netease", &remote_pid.to_string(), &name) {
                Some(id) => id,
                None => db::create_playlist(&conn, &name)?,
            };
        db::set_playlist_remote(&conn, pid, "netease", &remote_pid.to_string());
        // 记录原始导入名：改名后重导入仍能认出（配合 remote id 兜底）
        db::set_playlist_origin(&conn, pid, &name);
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
    let (list_id, added) = {
        let conn = state.db.lock();
        let pid = match db::find_playlist_by_remote(&conn, "qq", &remote_pid.to_string(), &name) {
            Some(id) => id,
            None => db::create_playlist(&conn, &name)?,
        };
        db::set_playlist_remote(&conn, pid, "qq", &remote_pid.to_string());
        // 记录原始导入名：改名后重导入仍能认出（配合 remote id 兜底）
        db::set_playlist_origin(&conn, pid, &name);
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

/// WASAPI 独占模式开关（切换后下一首生效）
#[tauri::command]
pub async fn set_wasapi_exclusive(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let eng = state.engine.lock().clone();
    eng.set_exclusive_enabled(enabled);
    // 关闭独占时立即停止会话、把设备还给系统混音器，并从当前进度切回共享续播
    if !enabled {
        if let Err(e) = eng.stop_exclusive_resume_shared() {
            eprintln!("[engine] 关闭独占切回共享失败: {e}");
        }
    }
    let conn = state.db.lock();
    db::set_setting(
        &conn,
        "wasapi_exclusive",
        if enabled { "true" } else { "false" },
    );
    Ok(())
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

/// 当前播放状态快照（含进度）。WebView 挂起恢复窗口期的事件推送可能丢失，
/// 前端恢复后主动拉取本命令做权威同步，不依赖任何固定延迟的补发。
#[tauri::command]
pub async fn get_play_state(
    state: State<'_, AppState>,
) -> Result<Option<crate::engine::PlayStateSnapshot>, String> {
    Ok(engine_clone(&state).snapshot())
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
    let close_action = db::get_setting(&conn, "close_action").unwrap_or_else(|| "tray".to_string());
    let auto_update = db::get_setting(&conn, "auto_update")
        .map(|s| s != "false")
        .unwrap_or(true);
    let wasapi_exclusive = db::get_setting(&conn, "wasapi_exclusive")
        .map(|s| s == "true")
        .unwrap_or(false);
    Ok(SettingsPayload {
        volume,
        speed,
        eq_gains,
        eq_enabled,
        quality,
        cache_limit,
        close_action,
        auto_update,
        wasapi_exclusive,
    })
}

/// 设置是否在启动时自动检查更新
#[tauri::command]
pub async fn set_auto_update(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let conn = state.db.lock();
    db::set_setting(&conn, "auto_update", if enabled { "true" } else { "false" });
    Ok(())
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
pub async fn set_cache_limit(state: State<'_, AppState>, bytes: u64) -> Result<(), String> {
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
pub async fn extract_cover_palette(url: String) -> Result<Vec<String>, String> {
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
        let (r, g, b) = (
            p[0] as f64 / 255.0,
            p[1] as f64 / 255.0,
            p[2] as f64 / 255.0,
        );
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
    // "x,y,w,h" 四元组（逻辑像素）。
    // 注意：几何里存的是“歌词区高度”；实际窗口还要加上顶部 40px 的
    // 控制条预留区（前端常驻，不 hover 时全透明）——打开加、关闭减，
    // 保证歌词区始终是用户设定的高度，控制条不挤占歌词空间
    const CTRL_STRIP: f64 = 40.0;
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
        .inner_size(ww, hh + CTRL_STRIP)
        .position(x, (y - CTRL_STRIP).max(0.0))
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
    // 与 desktop_lyrics_open 对应：窗口高度含 40px 控制条预留区，
    // 保存几何时减掉，保证下次打开歌词区高度不变
    const CTRL_STRIP: f64 = 40.0;
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        // 几何持久化（逻辑像素四元组）
        let scale = w.scale_factor().unwrap_or(1.0);
        if let (Ok(pos), Ok(size)) = (w.outer_position(), w.inner_size()) {
            let geom = format!(
                "{},{},{},{}",
                (pos.x as f64 / scale),
                (pos.y as f64 / scale) + CTRL_STRIP,
                size.width as f64 / scale,
                ((size.height as f64 / scale) - CTRL_STRIP).max(70.0)
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

/// 桌面歌词窗口是否还开着。主窗口挂起期间歌词窗口可能被直接关闭
///（关闭事件丢失），恢复后查询本命令校准“词”按钮状态
#[tauri::command]
pub async fn desktop_lyrics_is_open(app: AppHandle) -> Result<bool, String> {
    Ok(app.get_webview_window("desktop-lyrics").is_some())
}

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

/// 用系统默认浏览器打开链接（release notes 内的跳转用）
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("不支持的链接".into());
    }
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("打开链接失败：{e}"))?;
    Ok(())
}
