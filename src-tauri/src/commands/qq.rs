//! QQ 音乐：搜索 / 播放取链 / 扫码登录与自动续期 / 歌词 / 榜单歌单

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db;
use crate::engine::TrackInfo;
use crate::lyrics;
use crate::models::LyricsPayload;
use crate::qq::{QqSession, RefreshError};
use crate::AppState;

use super::{engine_clone, notify_quality_fallback};

/// 进程内 QQ 会话缓存（避免每次点歌反复触发系统凭据管理器 IPC）
static SESSION_CACHE: std::sync::Mutex<Option<QqSession>> = std::sync::Mutex::new(None);
/// 续期互斥锁：并发请求同时撞上过期时串行化，后到线程直接复用已换新的 musickey，
/// 防止拿已作废的旧 refresh_key 二次请求把新凭证冲掉
static REFRESH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// 后台静默续期单飞行保护
static PROACTIVE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

fn not_logged_in_err() -> String {
    crate::i18n::pick(
        "未登录 QQ 音乐账号，无法获取播放链接，请先扫码登录",
        "Not signed in to QQ Music. Scan QR code to sign in first.",
    )
    .into()
}

/// 读取当前有效的 QQ 会话（内存缓存 → 系统凭据管理器 qq_session → 存量 qq_musickey 回退）
pub(crate) fn load_qq_session(state: &AppState) -> Option<QqSession> {
    if let Some(cached) = SESSION_CACHE.lock().unwrap().clone() {
        if cached.is_valid() {
            return Some(cached);
        }
    }
    let (db_musicid, db_euin, db_nick) = {
        let conn = state.db.lock();
        (
            db::get_setting(&conn, "qq_musicid").unwrap_or_default(),
            db::get_setting(&conn, "qq_encrypt_uin").unwrap_or_default(),
            db::get_setting(&conn, "qq_nickname").unwrap_or_default(),
        )
    };
    let mut sess = crate::credentials::get("qq_session")
        .and_then(|raw| serde_json::from_str::<QqSession>(&raw).ok())
        .unwrap_or_default();
    if sess.musicid.is_empty() {
        sess.musicid = db_musicid;
    }
    if sess.musickey.is_empty() {
        sess.musickey = crate::credentials::get("qq_musickey").unwrap_or_default();
    }
    if sess.encrypt_uin.is_empty() {
        sess.encrypt_uin = db_euin;
    }
    if sess.nickname.is_empty() {
        sess.nickname = db_nick;
    }
    if !sess.is_valid() {
        return None;
    }
    sess.login_type = QqSession::infer_login_type(sess.login_type, &sess.musickey);
    *SESSION_CACHE.lock().unwrap() = Some(sess.clone());
    Some(sess)
}

/// 将完整会话写入系统凭据管理器并同步非敏感展示字段到 SQLite
fn persist_qq_session(state: &AppState, sess: &QqSession) -> Result<(), String> {
    let raw = serde_json::to_string(sess).map_err(|e| e.to_string())?;
    crate::credentials::set("qq_session", &raw)?;
    // 同步维护历史键 qq_musickey，兼容测试审计与旧版本读取
    crate::credentials::set("qq_musickey", &sess.musickey)?;
    {
        let conn = state.db.lock();
        db::set_setting(&conn, "qq_musicid", &sess.musicid);
        if !sess.encrypt_uin.is_empty() {
            db::set_setting(&conn, "qq_encrypt_uin", &sess.encrypt_uin);
        }
        if !sess.nickname.is_empty() {
            db::set_setting(&conn, "qq_nickname", &sess.nickname);
        }
        let _ = db::set_setting(&conn, "qq_wx_refresh_token", "");
    }
    *SESSION_CACHE.lock().unwrap() = Some(sess.clone());
    Ok(())
}

/// 会话代际号：登出或新登录时递增，阻断在途后台续期把旧账号写回
static QQ_SESSION_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 清空本地 QQ 登录态（主动退出或续期令牌也被服务端判定失效时调用）
fn clear_qq_session(app: Option<&AppHandle>, state: &AppState, emit_expired: bool) {
    QQ_SESSION_GEN.fetch_add(1, Ordering::SeqCst);
    *SESSION_CACHE.lock().unwrap() = None;
    crate::credentials::clear("qq_session");
    crate::credentials::clear("qq_musickey");
    {
        let conn = state.db.lock();
        db::set_setting(&conn, "qq_musicid", "");
        db::set_setting(&conn, "qq_encrypt_uin", "");
        db::set_setting(&conn, "qq_nickname", "");
        db::set_setting(&conn, "qq_wx_refresh_token", "");
    }
    if emit_expired {
        if let Some(h) = app {
            let _ = h.emit("qq://session-expired", json!({}));
        }
    }
}

