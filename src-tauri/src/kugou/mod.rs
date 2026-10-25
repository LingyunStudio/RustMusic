//! 酷狗音乐接口客户端（搜索 / 播放链接 / KRC 逐字歌词 / 排行榜 / 歌单广场 / 扫码登录）
//!
//! 匿名访问其公共接口：免费曲目直接返回可播直链；付费/VIP 曲目需用户
//! 扫码登录自己的账号后按会员权益获取播放链接，不包含任何绕过付费 /
//! 版权限制的功能。
//!
//! 接口协议参考开源实现 MakcRe/KuGouMusicApi（MIT）：
//! - 网关请求（gateway.kugou.com + x-router）：Android 客户端参数 +
//!   MD5 签名（参数排序 k=v 拼接 + 盐）；
//! - 老明文接口（搜索/榜单/歌词/播放信息）：免签名直连；
//! - 登录（login-user.kugou.com）：Web 端盐值签名。
//!
//! 设备标识（guid/mid/dfid）为进程内随机生成，仅用于标识本次安装，
//! 不涉及任何用户隐私数据。

use base64::Engine as _;
use flate2::read::ZlibDecoder;
use md5::{Digest as Md5Digest, Md5};
use num_bigint::BigUint;
use rand::Rng;
use serde::Serialize;
use std::io::Read;
use std::time::Duration;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
/// 网关接口要求的安卓客户端 UA
const ANDROID_UA: &str = "Android15-1070-11083-46-0-DiscoveryDRADProtocol-wifi";
const TIMEOUT: Duration = Duration::from_secs(12);
const SALT_ANDROID: &str = "OIlwieks28dk2k092lksi2UIkp";
const SALT_WEB: &str = "NVPh5oo715z5DIWAeQlhMDsWXXQV4hwt";
/// /v5/url 的 key 盐（tracker 取链接签名）
const SALT_URL_KEY: &str = "57ae12eb6890223e355ccfcb74edf70d";
const APPID: &str = "1005";
const CLIENTVER: &str = "20489";

fn md5_hex(s: &str) -> String {
    let mut h = Md5::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}


mod browse;
mod device;
mod lyrics;
mod search;
mod url;
mod user;
pub use browse::{playlist_tracks, random_playlist, toplist_tracks, toplists};
pub use device::{current_dfid, current_mid, register_dev, set_account, set_device};
pub use lyrics::lyric;
pub use search::{enrich_hashes, search};
pub use url::{last_hires_error, song_url};
pub use user::{KgUserPlaylist, qr_check, qr_create, user_playlists};
use device::*;
// ---------- HTTP 基础 ----------

