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
    // 正向样例：新歌有 Hi-Res 文件，验证真实 media_mid 的文件名格式
    // 孤勇者：QQ 明确提供 Hi-Res 的代表性曲目（正向验证样例）
    let songs = crate::qq::search("孤勇者", 6, 1).expect("qq search failed");
    // 详情请求带登录 cookie（匿名端点隐藏 Hi-Res 元数据）
    let qq_cookie = format!(
        "uin={musicid}; qqmusic_uin={musicid}; qm_keyst={musickey}; qqmusic_key={musickey}"
    );
    println!("  [详情] cookie len={}", qq_cookie.len());
    for s in songs.iter().take(3) {
        let songmid0 = s.id.clone();
        println!(
            "样曲 {} - {} vip={} mid={} mediaMid={} song_id={}",
            s.name, s.singer, s.vip, s.id, s.media_mid, s.song_id
        );
        // 匿名单曲详情：真实 media_mid + size_hires
        let detail_file = crate::qq::song_detail_file(&s.id, &qq_cookie).unwrap_or_default();
        let detail_mm = detail_file
            .get("media_mid")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let size_hires = detail_file.get("size_hires").and_then(|v| v.as_u64()).unwrap_or(0);
        println!("  [详情] media_mid={detail_mm} size_hires={size_hires}");
        // GetPlayUrlInfo 扫描：现代网页播放器端点，扫 quality 参数找 Hi-Res
        if !musickey.is_empty() {
            for qn in [1u8, 2, 3, 4, 5, 6, 7, 8] {
                let payload = serde_json::json!({
                    "comm": {"ct": 19, "cv": 1908, "uin": musicid},
                    "req_1": {
                        "module": "music.musichallSong.PlayUrlInfo.GetPlayUrlInfo",
                        "method": "GetPlayUrlInfo",
                        "param": {"songmid": [s.id], "quality": qn, "guid": "", "uis": ""}
                    }
                });
                if let Ok(v) = crate::qq::musicu_signed(&payload, Some(&qq_cookie)) {
                    let mi = v.pointer("/req_1/data/midurlinfo/0");
                    let fname = mi.and_then(|m| m.get("filename")).and_then(|f| f.as_str()).unwrap_or("");
                    let purl = mi.and_then(|m| m.get("purl")).and_then(|p| p.as_str()).unwrap_or("");
                    if !purl.is_empty() {
                        println!("  [PlayUrlInfo q={qn}] filename={fname} purl=…{}", &purl[purl.len().min(40)..]);
                    }
                }
            }
        }

        if size_hires > 0 {
            // 用真实 media_mid 实测三种文件名格式的 RS01 vkey + 下载
            for (tag, fname) in [
                ("media+mid", format!("RS01{detail_mm}{songmid0}")),
                ("media仅", format!("RS01{detail_mm}")),
                ("mid仅", format!("RS01{songmid0}")),
            ] {
                let url = crate::qq::hires_url_probe(&fname, &musicid, &musickey);
                match url {
                    Some(u) => {
                        let dl = ureq::get(&u).timeout(std::time::Duration::from_secs(8)).call();
                        let st = match &dl {
                            Ok(r) => format!("HTTP {}", r.status()),
                            Err(ureq::Error::Status(c, _)) => format!("HTTP {c}"),
                            Err(e) => format!("传输: {e}"),
                        };
                        println!("  [探针] {tag:<10} 下载: {st}");
                    }
                    None => println!("  [探针] {tag:<10} 无 purl"),
                }
            }
        }

        for q in ["standard", "high", "lossless", "hires"] {
            let payload = serde_json::json!({
                "comm": {"ct": 19, "cv": 1859, "uin": musicid},
                "req_1": {
                    "module": "music.musichallSong.SongInfoInter",
                    "method": "GetSongInfo",
                    "param": {"ids": [s.song_id], "info_is": 1}
                }
            });
            let detail = crate::qq::musicu_signed(&payload, Some(&credential(&conn, "qq_musickey")));
            match detail {
                Ok(v) => {
                    println!(
                        "  [详情] 原始 = {}",
                        serde_json::to_string(&v).unwrap_or_default().chars().take(500).collect::<String>()
                    );
                }
                Err(e) => println!("  [详情] ✗ {e}"),
            }
        }

        for q in ["standard", "high", "lossless", "hires"] {
            match crate::qq::song_url(&s.id, &s.media_mid, &musicid, &musickey, q, s.vip) {
                Ok((url, ext, label)) => {
                    // 播放时失败排查：取链成功后按引擎同款（无 Referer）实测可下载性
                    let dl = ureq::get(&url)
                        .timeout(std::time::Duration::from_secs(10))
                        .call();
                    let dl_status = match &dl {
                        Ok(r) => format!("HTTP {} ({} bytes)", r.status(), r.header("content-length").unwrap_or("?")),
                        Err(ureq::Error::Status(code, _)) => format!("HTTP {code}"),
                        Err(e) => format!("传输错误: {e}"),
                    };
                    println!("  {q:<10} 取链✓ [{label}] ext={ext} 下载: {dl_status}");
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