/// 串行化执行一次 QQ 登录态续期。
/// - `expected_old_key`：发起方当时持有的旧 musickey；若锁内发现已被并发线程换新则直接复用。
/// - `clear_on_unrecoverable`：仅在业务接口已实锤 104009/1000 过期、且续期也明确不可恢复时
///   置 true 清理失效登录态；后台预刷或断网瞬时失败绝不清本地凭据。
fn refresh_qq_session_locked(
    app: Option<&AppHandle>,
    state: &AppState,
    expected_old_key: Option<&str>,
    clear_on_unrecoverable: bool,
) -> Result<QqSession, RefreshError> {
    let _guard = REFRESH_LOCK.lock().unwrap();
    let Some(cur) = load_qq_session(state) else {
        return Err(RefreshError::MissingRefreshToken);
    };
    let now = crate::qq::now_secs();
    if let Some(old_key) = expected_old_key {
        if cur.musickey != old_key && !cur.is_expired_at(now) {
            return Ok(cur);
        }
    }
    let gen = QQ_SESSION_GEN.load(Ordering::SeqCst);
    match crate::qq::refresh_session(&cur) {
        Ok(fresh) => {
            // 若刷新期间用户已登出或切换新账号，丢弃过期续期结果
            if QQ_SESSION_GEN.load(Ordering::SeqCst) != gen {
                return Err(RefreshError::MissingRefreshToken);
            }
            if let Err(e) = persist_qq_session(state, &fresh) {
                eprintln!("[qq] 续期成功但保存凭据失败: {e}");
            } else {
                eprintln!(
                    "[qq] 登录凭证自动续期成功（musicid={}, login_type={}, ttl={}s）",
                    fresh.musicid, fresh.login_type, fresh.key_expires_in
                );
            }
            Ok(fresh)
        }
        Err(err) => {
            if clear_on_unrecoverable
                && matches!(
                    err,
                    RefreshError::AuthExpired(_, _) | RefreshError::MissingRefreshToken
                )
            {
                eprintln!("[qq] 登录态已过期且无法自动续期（{err:?}），需重新扫码");
                clear_qq_session(app, state, true);
            } else {
                eprintln!("[qq] 登录凭证续期未通过（保留本地凭据待下次重试）：{err:?}");
            }
            Err(err)
        }
    }
}

/// 触发一次非阻塞后台预续期（剩余寿命过半但尚未真正过期时调用）
fn maybe_spawn_proactive_refresh(app: AppHandle, old_musickey: String) {
    if PROACTIVE_IN_FLIGHT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    std::thread::spawn(move || {
        struct ResetGuard;
        impl Drop for ResetGuard {
            fn drop(&mut self) {
                PROACTIVE_IN_FLIGHT.store(false, Ordering::SeqCst);
            }
        }
        let _reset = ResetGuard;
        let state = app.state::<AppState>();
        let _ = refresh_qq_session_locked(Some(&app), &state, Some(&old_musickey), false);
    });
}

/// 启动常驻后台保活线程：启动延迟 8 秒首检，此后每 6 小时巡检一次；
/// 解决长期最小化到系统托盘不重启、72 小时后 musickey 自然过期的问题。
pub fn spawn_qq_keepalive(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(8));
        loop {
            {
                let state = app.state::<AppState>();
                if let Some(sess) = load_qq_session(&state) {
                    let now = crate::qq::now_secs();
                    if sess.needs_proactive_refresh(now) {
                        let _ = refresh_qq_session_locked(
                            Some(&app),
                            &state,
                            Some(&sess.musickey),
                            false,
                        );
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(6 * 3600));
        }
    });
}

/// 获取可用的 `(musicid, musickey)`：
/// - 本地时间戳已过期且有刷新令牌 → 立即同步换新后再返回；
/// - 尚未过期但剩余寿命 ≤ 36h → 立即返回当前可用凭据，同时派生后台静默换新。
fn qq_credential_fresh(app: &AppHandle) -> Result<(String, String), String> {
    let state = app.state::<AppState>();
    let sess = load_qq_session(&state).ok_or_else(not_logged_in_err)?;
    let now = crate::qq::now_secs();
    if sess.is_expired_at(now) && sess.can_refresh() {
        if let Ok(fresh) =
            refresh_qq_session_locked(Some(app), &state, Some(&sess.musickey), false)
        {
            return Ok((fresh.musicid, fresh.musickey));
        }
    } else if sess.needs_proactive_refresh(now) {
        maybe_spawn_proactive_refresh(app.clone(), sess.musickey.clone());
    }
    Ok((sess.musicid, sess.musickey))
}

