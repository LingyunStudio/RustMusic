//! 在线歌曲收藏 / 下载到本地（写标签入库）/ 我喜欢与最近播放在线条目

use serde_json::json;
use std::io::{Read, Write};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db;

use crate::AppState;

use super::kugou::{kg_prepare, kugou_hashes_enriched};
use super::netease::netease_cookie;
use super::qq::qq_credential;
// ---------- 在线歌曲收藏到本地 / 歌单导入 ----------

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineSaveReq {
    pub kind: String, // netease | qq | kugou
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
    /// 酷狗专辑音频 ID（v5 取链必需；media_mid 列只在存档行有值）
    #[serde(default)]
    pub album_audio_id: u64,
    /// 酷狗各档质量 hash（搜索/榜单行自带；最近播放等存档行可缺省，
    /// 缺省时按标题反查补齐）——下载音质 HQ/无损必需
    #[serde(default)]
    pub hq_hash: String,
    #[serde(default)]
    pub sq_hash: String,
    #[serde(default)]
    pub super_hash: String,
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
pub async fn download_online(app: AppHandle, req: OnlineSaveReq) -> Result<String, String> {
    let task_app = app.clone();
    super::blocking(move || download_online_task(&task_app, req)).await
}

/// 同步任务体：取链 + 分块下载 + 封面拉取 + 标签写入全是阻塞 IO，
/// 整体放专用线程池执行（大文件可能持续数分钟，绝不能占用异步 worker）
fn download_online_task(app: &AppHandle, req: OnlineSaveReq) -> Result<String, String> {
    let state = app.state::<AppState>();
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
            // 搜索行带 albumAudioId；存档行的 media_mid 列也存了它
            let album_audio_id = if req.album_audio_id > 0 {
                req.album_audio_id
            } else {
                req.media_mid.parse().unwrap_or(0)
            };
            // 前端带来的各档 hash 优先；存档行没有时按标题反查补齐
            let (hq, sq, sup) = kugou_hashes_enriched(
                &req.id,
                &title,
                &req.artist,
                &req.hq_hash,
                &req.sq_hash,
                &req.super_hash,
            );
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
        .flatten()
        .map(|t| t.lrc),
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
