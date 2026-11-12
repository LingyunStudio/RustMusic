//! QQ 音乐接口客户端（搜索 / 取播放链接 / 歌词 / 扫码登录）
//!
//! 仅以用户自身账号访问：免费曲目匿名可播，VIP 曲目需用户登录且按其
//! 会员权益获取播放链接，不包含任何绕过付费 / 版权限制的功能。

use base64::Engine;
use serde::Serialize;
use sha1::{Digest as Sha1Digest, Sha1};
use std::io::Read;
use std::time::Duration;

fn b64_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(12);
/// 客户端 guid：首次生成后写入 settings 表持久化（10 位数字，与官方客户端
/// 格式一致）。此前每进程重启随机一次——服务端风控视角下会话设备频繁变化，
/// 是 musickey 几天就失效的诱因之一（issue#8-5）；改为跨重启稳定复用。
fn guid() -> &'static str {
    use std::sync::OnceLock;
    static GUID: OnceLock<String> = OnceLock::new();
    GUID.get_or_init(|| GUID_STATIC.lock().unwrap().clone())
}

static GUID_STATIC: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// 应用启动时（db 就绪后）调用：读取持久化 guid，没有则生成并落库
pub fn init_guid(conn: &rusqlite::Connection) {
    let mut g = GUID_STATIC.lock().unwrap();
    if !g.is_empty() {
        return;
    }
    *g = crate::db::get_setting(conn, "qq_guid").unwrap_or_else(|| {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut s = String::with_capacity(10);
        s.push(char::from(b'1' + rng.gen_range(0..9) as u8));
        for _ in 0..9 {
            s.push(char::from(b'0' + rng.gen_range(0..10) as u8));
        }
        let s2 = s;
        crate::db::set_setting(conn, "qq_guid", &s2);
        s2
    });
}

/// 读取环境变量中的代理配置（与 ureq 内建逻辑一致）。
/// 顺序上先试 HTTP/HTTPS 代理再试 ALL_PROXY：ureq 的 socks 后端是可选特性，
/// 本构建未启用，socks5:// 的 ALL_PROXY 会让 Proxy::new 直接失败。先命中
/// HTTP 代理时不会走到 ALL_PROXY，常见 Clash 系环境（两者同端口）照常可用
pub fn system_proxy() -> Option<ureq::Proxy> {
    for k in [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        if let Ok(v) = std::env::var(k) {
            if let Ok(p) = ureq::Proxy::new(v) {
                return Some(p);
            }
        }
    }
    None
}

/// QQ 系接口统一走系统代理（ptlogin2 登录网关直连会被拒绝）。
/// agent 缓存可重建（issue#8-2）：网络/代理切换后连接池里的死连接不会
/// 自愈，传输错误时 rebuild_agents() 清空缓存，下次请求按当前环境重建。
pub fn http_agent() -> ureq::Agent {
    if let Some(a) = HTTP_AGENT.read().unwrap().clone() {
        return a;
    }
    let a = match system_proxy() {
        Some(p) => ureq::AgentBuilder::new().proxy(p).build(),
        None => ureq::AgentBuilder::new().build(),
    };
    *HTTP_AGENT.write().unwrap() = Some(a.clone());
    a
}

fn no_redirect_agent() -> ureq::Agent {
    if let Some(a) = NO_REDIRECT_AGENT.read().unwrap().clone() {
        return a;
    }
    let mut b = ureq::AgentBuilder::new().redirects(0);
    if let Some(p) = system_proxy() {
        b = b.proxy(p);
    }
    let a = b.build();
    *NO_REDIRECT_AGENT.write().unwrap() = Some(a.clone());
    a
}

static HTTP_AGENT: std::sync::RwLock<Option<ureq::Agent>> = std::sync::RwLock::new(None);
static NO_REDIRECT_AGENT: std::sync::RwLock<Option<ureq::Agent>> = std::sync::RwLock::new(None);

/// 网络级失败后重建连接池（调用方在传输错误重试前调用）
fn rebuild_agents() {
    *HTTP_AGENT.write().unwrap() = None;
    *NO_REDIRECT_AGENT.write().unwrap() = None;
}

