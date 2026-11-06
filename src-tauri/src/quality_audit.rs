//! 音质档位真实取链审计（issue 跟进："有的音质无法获取"）。
//! 运行：`cargo test quality_audit -- --ignored --nocapture`
//! 读取本机应用的真实登录态（系统凭据管理器 + library.db 的在线播放记录），
//! 用真实曲目逐档请求取链，打印"档位 → 实际交付"矩阵。
#![cfg(test)]


fn app_conn() -> rusqlite::Connection {
    let db_path = std::env::var("APPDATA").unwrap() + r"\com.rustmusic.app\library.db";
    rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("打开应用数据库失败")
}

fn setting(conn: &rusqlite::Connection, key: &str) -> String {
    conn.query_row(
        "SELECT value FROM settings WHERE key=?1",
        [key],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_default()
}

/// 凭据：系统凭据管理器优先，回落 settings 表（存量明文）
fn credential(conn: &rusqlite::Connection, key: &str) -> String {
    crate::credentials::get(key).unwrap_or_else(|| setting(conn, key))
}

/// 在线播放记录里最近一首指定源的曲目（rid / mediaMid / vip / 标题）
fn last_online(conn: &rusqlite::Connection, kind: &str) -> Option<(String, String, i64, String)> {
    conn.query_row(
        "SELECT rid, COALESCE(media_mid,''), COALESCE(vip,0), title FROM online_tracks WHERE kind=?1 ORDER BY last_played DESC LIMIT 1",
        [kind],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .ok()
}

#[test]
#[ignore = "真实网络审计：cargo test quality_audit -- --ignored --nocapture"]
fn quality_audit_all_sources() {
    let conn = app_conn();

    // ---------- 网易云 ----------
    println!("\n===== 网易云 =====");
    let music_u = credential(&conn, "netease_music_u");
    println!(
        "登录态 MUSIC_U: {}",
        if music_u.is_empty() { "（未登录）" } else { "已登录" }
    );
    // 最近 3 首网易云播放记录：区分"歌曲本身无高档"与"取链失败"
    let nids: Vec<i64> = {
        let mut st = conn
            .prepare(
                "SELECT rid FROM online_tracks WHERE kind='netease' ORDER BY last_played DESC LIMIT 3",
            )
            .unwrap();
        let rows = st
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|s| s.ok().and_then(|v| v.parse().ok()))
            .collect::<Vec<_>>();
        if rows.is_empty() {
            vec![347230]
        } else {
            rows
        }
    };
    for nid in nids {
    println!("样曲 nid={nid}");
    for q in [
        "standard",
        "higher",
        "high",
        "lossless",
        "hires",
        "jymaster",
        "sky",
    ] {
        match crate::netease::song_url(nid, Some(music_u.as_str()).filter(|s| !s.is_empty()), q) {
            Ok(Some((url, br, ext, level))) => println!(
                "  {q:<10} ✓ br={br} ext={ext} level={level:?} url={}",
                &url[..url.len().min(55)]
            ),
            Ok(None) => println!("  {q:<10} ✗ 无链接"),
            Err(e) => println!("  {q:<10} ✗ {e}"),
        }
    }
    }

    // ---------- QQ ----------
    println!("\n===== QQ 音乐 =====");
    // 测试进程没有跑应用 setup：guid 需从本机库初始化（空 guid 会让 vkey 全空）
    crate::qq::init_guid(&conn);
    let musickey = credential(&conn, "qq_musickey");
    let musicid = setting(&conn, "qq_musicid");
    println!(
        "登录态 musickey: {}",
        if musickey.is_empty() { "（未登录）" } else { "已登录" }
    );
    let songs = crate::qq::search("晴天", 6, 1).expect("qq search failed");
    for s in songs.iter().take(3) {
        println!(
            "样曲 {} - {} vip={} mid={} mediaMid={}",
            s.name, s.singer, s.vip, s.id, s.media_mid
        );
        for q in ["standard", "high", "lossless", "hires"] {
            match crate::qq::song_url(&s.id, &s.media_mid, &musicid, &musickey, q, s.vip) {
                Ok((url, ext, label)) => println!(
                    "  {q:<10} ✓ [{label}] ext={ext} url={}",
                    &url[..url.len().min(55)]
                ),
                Err(e) => println!("  {q:<10} ✗ {e}"),
            }
        }
        println!("  ---");
    }

    // ---------- 酷狗 ----------
    println!("\n===== 酷狗 =====");
    let token = credential(&conn, "kg_token");
    let userid = setting(&conn, "kg_userid");
    println!(
        "登录态 token: {}",
        if token.is_empty() { "（未登录）" } else { "已登录" }
    );
    let songs = crate::kugou::search("晴天", 1).expect("kugou search failed");
    for s in songs.iter().take(2) {
        println!(
            "样曲 {} - {} vip={} hash={}",
            s.name, s.singer, s.vip, s.id
        );
        println!(
            "  自带 hash: hq={} sq={} super={} album_audio_id={} album_id={}",
            s.hq_hash.len(),
            s.sq_hash.len(),
            s.super_hash.len(),
            s.album_audio_id,
            s.album_id
        );
        for q in ["standard", "high", "lossless", "hires"] {
            match crate::kugou::song_url(
                &s.id,
                s.album_audio_id,
                s.album_id,
                &s.hq_hash,
                &s.sq_hash,
                &s.super_hash,
                s.vip,
                &token,
                &userid,
                q,
            ) {
                Ok((url, ext, label)) => println!(
                    "  {q:<10} ✓ [{label}] ext={ext} url={}",
                    &url[..url.len().min(55)]
                ),
                Err(e) => println!("  {q:<10} ✗ {e}"),
            }
        }
        println!("  ---");
    }
}
