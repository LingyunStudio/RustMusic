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
            "jyeffect",
            "sky",
            "jymaster",
        ] {
            match crate::netease::song_url(
                nid,
                Some(music_u.as_str()).filter(|s| !s.is_empty()),
                q,
            ) {
                Ok(Some((url, br, ext, level))) => {
                    let dec = verify_stream_decode(&url);
                    println!(
                        "  {q:<10} ✓ br={br:<6} ext={ext:<5} level={level:<10} {dec} url={}...",
                        &url[..url.len().min(50)]
                    );
                }
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
    let songs = crate::qq::search("周杰伦 晴天", 3, 1).expect("qq search failed");
    for s in songs.iter().take(2) {
        println!(
            "样曲 {} - {} vip={} mid={} mediaMid={} song_id={}",
            s.name, s.singer, s.vip, s.id, s.media_mid, s.song_id
        );
        for q in [
            "standard", "high", "lossless", "hires", "atmos_2", "atmos", "master",
        ] {
            match crate::qq::song_url(&s.id, &s.media_mid, &musicid, &musickey, q, s.vip) {
                Ok((url, ext, label)) => {
                    let dec = verify_stream_decode(&url);
                    println!("  {q:<10} 取链✓ [{label:<10}] ext={ext:<5} {dec}");
                }
                Err(e) => println!("  {q:<10} ✗ 取链失败: {e}"),
            }
        }
        println!("  ---");
    }

    // ---------- 酷狗 ----------
    println!("\n===== 酷狗 =====");
    let token = credential(&conn, "kg_token");
    let userid = setting(&conn, "kg_userid");
    let mid = setting(&conn, "kg_mid");
    let dfid = setting(&conn, "kg_dfid");
    crate::kugou::set_account(&token, &userid);
    if !mid.is_empty() && !dfid.is_empty() {
        crate::kugou::set_device(&mid, &dfid);
    }
    println!(
        "登录态 token: {}",
        if token.is_empty() { "（未登录）" } else { "已登录" }
    );
    let songs = crate::kugou::search("晴天 周杰伦", 1).expect("kugou search failed");
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
        for q in [
            "standard",
            "high",
            "lossless",
            "hires",
            "viper_clear",
            "viper_atmos",
            "viper_tape",
        ] {
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
                Ok((url, ext, label)) => {
                    let dec = verify_stream_decode(&url);
                    println!(
                        "  {q:<12} ✓ [{label:<10}] ext={ext:<5} {dec} url={}...",
                        &url[..url.len().min(48)]
                    );
                }
                Err(e) => println!("  {q:<12} ✗ {e}"),
            }
        }
        println!("  ---");
    }
}

/// 拉取流前 256KB 验证 HTTP 2xx + rodio 解码 + EqSource 多声道混音输出
fn verify_stream_decode(url: &str) -> String {
    use rodio::Source;
    use std::io::Read;
    let resp = ureq::get(url)
        .set("Range", "bytes=0-262143")
        .timeout(std::time::Duration::from_secs(10))
        .call();
    match resp {
        Ok(r) => {
            let status = r.status();
            let cr = r
                .header("content-range")
                .or_else(|| r.header("content-length"))
                .unwrap_or("?")
                .to_string();
            let mut bytes = Vec::new();
            let _ = r.into_reader().take(262144).read_to_end(&mut bytes);
            match rodio::Decoder::new(std::io::Cursor::new(bytes)) {
                Ok(dec) => {
                    let raw_sr = dec.sample_rate();
                    let raw_ch = dec.channels();
                    let eq = std::sync::Arc::new(crate::eq::EqShared::new([0.0; 10], false));
                    let pos_ms = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
                    let mut eq_src = crate::eq::EqSource::new(dec, eq, pos_ms);
                    let out_ch = eq_src.channels();
                    let samples_ok = eq_src.by_ref().take(512).count();
                    format!(
                        "HTTP {status} ({cr}) decode✓ raw={raw_sr}Hz/{raw_ch}ch -> out={out_ch}ch ({samples_ok} samples)"
                    )
                }
                Err(e) => format!("HTTP {status} ({cr}) decode✗ {e}"),
            }
        }
        Err(ureq::Error::Status(c, _)) => format!("HTTP {c}"),
        Err(e) => format!("传输错误: {e}"),
    }
}