fn form_encode(s: &str) -> String {
    // RFC3986 unreserved（字母数字与 - _ . ~）不编码，与官方客户端一致；
    // 之前的 NON_ALPHANUMERIC 会把 _ 编码成 %5F，酷狗网关按原始 query
    // 验签，直接报参数错误（err appid/clientver/mid/dfid）
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 读取响应体文本
fn read_body(resp: ureq::Response) -> Result<String, String> {
    let mut buf = String::new();
    resp.into_reader()
        .read_to_string(&mut buf)
        .map_err(|e| format!("酷狗响应读取失败: {e}"))?;
    Ok(buf)
}

fn parse_json(text: &str, what: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(text).map_err(|e| {
        format!(
            "{what}解析失败: {e} | body: {}",
            text.chars().take(150).collect::<String>()
        )
    })
}

/// GET 并返回响应体文本（非 2xx 时也读取响应体，便于错误分类）
fn get_text(url: &str, referer: &str, ua: &str) -> Result<String, String> {
    match ureq::get(url)
        .set("User-Agent", ua)
        .set("Referer", referer)
        .timeout(TIMEOUT)
        .call()
    {
        Ok(resp) => read_body(resp),
        Err(ureq::Error::Status(_, resp)) => read_body(resp),
        Err(e) => Err(format!("请求酷狗接口失败: {e}")),
    }
}

fn get_json(url: &str, referer: &str, ua: &str, what: &str) -> Result<serde_json::Value, String> {
    let text = get_text(url, referer, ua)?;
    parse_json(&text, what)
}

// ---------- 签名与网关请求 ----------

/// Android 网关签名：盐 + "k=v"（按 key 排序，重复 key 保持出现顺序）拼接
/// + 请求体 + 盐 取 MD5。
/// 注意必须对 key 排序而非对整个 "k=v" 串排序：/v5/url 同时带设备
/// clientver 与取链 clientver 两个同名参数，按值排序会得到不同顺序，
/// 服务端验签直接报参数错误。
fn sign_android(pairs: &[(String, String)], data: &str) -> String {
    let mut kv: Vec<(String, String)> = pairs.to_vec();
    kv.sort_by(|a, b| a.0.cmp(&b.0)); // sort_by 稳定：重复 key 保持插入顺序
    let s: String = kv.iter().map(|(k, v)| format!("{k}={v}")).collect();
    md5_hex(&format!("{SALT_ANDROID}{s}{data}{SALT_ANDROID}"))
}

/// Web 端签名（登录接口）：盐 + 排序 "k=v" 拼接 + 盐
fn sign_web(pairs: &[(String, String)]) -> String {
    let mut kv: Vec<(String, String)> = pairs.to_vec();
    kv.sort_by(|a, b| a.0.cmp(&b.0));
    let s: String = kv.iter().map(|(k, v)| format!("{k}={v}")).collect();
    md5_hex(&format!("{SALT_WEB}{s}{SALT_WEB}"))
}

/// 网关请求（gateway.kugou.com）：自动注入设备参数并计算 signature。
/// `router` 为 x-router 目标服务；`body` 非 None 时发 POST JSON；
/// `extra_headers` 为附加协议头（如 kmr 接口的 kg-tid）。
fn gateway(
    path: &str,
    router: &str,
    extra: &[(String, String)],
    body: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    gateway_ex(path, router, extra, body, &[])
}

fn gateway_ex(
    path: &str,
    router: &str,
    extra: &[(String, String)],
    body: Option<&serde_json::Value>,
    extra_headers: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    let dfid = dfid_of();
    let clienttime = now_secs().to_string();
    let mut pairs: Vec<(String, String)> = vec![
        ("dfid".into(), dfid.clone()),
        ("mid".into(), mid_of()),
        ("uuid".into(), "-".into()),
        ("appid".into(), APPID.into()),
        ("clientver".into(), CLIENTVER.into()),
        ("clienttime".into(), clienttime.clone()),
    ];
    // extra 按名覆盖默认参数（与参考实现的 Object.assign 语义一致）：
    // /v5/url 的取链 clientver=11430 必须覆盖设备默认 20489，否则出现
    // 重复参数，服务端直接报参数错误
    for (k, v) in extra.iter() {
        match pairs.iter_mut().find(|(ek, _)| ek == k) {
            Some(slot) => slot.1 = v.clone(),
            None => pairs.push((k.clone(), v.clone())),
        }
    }
    let data = body
        .map(|b| serde_json::to_string(b).unwrap_or_default())
        .unwrap_or_default();
    pairs.push(("signature".into(), sign_android(&pairs, &data)));

    let query = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", form_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let url = format!("https://gateway.kugou.com{path}?{query}");

    let mut req = match body {
        Some(_) => ureq::post(&url),
        None => ureq::get(&url),
    }
    .set("User-Agent", ANDROID_UA)
    .set("dfid", &dfid)
    .set("clienttime", &clienttime)
    .set("mid", &mid_of())
    .set("kg-rc", "1")
    .set("kg-thash", "5d816a0")
    .set("kg-rec", "1")
    .set("kg-rf", "B9EDA08A64250DEFFBCADDEE00F8F25F")
    .timeout(TIMEOUT);
    if !router.is_empty() {
        req = req.set("x-router", router);
    }
    for (k, v) in extra_headers {
        req = req.set(k, v);
    }
    let resp = if let Some(b) = body {
        req.set("Content-Type", "application/json")
            .send_string(&serde_json::to_string(b).unwrap_or_default())
    } else {
        req.call()
    };
    let text = match resp {
        Ok(r) => read_body(r)?,
        Err(ureq::Error::Status(_, r)) => read_body(r)?,
        Err(e) => return Err(format!("酷狗网关请求失败: {e}")),
    };
    let v = parse_json(&text, "酷狗网关响应")?;
    if v.get("status").and_then(|s| s.as_i64()) == Some(0) {
        let msg = v
            .get("error_msg")
            .or_else(|| v.get("errmsg"))
            .or_else(|| v.get("message"))
            .or_else(|| v.get("error"))
            .and_then(|m| m.as_str())
            .unwrap_or("");
        let code = v.get("error_code").and_then(|c| c.as_i64()).unwrap_or(0);
        return Err(format!("酷狗接口请求失败（{code} {msg}）"));
    }
    Ok(v)
}

// ---------- 数据模型 ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KgSong {
    /// 歌曲 hash（酷狗的曲目唯一标识，128k 档）
    pub id: String,
    pub name: String,
    pub singer: String,
    pub album: String,
    pub duration_ms: u64,
    pub cover: String,
    /// 付费/VIP 曲目：匿名不可播，界面上打 VIP 角标
    pub vip: bool,
    /// 专辑音频 ID（取链接 / 歌词检索要用）
    #[serde(default)]
    pub album_audio_id: u64,
    #[serde(default)]
    pub album_id: u64,
    /// 高音质文件 hash（搜索/榜单通道附带；匿名取不到链接，登录后可用）
    #[serde(default)]
    pub hq_hash: String,
    #[serde(default)]
    pub sq_hash: String,
    #[serde(default)]
    pub super_hash: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KgToplist {
    pub id: i64,
    pub name: String,
    pub pic: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KgPublicPlaylist {
    /// 歌单 ID（global_collection_id，字符串）
    pub id: String,
    pub name: String,
    pub cover: String,
    pub play_count: i64,
    pub creator: String,
    pub songs: Vec<KgSong>,
    /// 服务端声称的曲目总数（0 = 未知；详情拉取后与 songs.len() 对照可发现截断）
    #[serde(default)]
    pub total_count: i64,
}

/// 数值字段兼容解析（酷狗各接口数字/字符串混用）
fn json_num(v: Option<&serde_json::Value>) -> u64 {
    match v {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

fn cover_sized(url: &str, size: u32) -> String {
    if url.is_empty() {
        String::new()
    } else {
        url.replace("{size}", &size.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::url::url_ext;

    #[test]
    fn sign_vectors() {
        // 与参考实现（MakcRe/KuGouMusicApi helper.js）交叉验证的向量
        let pairs = vec![
            ("appid".to_string(), "1005".to_string()),
            ("clienttime".to_string(), "1790000000".to_string()),
            ("clientver".to_string(), "20489".to_string()),
            ("dfid".to_string(), "abc123".to_string()),
            ("mid".to_string(), "12345".to_string()),
            ("uuid".to_string(), "-".to_string()),
        ];
        assert_eq!(sign_android(&pairs, ""), "58cfefe77313a1706ffadca4f3b4ff06");
        let pairs2 = vec![
            ("appid".to_string(), "1005".to_string()),
            ("clientver".to_string(), "20489".to_string()),
        ];
        assert_eq!(
            sign_android(&pairs2, "{\"a\":1}"),
            "345153e763985ede8f4eb4369b6f9b0c"
        );
        let pairs3 = vec![
            ("appid".to_string(), "1001".to_string()),
            ("plat".to_string(), "4".to_string()),
            ("srcappid".to_string(), "2919".to_string()),
            ("clienttime".to_string(), "1790000000".to_string()),
        ];
        assert_eq!(sign_web(&pairs3), "62607c708107bad91f0b9249d0a9b87d");
    }

    #[test]
    fn mid_from_guid() {
        // guid=550e8400-e29b-41d4-a716-446655440000 → md5 e8456cee... → 十进制
        let guid_md5 = md5_hex("550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(guid_md5, "e8456cee7fd2348cfa239342b10a23bd");
        let mid = BigUint::parse_bytes(guid_md5.as_bytes(), 16)
            .map(|n| n.to_string())
            .unwrap();
        assert_eq!(mid, "308741372901437977228425563242293240765");
    }

    #[test]
    fn url_ext_cases() {
        assert_eq!(url_ext("https://a.b/x/file.mp3?k=v"), "mp3");
        assert_eq!(url_ext("https://a.b/x/file"), "mp3");
        assert_eq!(url_ext("https://a.b/x/file.flac"), "flac");
    }

    #[test]
    fn test_search_and_lyric() {
        let songs = search("晴天", 1).unwrap();
        assert!(!songs.is_empty());
        assert!(!songs[0].id.is_empty());
        // 付费曲目报“需要 VIP/付费”，免费曲目拿到直链
        let free = songs.iter().find(|s| !s.vip).expect("有免费曲目");
        let (url, _ext, _label) = song_url(
            &free.id,
            free.album_audio_id,
            free.album_id,
            &free.hq_hash,
            &free.sq_hash,
            &free.super_hash,
            false,
            "",
            "",
            "high",
        )
        .unwrap();
        assert!(url.starts_with("http"));
        let lyric = lyric(&free.id).unwrap();
        let _ = lyric; // 有无歌词均可（有词时应包含时间标签）
    }

    #[test]
    #[ignore] // 需要网络
    fn test_krc_word_lyrics() {
        // 热门歌曲应有 KRC 逐字歌词：歌词接口返回增强 LRC，解析出词级时间轴
        let songs = search("晴天", 1).unwrap();
        let free = songs.iter().find(|s| !s.vip).expect("有免费曲目");
        let lrc = lyric(&free.id).unwrap().expect("有歌词");
        let p = crate::lyrics::parse(&lrc);
        assert!(p.synced);
        assert!(
            p.lines.iter().any(|l| l.words.is_some()),
            "应为 KRC 逐字歌词（增强 LRC 带词级时间轴）"
        );
        // 词时间必须是绝对毫秒（词首 ≥ 行首）：线上 KRC 词时间是相对行首的
        // 偏移，转换器漏加行首的话词首全落在 0 附近，前端逐词染色失效
        for l in p.lines.iter() {
            let Some(words) = l.words.as_ref() else {
                continue;
            };
            let Some(first) = words.iter().find(|w| !w.text.is_empty()) else {
                continue;
            };
            assert!(
                first.start_ms + 100 >= l.time_ms.unwrap_or(0),
                "词时间应为绝对毫秒：行首 {:?}，词首 {}（{}）",
                l.time_ms,
                first.start_ms,
                l.text
            );
        }
    }

    #[test]
    #[ignore] // 需要网络 + 本机应用数据库
    fn test_full_app_path_with_real_account() {
        // 直接复现应用路径：读本机库里的登录凭证 → 注册/注入 dfid →
        // 搜索 → 挑 VIP 曲目 → 按当前音质取链接
        let db_path = std::env::var("APPDATA").unwrap() + r"\com.rustmusic.app\library.db";
        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("打开应用数据库失败");
        let get = |k: &str| -> String {
            conn.query_row("SELECT value FROM settings WHERE key = ?1", [k], |r| {
                r.get::<_, String>(0)
            })
            .unwrap_or_default()
        };
        let token = get("kg_token");
        let userid = get("kg_userid");
        let dfid = get("kg_dfid");
        println!("userid={userid} token_len={} dfid={dfid}", token.len());
        assert!(!token.is_empty(), "本机应有酷狗登录凭证（先扫码登录）");
        set_account(&token, &userid);
        let (stored_mid, stored_dfid) = {
            let m = conn
                .query_row("SELECT value FROM settings WHERE key = 'kg_mid'", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap_or_default();
            (m, dfid)
        };
        if stored_mid.is_empty() || stored_dfid.is_empty() {
            set_device("", "");
            let d = register_dev().expect("register_dev failed");
            println!("new mid={} dfid={d}", current_mid());
        } else {
            set_device(&stored_mid, &stored_dfid);
            println!("injected mid={stored_mid} dfid={stored_dfid}");
        }

        let songs = search("七里香", 1).expect("search failed");
        let vip_list: Vec<_> = songs.iter().filter(|s| s.vip).collect();
        println!("共 {} 首，VIP {} 首", songs.len(), vip_list.len());
        assert!(!vip_list.is_empty(), "搜索结果应含 VIP 曲目");

        let v = &vip_list[0];
        println!(
            "试播 VIP: {} - {} (album_audio_id={})",
            v.singer, v.name, v.album_audio_id
        );
        let (url, ext, label) = song_url(
            &v.id,
            v.album_audio_id,
            v.album_id,
            &v.hq_hash,
            &v.sq_hash,
            &v.super_hash,
            v.vip,
            &token,
            &userid,
            "lossless",
        )
        .expect("VIP 曲目取链接失败");
        println!("✓ {label} .{ext} {}...", &url[..url.len().min(70)]);
    }

    #[test]
    #[ignore] // 需要网络
    fn test_lyric_debug() {
        // 七里香（VIP 正版）的歌词：应返回 KRC 转换的增强 LRC
        let r = lyric("9218d81686c6ac681bc7ca621958ad6d");
        match r {
            Ok(Some(text)) => println!(
                "lyric ok, head: {}",
                text.chars().take(200).collect::<String>()
            ),
            Ok(None) => println!("lyric: None（无歌词）"),
            Err(e) => println!("lyric error: {e}"),
        }
    }

    #[test]
    #[ignore] // 需要网络
    fn test_playlist_vip_badges() {
        // 用户 Jay Chou 歌单：周杰伦正版歌应标 VIP（pay_type==3 回填）
        let pl = playlist_tracks("collection_3_560871299_3_0").expect("playlist failed");
        let vip_count = pl.songs.iter().filter(|s| s.vip).count();
        println!(
            "歌单 {} 首，VIP {} 首，第一首 vip={} album_audio_id={}",
            pl.songs.len(),
            vip_count,
            pl.songs.first().map(|s| s.vip).unwrap_or(false),
            pl.songs.first().map(|s| s.album_audio_id).unwrap_or(0)
        );
        assert!(vip_count > 0, "歌单应包含 VIP 曲目");
    }

    #[test]
    #[ignore] // 需要网络
    fn test_toplist_vip_badges() {
        // 榜单曲目也要有 VIP 标签（回归：toplist_tracks 曾漏掉权益回填）
        let songs = toplist_tracks(8888, 1).expect("toplist failed");
        let vip = songs.iter().filter(|s| s.vip).count();
        println!("TOP500 前 30 首：VIP {} 首", vip);
        assert!(vip > 0, "榜单应包含 VIP 曲目");
    }

    #[test]
    #[ignore] // 需要网络
    fn test_register_dev() {
        let dfid = register_dev().expect("register_dev failed");
        assert_eq!(dfid.len(), 24, "dfid 应为 24 位");
        // 二次注册：服务端对同一 mid 返回空 data，应复用缓存 dfid
        let dfid2 = register_dev().expect("re-register failed");
        assert_eq!(dfid2, dfid);
    }

    #[test]
    #[ignore] // 需要网络
    fn test_search_shows_vip() {
        // 匿名搜索也必须能看到 VIP 曲目（回归：songsearch_v2 主通道会
        // 把 pay_type==3 的歌整体过滤掉）
        let songs = search("七里香", 1).unwrap();
        assert!(songs.iter().any(|s| s.vip), "搜索结果应包含 VIP 曲目");
    }

    #[test]
    #[ignore] // 需要网络，依赖网关可用性
    fn test_toplists_and_random() {
        let tops = toplists().expect("toplists failed");
        assert!(!tops.is_empty());
        let songs = toplist_tracks(tops[0].id, 1).expect("toplist tracks failed");
        assert!(!songs.is_empty());
        let pl = random_playlist().expect("random playlist failed");
        assert!(!pl.songs.is_empty());
    }
}

/// 音质审计：按标题反查各档 hash（存档行只有 128 hash 时的补齐路径）
#[test]
#[ignore] // 需要网络
fn test_quality_hashes_by_search_audit() {
    // 验证 app 真实链路：搜索结果经 enrich_from_songsearch 补齐各档 hash
    let songs = search("突然好想你", 1).expect("search failed");
    let s = songs.first().expect("no songs");
    println!(
        "✓ 搜索链路 hash 补齐: hq={} sq={} super={}",
        s.hq_hash.len(),
        s.sq_hash.len(),
        s.super_hash.len()
    );
    // 反查（存档行补齐用）：可能因同名多版本匹配失败，仅观察
    match crate::kugou::search::quality_hashes_by_search(&s.id, &s.name) {
        Some((h, sq, sup)) => {
            println!(
                "✓ 反查成功: hq={} sq={} super={}",
                h.len(),
                sq.len(),
                sup.len()
            )
        }
        None => println!("反查未命中（同名多版本，属已知局限）"),
    }
}

/// 下载质量审计：完整复现 download_online 对酷狗的逻辑（搜索 → hash 补齐 →
/// v5 按无损取链 → 探测实际文件大小），需要网络 + 本机酷狗登录
#[test]
#[ignore]
fn test_kugou_download_quality_audit() {
    let db_path = std::env::var("APPDATA").unwrap() + r"\com.rustmusic.app\library.db";
    let conn =
        rusqlite::Connection::open_with_flags(&db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("打开应用数据库失败");
    let get = |k: &str| -> String {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [k], |r| {
            r.get::<_, String>(0)
        })
        .unwrap_or_default()
    };
    let token = get("kg_token");
    let userid = get("kg_userid");
    println!("token_len={} userid={}", token.len(), userid);

    let songs = search("晴天 周杰伦", 1).expect("search failed");
    let s = songs
        .iter()
        .find(|s| s.name.contains("晴天") && s.singer.contains("周杰伦"))
        .expect("未找到目标歌曲");
    println!(
        "搜索到: {} - {} | 128hash={} hq={} sq={} super={}",
        s.singer,
        s.name,
        &s.id[..8],
        s.hq_hash.len(),
        s.sq_hash.len(),
        s.super_hash.len()
    );
    // 下载路径：存档行无各档 hash，按标题反查补齐
    let (hq, sq, sup) = enrich_hashes(
        &s.id,
        &s.name,
        &s.singer,
        &s.hq_hash,
        &s.sq_hash,
        &s.super_hash,
    );
    println!(
        "补齐后: hq={} sq={} super={}",
        hq.len(),
        sq.len(),
        sup.len()
    );
    // 带真实 album_audio_id（修复后的下载路径）
    let (url, ext, label) = song_url(
        &s.id,
        s.album_audio_id,
        s.album_id,
        &hq,
        &sq,
        &sup,
        s.vip,
        &token,
        &userid,
        "lossless",
    )
    .expect("song_url failed");
    let agent = ureq::AgentBuilder::new().build();
    let size = agent
        .head(&url)
        .call()
        .ok()
        .and_then(|r| {
            r.header("content-length")
                .and_then(|v| v.parse::<u64>().ok())
        })
        .unwrap_or(0);
    println!(
        "✓ 带 album_audio_id={}: {label} .{ext} 大小={}",
        s.album_audio_id,
        bytes_mb(size)
    );
    // 存档行场景（最近播放/收藏下载）：只有 128 hash，按"标题+歌手"反查补齐
    let (hq2, sq2, sup2) = enrich_hashes(&s.id, &s.name, &s.singer, "", "", "");
    println!(
        "存档行反查（{} {}）: hq={} sq={} super={}",
        s.name,
        s.singer,
        hq2.len(),
        sq2.len(),
        sup2.len()
    );
    match song_url(
        &s.id, 0, 0, &hq2, &sq2, &sup2, s.vip, &token, &userid, "lossless",
    ) {
        Ok((url0, ext0, label0)) => {
            let size0 = agent
                .head(&url0)
                .call()
                .ok()
                .and_then(|r| {
                    r.header("content-length")
                        .and_then(|v| v.parse::<u64>().ok())
                })
                .unwrap_or(0);
            println!(
                "✓ 存档行反查后取链: {label0} .{ext0} 大小={}",
                bytes_mb(size0)
            );
        }
        Err(e) => println!("✗ 存档行反查后取链仍失败（{e}）"),
    }
}

#[cfg(test)] // 仅测试用（生产构建否则报 never used）
fn bytes_mb(n: u64) -> String {
    format!("{:.2} MB", n as f64 / 1048576.0)
}

/// 榜单审计：翻页拉全后与单页 30 对比（需要网络）
#[test]
#[ignore]
fn test_toplist_tracks_count_audit() {
    let tops = toplists().expect("榜单列表失败");
    println!("榜单数: {}", tops.len());
    for t in tops.iter().take(4) {
        match toplist_tracks(t.id, 1) {
            Ok(songs) => println!("✓ 榜单「{}」→ {} 首", t.name, songs.len()),
            Err(e) => println!("✗ 榜单「{}」失败: {e}", t.name),
        }
    }
}

/// 歌单详情审计：拉一个真实推荐歌单，检查实际返回曲目数（需要网络）
#[test]
#[ignore]
fn test_playlist_tracks_count_audit() {
    let specials = crate::kugou::browse::special_recommend(0, 1).expect("推荐页失败");
    println!("推荐歌单数: {}", specials.len());
    // 逐页打印：每页 got 与累计去重数（服务端若忽略 begin_idx 会重复返回首页）
    let pagesize = 90i64;
    for pick in specials.iter().take(4) {
        let mut seen = std::collections::HashSet::new();
        let mut total = 0usize;
        let mut pages = String::new();
        for page in 1..=20i64 {
            let extra = vec![
                ("area_code".into(), "1".into()),
                ("begin_idx".into(), ((page - 1) * pagesize).to_string()),
                ("plat".into(), "1".into()),
                ("type".into(), "1".into()),
                ("mode".into(), "1".into()),
                ("personal_switch".into(), "1".into()),
                (
                    "extend_fields".into(),
                    "abtags,hot_cmt,popularization".into(),
                ),
                ("pagesize".into(), pagesize.to_string()),
                ("global_collection_id".into(), pick.id.clone()),
            ];
            let v = gateway("/pubsongs/v2/get_other_list_file_nofilt", "", &extra, None)
                .expect("拉取失败");
            let list = v
                .pointer("/data/songs")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default();
            let got = list.len();
            for t in &list {
                if let Some(h) = t.get("hash").and_then(|x| x.as_str()) {
                    if seen.insert(h.to_string()) {
                        total += 1;
                    }
                }
            }
            pages.push_str(&format!("p{page}={got}/{} ", total));
            if got < pagesize as usize {
                break;
            }
        }
        println!("「{}」: {}", pick.name, pages);
    }
}