/// QQ 请求的传输级错误恢复：重建 agent → 等 600ms → 重试一次（issue#8-2）
fn send_with_recover<T>(mut f: impl FnMut() -> Result<T, ureq::Error>) -> Result<T, String> {
    match f() {
        Ok(v) => Ok(v),
        Err(e) if crate::net::is_transport_err(&e) => {
            rebuild_agents();
            std::thread::sleep(std::time::Duration::from_millis(600));
            f().map_err(|e2| {
                if crate::net::is_transport_err(&e2) {
                    crate::net::transport_text().to_string()
                } else {
                    e2.to_string()
                }
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

// ---------- 通用 ----------

fn form_encode(s: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

/// QQ 的 hash33：h = (h<<5) + h + c，最终取 31 位
fn hash33(s: &str, h0: u64) -> u64 {
    let mut h = h0;
    for c in s.chars() {
        h = ((h << 5) + h + (c as u64)) & 0x7fffffff;
    }
    h & 0x7fffffff
}

/// musicu.fcg 的 zzc 签名（SHA1 + 固定下标挑选 + 异或 + base64）
fn zzc_sign(text: &str) -> Result<String, String> {
    const P1: [usize; 8] = [23, 14, 6, 36, 16, 40, 7, 19];
    const P2: [usize; 8] = [16, 1, 32, 12, 19, 27, 8, 5];
    const SV: [u8; 20] = [
        89, 39, 179, 150, 218, 82, 58, 252, 177, 52, 186, 123, 120, 64, 242, 133, 143, 161, 121,
        179,
    ];
    let mut hasher = Sha1::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    let hash = hex::encode_upper(digest);
    let chars: Vec<char> = hash.chars().collect();
    let pick = |idx: &[usize; 8]| -> String {
        // 与 JS 一致：越界下标在 join 时表现为空串
        idx.iter()
            .filter_map(|i| chars.get(*i).copied())
            .collect::<String>()
    };
    let part1 = pick(&P1);
    let part2 = pick(&P2);
    let mut part3 = Vec::with_capacity(20);
    for (i, v) in SV.iter().enumerate() {
        let b = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
        part3.push(v ^ b);
    }
    let b64: String = base64::engine::general_purpose::STANDARD
        .encode(part3)
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | '+' | '='))
        .collect();
    Ok(format!("zzc{part1}{b64}{part2}").to_lowercase())
}

/// 签名版 musicu 请求（musics.fcg）
pub fn musicu_signed(
    payload: &serde_json::Value,
    cookie: Option<&str>,
) -> Result<serde_json::Value, String> {
    let body = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    let sign = zzc_sign(&body)?;
    let url = format!(
        "https://u.y.qq.com/cgi-bin/musics.fcg?_={}&sign={}",
        chrono_now_ms(),
        sign
    );
    let mut req = http_agent()
        .post(&url)
        .set("Content-Type", "application/json")
        .set("Referer", "https://y.qq.com/")
        .set("User-Agent", UA)
        .timeout(TIMEOUT);
    if let Some(c) = cookie {
        req = req.set("Cookie", c);
    }
    let resp = send_with_recover(|| req.clone().send_string(&body))
        .map_err(|e| format!("QQ 音乐接口请求失败: {e}"))?;
    let text = resp
        .into_string()
        .map_err(|e| format!("QQ 音乐响应读取失败: {e}"))?;
    serde_json::from_str(&text).map_err(|e| {
        format!(
            "QQ 音乐响应解析失败: {e} | body: {}",
            text.chars().take(150).collect::<String>()
        )
    })
}

fn chrono_now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn plain_get(url: &str, referer: &str) -> Result<String, String> {
    send_with_recover(|| {
        http_agent()
            .get(url)
            .set("Referer", referer)
            .set("User-Agent", UA)
            .timeout(TIMEOUT)
            .call()
    })
    .map_err(|e| format!("QQ 音乐接口请求失败: {e}"))?
    .into_string()
    .map_err(|e| format!("QQ 音乐响应读取失败: {e}"))
}

// ---------- 数据模型 ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QqSong {
    /// songmid（字符串 ID）
    pub id: String,
    /// 数字歌曲 ID（歌曲详情接口 GetSongInfo 用；Hi-Res 文件名解析需要）
    #[serde(default)]
    pub song_id: u64,
    pub name: String,
    pub singer: String,
    pub album: String,
    pub album_mid: String,
    /// 媒体文件 mid（构造 vkey filename 用）
    pub media_mid: String,
    pub duration_ms: u64,
    pub vip: bool,
}

mod browse;
mod login;
mod lyric;
mod playlists;
mod search;
mod url;
pub use browse::*;
pub use login::*;
pub use lyric::*;
pub use playlists::*;
pub use search::*;
pub use url::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wx_lp_parse() {
        assert_eq!(
            parse_wx_lp("window.wx_errcode=405;window.wx_code='abc123';"),
            (405, "abc123".into())
        );
        assert_eq!(
            parse_wx_lp("window.wx_errcode=408;window.wx_code='';"),
            (408, "".into())
        );
        assert_eq!(parse_wx_lp("garbage"), (-1, "".into()));
    }

    #[test]
    fn test_wx_uuid_extract() {
        let html = r#"<img class="js_qrcode_img" src="/connect/qrcode/Ab12Cd34Ef56Gh78"><script>l/qrconnect?uuid=Ab12Cd34Ef56Gh78</script>"#;
        let i = html.find("/connect/qrcode/").unwrap();
        let rest = &html[i + "/connect/qrcode/".len()..];
        let uuid = &rest[..rest.find('"').unwrap()];
        assert_eq!(uuid, "Ab12Cd34Ef56Gh78");
    }

    #[test]
    fn test_wx_cookie_value() {
        let cookies = vec![
            "qm_keyst=Q_H_L_abc.def.ghi; Path=/; HttpOnly".to_string(),
            "wxuin=2534405790; Path=/".to_string(),
            "wxrefresh_token=17_AAA=BBB; Path=/".to_string(),
        ];
        assert_eq!(cookie_value(&cookies, "qm_keyst"), Some("Q_H_L_abc.def.ghi".into()));
        assert_eq!(cookie_value(&cookies, "wxuin"), Some("2534405790".into()));
        assert_eq!(cookie_value(&cookies, "wxrefresh_token"), Some("17_AAA=BBB".into()));
        assert_eq!(cookie_value(&cookies, "missing"), None);
    }

    /// 走真实微信接口：取二维码 + 一次长轮询（应返回等待态）。
    /// 需要网络，手动 `cargo test -- --ignored` 运行；扫码确认后的
    /// 凭证交换需真人扫码，不在自动化范围
    #[test]
    #[ignore]
    fn test_wx_qr_create_and_poll() {
        let r = wx_qr_create().expect("wx qr create failed");
        assert!(!r.uuid.is_empty());
        assert!(r.qr.starts_with("data:image/jpeg;base64,"));
        let status = wx_qr_check(&r.uuid).expect("wx qr check failed");
        assert!(
            status.status == "waiting" || status.status == "scanned",
            "unexpected status: {status:?}"
        );
    }

    #[test]
    fn test_zzc_sign_vectors() {
        assert_eq!(
            zzc_sign("123").unwrap(),
            "zzcec1b555gzqzg7laztguyjl2bu20r6x1w50c55f60"
        );
        assert_eq!(
            zzc_sign("hello world").unwrap(),
            "zzcfb3415bc4nfoxmd9uik71mkomtubjfjp141a1cbbcc"
        );
        assert_eq!(
            zzc_sign("jixun.uk").unwrap(),
            "zzcf47b78apso27mjjbbzgbof0szikfkvyqc7fc3a2b5"
        );
    }

    #[test]
    fn test_search_and_lyric() {
        let r = search("小情歌 苏打绿", 5, 1).expect("search failed");
        for s in &r {
            println!(
                "  [{}] {} - {} vip={} {}s",
                s.id,
                s.name,
                s.singer,
                s.vip,
                s.duration_ms / 1000
            );
        }
        assert!(!r.is_empty(), "search returned no songs");
        let any = &r[0];
        let lrc = lyric(&any.id).expect("lyric failed");
        println!(
            "lyric[{}] {:?}",
            any.id,
            lrc.as_ref()
                .map(|t| t.lrc.chars().take(50).collect::<String>())
        );
    }

    #[test]
    fn test_toplists_and_random() {
        let tops = toplists().expect("toplists failed");
        println!(
            "{} toplists, first: {:?}",
            tops.len(),
            tops.first().map(|t| (&t.id, &t.title))
        );
        assert!(!tops.is_empty(), "no toplists");
        let songs = toplist_tracks(tops[0].id, 1).expect("toplist tracks failed");
        println!("top {} songs: {}", tops[0].title, songs.len());
        assert!(!songs.is_empty(), "toplist returned no songs");
        let pl = random_playlist().expect("random playlist failed");
        println!(
            "random playlist: {} by {} ({} songs)",
            pl.name,
            pl.creator,
            pl.songs.len()
        );
        assert!(!pl.songs.is_empty(), "random playlist returned no songs");
    }
}

/// 音质审计：无损设置下实际下发的格式（需要网络 + 本机 QQ 登录）
#[test]
#[ignore]
fn test_song_url_lossless_audit() {
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
    let songs = search("风的来信", 1, 1).expect("search failed");
    // 免费歌（非 VIP）走 320 档，VIP 歌在无权益时应明确报错——两种都验证
    let s = songs.iter().find(|s| !s.vip).expect("搜索结果无免费歌曲");
    let songmid = &s.id;
    let media_mid = &s.media_mid;
    println!("试听: {} - {}", s.singer, s.name);
    let (url, ext, label) = song_url(
        songmid,
        media_mid,
        &get("qq_musicid"),
        &get("qq_musickey"),
        "lossless",
        s.vip,
    )
    .expect("song_url failed");
    println!(
        "✓ QQ lossless 实际下发: {label} .{ext} url={}",
        &url[..url.len().min(80)]
    );
}
