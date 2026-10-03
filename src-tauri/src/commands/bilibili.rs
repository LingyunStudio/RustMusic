//! B 站：扫码登录 / 字幕歌词 / UP 主空间 / 收藏夹 / 合集 / 视频解析 / 播放

use serde_json::json;
use tauri::{AppHandle, Manager, State};

use crate::db;
use crate::engine::TrackInfo;

use crate::AppState;

use super::engine_clone;
/// 解析 B 站视频（链接 / BV 号 / av 号 / b23.tv 短链）并加入在线音源列表：
/// 多P视频每个分P各加一条，已存在的自动跳过。返回本次实际新增条数。
#[tauri::command]
pub async fn bilibili_add(state: State<'_, AppState>, input: String) -> Result<usize, String> {
    let rows = super::blocking(move || crate::bilibili::resolve_pages(&input)).await?;
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
pub async fn bilibili_qr_create() -> Result<serde_json::Value, String> {
    let (key, qr) = super::blocking(crate::bilibili::login_qr_create).await?;
    Ok(json!({ "key": key, "qr": qr }))
}

#[tauri::command]
pub async fn bilibili_qr_check(
    state: State<'_, AppState>,
    key: String,
) -> Result<serde_json::Value, String> {
    let r = super::blocking(move || crate::bilibili::login_qr_check(&key)).await?;
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

/// 拉取并缓存 B 站字幕歌词（bilibili_lyric 命令与预热线程共用）。
/// 字幕内容稳定：进程内缓存，避免每次播放重复拉取。
/// （原文件此处误挂了一个未注册的 #[tauri::command]，已随拆分清理）
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
        .unwrap_or(crate::models::LyricsPayload::new(false, vec![]));
    let mut payload = payload;
    if !payload.lines.is_empty() {
        payload.source = Some("字幕 B站".into());
    }
    if !payload.lines.is_empty() {
        if let Ok(mut m) = cache.lock() {
            m.insert(rid.to_string(), payload.clone());
        }
    }
    Ok(payload)
}

/// 播放前后台预取字幕（与音频下载并行）：开播时歌词缓存已就绪，秒出。
/// 失败静默——播放不受影响，前端随后请求时再重试。
pub(crate) fn spawn_bili_lyric_warmup(state: &State<'_, AppState>, rid: String) {
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

/// B 站字幕歌词：rid = "BVxxx-cid"。字幕列表需登录（SESSDATA），
/// 未登录返回无歌词（字幕接口匿名一律为空列表）。
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
    super::blocking(move || bili_lyric_fetch(&rid, &sessdata, &buvid3)).await
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
    let mid2 = mid.clone();
    let (name, face, fans, card_total, rows, list_total, has_more, seasons) = super::blocking(
        move || -> Result<_, String> {
            let (name, face, fans, card_total) = crate::bilibili::space_card(&mid2)?;
            let (rows, list_total, has_more) =
                crate::bilibili::space_videos(&mid2, &order, 1, c.as_ref())?;
            let seasons = crate::bilibili::space_seasons(&mid2, c.as_ref()).unwrap_or_default();
            Ok((name, face, fans, card_total, rows, list_total, has_more, seasons))
        },
    )
    .await?;
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
    let (rows, list_total, has_more, name, _face, _fans, card_total) =
        super::blocking(move || -> Result<_, String> {
            let (rows, list_total, has_more) =
                crate::bilibili::space_videos(&mid, &order, pn, c.as_ref())?;
            let (name, _face, _fans, card_total) = crate::bilibili::space_card(&mid)?;
            Ok((rows, list_total, has_more, name, _face, _fans, card_total))
        })
        .await?;
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
    let (user, folders) = super::blocking(move || crate::bilibili::fav_folders(&c)).await?;
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
        super::blocking(move || crate::bilibili::fav_folder_videos(media_id, pn, &c)).await?;
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
    // "fav"：登录用户收藏夹（复用结果区 UI，id = media_id）——登录态先取好再进阻塞线程
    let sess = bili_cookies_req(&state).ok();
    let (rows, total, has_more) = super::blocking(move || -> Result<_, String> {
        match kind.as_str() {
            "season" => crate::bilibili::space_season_archives(&mid, id, 1, c.as_ref()),
            "fav" => {
                let sess = sess.ok_or_else(|| "请先扫码登录 B 站账号".to_string())?;
                crate::bilibili::fav_folder_videos(id, 1, &sess)
            }
            _ => crate::bilibili::space_series_archives(&mid, id, 1, c.as_ref()),
        }
    })
    .await?;
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
    let sess = bili_cookies_req(&state).ok();
    let pn = pn.max(1) as u32;
    let (rows, total, has_more) = super::blocking(move || -> Result<_, String> {
        match kind.as_str() {
            "season" => crate::bilibili::space_season_archives(&mid, id, pn, c.as_ref()),
            "fav" => {
                let sess = sess.ok_or_else(|| "请先扫码登录 B 站账号".to_string())?;
                crate::bilibili::fav_folder_videos(id, pn, &sess)
            }
            _ => crate::bilibili::space_series_archives(&mid, id, pn, c.as_ref()),
        }
    })
    .await?;
    Ok(json!({
        "total": total,
        "hasMore": has_more,
        "items": space_items(rows, ""),
    }))
}

/// 解析单个 B 站视频（当前结果展示，不落库）：每分P一条，rid = "BVxxx-cid"
#[tauri::command]
pub async fn bilibili_video_info(input: String) -> Result<Vec<BiliSpaceItem>, String> {
    let rows = super::blocking(move || crate::bilibili::video_rows(&input)).await?;
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
pub async fn bilibili_play(app: AppHandle, track: BiliPlayReq) -> Result<(), String> {
    let task_app = app.clone();
    super::blocking(move || bilibili_play_task(&task_app, track)).await
}

/// 同步任务体：音频直链解析是阻塞网络调用（经 blocking 在专用线程池执行）
fn bilibili_play_task(app: &AppHandle, track: BiliPlayReq) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (bvid, cid) = crate::bilibili::parse_rid(&track.rid)?;
    let (audio_url, quality) = crate::bilibili::audio_stream(&bvid, cid)?;
    // 字幕与音频并行预取：开播时歌词缓存已就绪
    spawn_bili_lyric_warmup(&state, track.rid.clone());
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
    super::record_online_play(&state, &info, "", false);
    engine_clone(&state).play_url(audio_url, info)
}