/// 统一封装需登录的 QQ 业务调用：预刷保鲜 + 遇到服务端报登录过期（104009/1000）时
/// 自动强制续期并透明重放一次请求（播放、歌单、逐字歌词、下载全链路共用）。
pub(crate) fn qq_with_auto_refresh<T, F>(app: &AppHandle, mut op: F) -> Result<T, String>
where
    F: FnMut(&str, &str) -> Result<T, String>,
{
    let (musicid, musickey) = qq_credential_fresh(app)?;
    match op(&musicid, &musickey) {
        Ok(v) => Ok(v),
        Err(e) if crate::qq::is_session_expired_err(&e) => {
            let state = app.state::<AppState>();
            match refresh_qq_session_locked(Some(app), &state, Some(&musickey), true) {
                Ok(fresh) => op(&fresh.musicid, &fresh.musickey),
                Err(_) => Err(e),
            }
        }
        Err(other) => Err(other),
    }
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

#[tauri::command]
pub async fn qq_search(keyword: String, page: Option<i64>) -> Result<serde_json::Value, String> {
    let songs = super::blocking(move || crate::qq::search(&keyword, 30, page.unwrap_or(1))).await?;
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
    let quality_setting = {
        let conn = state.db.lock();
        super::source_quality(&conn, "qq")
    };
    let quality = super::resolve_quality("qq", &quality_setting);
    let (url, _ext, quality_label) = qq_with_auto_refresh(app, |musicid, musickey| {
        crate::qq::song_url(
            &track.songmid,
            &track.media_mid,
            musicid,
            musickey,
            &quality,
            track.vip,
        )
    })?;
    notify_quality_fallback(
        app,
        "QQ 音乐",
        &quality_setting,
        quality_label,
        crate::i18n::pick(
            "账号权益未含该音质或该曲无对应音源版本",
            "membership doesn't include this quality or no matching track version exists",
        ),
    );
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
    super::record_online_play(&state, &info, &track.media_mid, track.vip);
    engine_clone(&state).play_url(url, info)
}

#[tauri::command]
pub async fn qq_lyric(app: AppHandle, songmid: String) -> Result<LyricsPayload, String> {
    let state = app.state::<AppState>();
    let cached = {
        let conn = state.db.lock();
        db::get_lyrics_cache(&conn, "qq", &songmid)
    };
    if let Some(json) = cached {
        if let Ok(p) = serde_json::from_str::<LyricsPayload>(&json) {
            return Ok(p);
        }
    }
    // 优先逐字歌词（QRC，需登录且支持过期自动续期）；失败回落匿名行级接口
    let task_app = app.clone();
    let songmid_for_task = songmid.clone();
    let payload = super::blocking(move || -> Result<LyricsPayload, String> {
        let songmid = songmid_for_task;
        let has_login = {
            let st = task_app.state::<AppState>();
            load_qq_session(&st).is_some()
        };
        if has_login {
            if let Ok(Some((enhanced, trans))) = qq_with_auto_refresh(&task_app, |musicid, musickey| {
                crate::qq::lyric_qrc(&songmid, musicid, musickey)
            }) {
                let mut p = lyrics::parse(&enhanced);
                if p.synced {
                    if let Some(t) = &trans {
                        lyrics::attach_translations(&mut p, &t);
                    }
                    let mut payload = LyricsPayload::new(p.synced, p.lines);
                    payload.source = Some("逐字 QRC".into());
                    return Ok(payload);
                }
            }
            eprintln!("[qq] {songmid} QRC 不可用（无权益/解密失败），回落行级 LRC");
        }
        let lt = crate::qq::lyric(&songmid)?;
        let Some(lt) = lt else {
            return Ok(LyricsPayload::new(false, vec![]));
        };
        let mut p = lyrics::parse(&lt.lrc);
        if let Some(t) = &lt.trans {
            lyrics::attach_translations(&mut p, t);
        }
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

/// 登录成功后把完整会话凭证写入系统凭据管理器与本机设置（QQ 扫码与微信扫码共用）
fn save_qq_credential(state: &State<AppState>, r: &crate::qq::QqQrCheck) -> Result<(), String> {
    if r.status != "success" {
        return Ok(());
    }
    let sess = if let Some(s) = &r.session {
        let mut s = s.clone();
        if s.nickname.is_empty() {
            if let Some(nick) = &r.nickname {
                s.nickname = nick.clone();
            }
        }
        Some(s)
    } else if let (Some(musicid), Some(musickey)) = (&r.musicid, &r.musickey) {
        Some(QqSession {
            musicid: musicid.clone(),
            musickey: musickey.clone(),
            login_type: QqSession::infer_login_type(0, musickey),
            refresh_token: r.refresh_token.clone().unwrap_or_default(),
            musickey_create_time: crate::qq::now_secs(),
            key_expires_in: QqSession::DEFAULT_KEY_EXPIRES_IN,
            encrypt_uin: r.encrypt_uin.clone().unwrap_or_default(),
            nickname: r.nickname.clone().unwrap_or_default(),
            ..Default::default()
        })
    } else {
        None
    };

    if let Some(s) = sess {
        QQ_SESSION_GEN.fetch_add(1, Ordering::SeqCst);
        persist_qq_session(state, &s).map_err(|e| {
            crate::i18n::pick(
                format!("登录成功但凭据保存失败: {e}"),
                format!("Signed in, but failed to save credentials: {e}"),
            )
        })?;
    }
    Ok(())
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
    save_qq_credential(&state, &r)?;
    Ok(json!({ "status": r.status, "nickname": r.nickname }))
}

#[tauri::command]
pub async fn qq_wx_qr_create() -> Result<serde_json::Value, String> {
    let r = super::blocking(crate::qq::wx_qr_create).await?;
    Ok(json!({ "uuid": r.uuid, "qr": r.qr }))
}

/// 微信扫码状态：一次调用 = 一次长轮询（服务端挂起约 15s），前端循环调用
#[tauri::command]
pub async fn qq_wx_qr_check(
    state: State<'_, AppState>,
    uuid: String,
) -> Result<serde_json::Value, String> {
    let r = super::blocking(move || crate::qq::wx_qr_check(&uuid)).await?;
    save_qq_credential(&state, &r)?;
    Ok(json!({ "status": r.status, "nickname": r.nickname }))
}

#[tauri::command]
pub async fn qq_status(app: AppHandle) -> Result<serde_json::Value, String> {
    let task_app = app.clone();
    super::blocking(move || {
        let state = task_app.state::<AppState>();
        let Some(sess) = load_qq_session(&state) else {
            return Ok(json!({ "loggedIn": false, "nickname": "" }));
        };
        let now = crate::qq::now_secs();
        if sess.is_expired_at(now) {
            if sess.can_refresh() {
                match refresh_qq_session_locked(
                    Some(&task_app),
                    &state,
                    Some(&sess.musickey),
                    false,
                ) {
                    Ok(fresh) => {
                        return Ok(json!({ "loggedIn": true, "nickname": fresh.nickname }));
                    }
                    Err(RefreshError::AuthExpired(_, _)) => {
                        if crate::qq::check_session_expired(&sess.musicid, &sess.musickey)
                            == Some(true)
                        {
                            clear_qq_session(Some(&task_app), &state, false);
                            return Ok(json!({ "loggedIn": false, "nickname": "" }));
                        }
                    }
                    Err(_) => {}
                }
            } else if crate::qq::check_session_expired(&sess.musicid, &sess.musickey) == Some(true)
            {
                clear_qq_session(Some(&task_app), &state, false);
                return Ok(json!({ "loggedIn": false, "nickname": "" }));
            }
        } else if sess.needs_proactive_refresh(now) {
            maybe_spawn_proactive_refresh(task_app.clone(), sess.musickey.clone());
        }
        Ok(json!({ "loggedIn": true, "nickname": sess.nickname }))
    })
    .await
}

#[tauri::command]
pub async fn qq_logout(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    clear_qq_session(Some(&app), &state, false);
    Ok(())
}

// ---------- 榜单 / 随机推荐（网易云、QQ，匿名可用） ----------

#[tauri::command]
pub async fn qq_toplists() -> Result<serde_json::Value, String> {
    let toplists = super::blocking(crate::qq::toplists).await?;
    Ok(json!({ "toplists": toplists }))
}

#[tauri::command]
pub async fn qq_toplist_tracks(
    top_id: i64,
    page: Option<i64>,
) -> Result<serde_json::Value, String> {
    let page = page.unwrap_or(1).max(1);
    let songs = super::blocking(move || crate::qq::toplist_tracks(top_id, page)).await?;
    Ok(json!({ "songs": songs, "hasMore": songs.len() as i64 >= 100 }))
}

#[tauri::command]
pub async fn qq_random_playlist() -> Result<crate::qq::QqPublicPlaylist, String> {
    super::blocking(crate::qq::random_playlist).await
}

#[tauri::command]
pub async fn qq_user_playlists(app: AppHandle) -> Result<Vec<crate::models::UserPlaylistMeta>, String> {
    let task_app = app.clone();
    super::blocking(move || {
        qq_with_auto_refresh(&task_app, |musicid, musickey| {
            crate::qq::user_playlists(musicid, musickey)
        })
    })
    .await
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
    let stored_euin = {
        let conn = state.db.lock();
        db::get_setting(&conn, "qq_encrypt_uin").unwrap_or_default()
    };
    let songs = qq_with_auto_refresh(app, |musicid, musickey| {
        crate::qq::playlist_tracks(
            remote_pid,
            musicid,
            musickey,
            &crate::qq::encrypt_uin_of(musicid, &stored_euin),
        )
    })?;
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
