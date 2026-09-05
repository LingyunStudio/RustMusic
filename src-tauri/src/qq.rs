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

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(12);
const GUID: &str = "2844095639";

/// 读取环境变量中的代理配置（与 ureq 内建逻辑一致）
fn system_proxy() -> Option<ureq::Proxy> {
    for k in [
        "ALL_PROXY",
        "all_proxy",
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
    ] {
        if let Ok(v) = std::env::var(k) {
            if let Ok(p) = ureq::Proxy::new(v) {
                return Some(p);
            }
        }
    }
    None
}

/// QQ 系接口统一走系统代理（ptlogin2 登录网关直连会被拒绝）
fn http_agent() -> &'static ureq::Agent {
    use std::sync::OnceLock;
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| match system_proxy() {
        Some(p) => ureq::AgentBuilder::new().proxy(p).build(),
        None => ureq::AgentBuilder::new().build(),
    })
}

/// 禁止重定向的 agent：check_sig / OAuth 的 p_skey、code 都在 302 响应上
fn no_redirect_agent() -> &'static ureq::Agent {
    use std::sync::OnceLock;
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let mut b = ureq::AgentBuilder::new().redirects(0);
        if let Some(p) = system_proxy() {
            b = b.proxy(p);
        }
        b.build()
    })
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
        let b = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16)
            .map_err(|e| e.to_string())?;
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
fn musicu_signed(
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
    let mut req = http_agent().post(&url)
        .set("Content-Type", "application/json")
        .set("Referer", "https://y.qq.com/")
        .set("User-Agent", UA)
        .timeout(TIMEOUT);
    if let Some(c) = cookie {
        req = req.set("Cookie", c);
    }
    let resp = req
        .send_string(&body)
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
    http_agent().get(url)
        .set("Referer", referer)
        .set("User-Agent", UA)
        .timeout(TIMEOUT)
        .call()
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
    pub name: String,
    pub singer: String,
    pub album: String,
    pub album_mid: String,
    pub duration_ms: u64,
    pub vip: bool,
}

// ---------- 搜索（匿名可用） ----------

pub fn search(keyword: &str, limit: i64) -> Result<Vec<QqSong>, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(vec![]);
    }
    let url = format!(
        "https://c.y.qq.com/soso/fcgi-bin/client_search_cp?ct=24&qqmusic_ver=1298&remoteplace=txt.yqq.top&t=0&aggr=1&cr=1&w={}&format=json&n={}",
        form_encode(keyword),
        limit
    );
    let text = plain_get(&url, "https://y.qq.com/")?;
    let resp: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("QQ 音乐搜索解析失败: {e}"))?;
    if resp.get("code").and_then(|c| c.as_i64()) != Some(0) {
        return Err("QQ 音乐搜索失败".into());
    }
    let mut out = Vec::new();
    if let Some(list) = resp.pointer("/data/song/list").and_then(|v| v.as_array()) {
        for s in list {
            let id = s.get("songmid").and_then(|v| v.as_str()).unwrap_or("");
            if id.is_empty() {
                continue;
            }
            let singer = s
                .get("singer")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                        .collect::<Vec<_>>()
                        .join(" / ")
                })
                .unwrap_or_default();
            out.push(QqSong {
                id: id.to_string(),
                name: s.get("songname").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                singer,
                album: s.get("albumname").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                album_mid: s.get("albummid").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                duration_ms: s.get("interval").and_then(|v| v.as_i64()).unwrap_or(0) as u64 * 1000,
                vip: s
                    .pointer("/pay/payplay")
                    .and_then(|v| v.as_i64())
                    .map(|v| v != 0)
                    .unwrap_or(false),
            });
        }
    }
    out.truncate(limit as usize);
    Ok(out)
}

// ---------- 播放链接（需登录 cookie） ----------

fn credential_cookie(musicid: &str, musickey: &str) -> String {
    format!(
        "uin={id}; qqmusic_uin={id}; qm_keyst={key}; qqmusic_key={key}",
        id = musicid,
        key = musickey
    )
}

/// 获取播放直链；失败（VIP/版权）返回 Err 带用户可读原因
pub fn song_url(songmid: &str, musicid: &str, musickey: &str) -> Result<String, String> {
    let payload = serde_json::json!({
        "comm": {"ct": 19, "cv": 1859},
        "req_1": {
            "module": "music.vkey.GetVkeyServerBase",
            "method": "CgiGetVkey",
            "param": {
                "guid": GUID,
                "songmid": [songmid],
                "songtype": [0],
                "uin": musicid,
                "loginflag": 1,
                "platform": "20",
            }
        }
    });
    let resp = musicu_signed(
        &payload,
        Some(&credential_cookie(musicid, musickey)),
    )?;
    let code = resp
        .pointer("/req_1/code")
        .and_then(|c| c.as_i64())
        .unwrap_or(0);
    if code != 0 {
        return Err(format!("获取播放链接失败（code {code}）"));
    }
    let purl = resp
        .pointer("/req_1/data/midurlinfo/0/purl")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if purl.is_empty() {
        return Err("该歌曲暂无可播放链接（可能需要 QQ 音乐 VIP 或版权受限）".into());
    }
    if purl.starts_with("http") {
        return Ok(purl.to_string());
    }
    let sip = resp
        .pointer("/req_1/data/sip")
        .and_then(|v| v.as_array())
        .and_then(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .find(|s| s.starts_with("http"))
        })
        .unwrap_or("http://ws.stream.qqmusic.qq.com/")
        .to_string();
    Ok(format!("{}{}", sip, purl))
}

// ---------- 歌词（匿名可用，返回 base64 编码的 LRC） ----------

pub fn lyric(songmid: &str) -> Result<Option<String>, String> {
    let url = format!(
        "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={}&g_tk=5381&format=json",
        form_encode(songmid)
    );
    let text = plain_get(&url, "https://y.qq.com/")?;
    let resp: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("歌词解析失败: {e}"))?;
    let b64 = resp
        .get("lyric")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    match b64 {
        Some(b) => {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(b)
                .map_err(|e| format!("歌词解码失败: {e}"))?;
            Ok(Some(String::from_utf8_lossy(&decoded).into_owned()))
        }
        None => Ok(None),
    }
}

// ---------- 扫码登录 ----------

/// 生成 QQ 登录二维码，返回 (qrsig, qr_png_base64)
pub fn qr_create() -> Result<(String, String), String> {
    let url = format!(
        "https://ssl.ptlogin2.qq.com/ptqrshow?appid=716027609&e=2&l=M&s=3&d=72&v=4&t={}&daid=383&pt_3rd_aid=100497308",
        chrono_now_ms()
    );
    let resp = http_agent().get(&url)
        .set("Referer", "https://xui.ptlogin2.qq.com/")
        .set("User-Agent", UA)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("获取登录二维码失败: {e}"))?;
    let qrsig = resp
        .all("Set-Cookie")
        .into_iter()
        .find_map(|c| {
            c.split(';')
                .next()
                .and_then(|seg| seg.trim().strip_prefix("qrsig="))
                .map(|s| s.to_string())
        })
        .ok_or_else(|| "未获取到登录标识（qrsig）".to_string())?;
    let mut png = Vec::new();
    resp.into_reader()
        .read_to_end(&mut png)
        .map_err(|e| format!("二维码读取失败: {e}"))?;
    if png.is_empty() {
        return Err("二维码内容为空".into());
    }
    let b64 = b64_encode(&png);
    Ok((qrsig, format!("data:image/png;base64,{b64}")))
}

/// 从 ptuiCB(...) 响应中提取单引号参数列表
fn extract_ptui_args(text: &str) -> Vec<String> {
    let start = match text.find("ptuiCB(") {
        Some(i) => i + 7,
        None => return vec![],
    };
    let end = text[start..].find(')').map(|i| start + i).unwrap_or(text.len());
    let inner = &text[start..end];
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut escape = false;
    for c in inner.chars() {
        if in_quote {
            if escape {
                cur.push(c);
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '\'' {
                args.push(std::mem::take(&mut cur));
                in_quote = false;
            } else {
                cur.push(c);
            }
        } else if c == '\'' {
            in_quote = true;
        }
    }
    args
}

fn parse_query_param(url: &str, key: &str) -> Option<String> {
    for seg in url.split(['?', '&']) {
        if let Some(v) = seg.strip_prefix(&format!("{key}=")) {
            return Some(v.to_string());
        }
    }
    None
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QqQrCheck {
    /// waiting | scanned | success | expired
    pub status: String,
    pub nickname: Option<String>,
    pub musicid: Option<String>,
    pub musickey: Option<String>,
}

/// 轮询扫码状态；成功时完成 check_sig → OAuth → QQLogin 换取播放凭证
pub fn qr_check(qrsig: &str) -> Result<QqQrCheck, String> {
    let token = hash33(qrsig, 0);
    let url = format!(
        "https://ssl.ptlogin2.qq.com/ptqrlogin?u1=https%3A%2F%2Fgraph.qq.com%2Foauth2.0%2Flogin_jump&ptqrtoken={token}&ptredirect=0&h=1&t=1&g=1&from_ui=1&ptlang=2052&action=0-0-{}&js_ver=20102616&js_type=1&pt_uistyle=40&aid=716027609&daid=383&pt_3rd_aid=100497308&has_onekey=1",
        chrono_now_ms()
    );
    let text = plain_get_with_cookie(&url, "https://xui.ptlogin2.qq.com/", &format!("qrsig={qrsig}"))?;
    let args = extract_ptui_args(&text);
    let code_str = args.first().cloned().unwrap_or_default();
    let code: i64 = code_str.parse().unwrap_or(-1);
    if code != 0 {
        let status = match code {
            65 => "expired",
            67 => "scanned",
            _ => "waiting",
        };
        return Ok(QqQrCheck { status: status.into(), nickname: None, musicid: None, musickey: None });
    }

    {
        if args.len() < 3 {
            return Err("登录响应缺少必要参数".into());
        }
        let check_url = &args[2];
        let uin = parse_query_param(check_url, "uin").ok_or("登录响应缺少 uin")?;
        let sigx = parse_query_param(check_url, "ptsigx").ok_or("登录响应缺少 ptsigx")?;

        // Step 1: check_sig —— 换取 qq.com 域的登录 cookie（p_skey 等）
        let check_sig_url = format!(
            "https://ssl.ptlogin2.graph.qq.com/check_sig?uin={uin}&pttype=1&service=ptqrlogin&nodirect=0&ptsigx={sigx}&s_url=https%3A%2F%2Fgraph.qq.com%2Foauth2.0%2Flogin_jump&ptlang=2052&ptredirect=100&aid=716027609&daid=383&j_later=0&low_login_hour=0&regmaster=0&pt_login_type=3&pt_aid=0&pt_aaid=16&pt_light=0&pt_3rd_aid=100497308"
        );
        let resp = no_redirect_agent().get(&check_sig_url)
            .set("Referer", "https://xui.ptlogin2.qq.com/")
            .set("User-Agent", UA)
            .timeout(TIMEOUT)
            .call()
            .map_err(|e| format!("check_sig 请求失败: {e}"))?;
        let check_cookies: Vec<String> = resp
            .all("Set-Cookie")
            .into_iter()
            .map(|c| c.split(';').next().unwrap_or("").trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
        let p_skey = check_cookies
            .iter()
            .find_map(|c| c.strip_prefix("p_skey="))
            .ok_or("未获取到 p_skey")?
            .to_string();
        let cookie_header = check_cookies.join("; ");

        // Step 2: OAuth 授权 —— 换取 code
        let g_tk = hash33(&p_skey, 5381);
        let form = format!(
            "response_type=code&client_id=100497308&redirect_uri={}&scope=get_user_info%2Cget_app_friends&state=state&switch=&from_ptlogin=1&src=1&update_auth=1&openapi=1010_1030&g_tk={g_tk}&auth_time={}&ui={}",
            form_encode("https://y.qq.com/portal/wx_redirect.html?login_type=1&surl=https://y.qq.com/"),
            chrono_now_ms(),
            rand_hex(16),
        );
        let resp = no_redirect_agent().post("https://graph.qq.com/oauth2.0/authorize")
            .set("Content-Type", "application/x-www-form-urlencoded")
            .set("Referer", "https://graph.qq.com/")
            .set("User-Agent", UA)
            .set("Cookie", &cookie_header)
            .timeout(TIMEOUT)
            .send_string(&form)
            .map_err(|e| format!("OAuth 授权失败: {e}"))?;
        let location = resp.header("Location").unwrap_or("").to_string();
        let oauth_code = parse_query_param(&location, "code").ok_or_else(|| {
            format!("OAuth 授权未返回 code（HTTP {}）", resp.status())
        })?;

        // Step 3: QQLogin —— 用 code 换取播放凭证（musicid / musickey）
        let payload = serde_json::json!({
            "comm": {"tmeLoginType": 2},
            "req_1": {
                "module": "QQConnectLogin.LoginServer",
                "method": "QQLogin",
                "param": {"code": oauth_code}
            }
        });
        let resp = musicu_signed(&payload, None)?;
        let top = &resp;
        let data = resp.get("data").unwrap_or(&serde_json::Value::Null);
        let pick_str = |base: &serde_json::Value, key: &str| -> Option<String> {
            base.get(key)
                .and_then(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| v.as_i64().map(|n| n.to_string()))
                })
                .filter(|s| !s.is_empty())
        };
        let musicid = pick_str(top, "musicid")
            .or_else(|| pick_str(data, "musicid"))
            .or_else(|| pick_str(top, "str_musicid"))
            .or_else(|| pick_str(data, "str_musicid"))
            .ok_or("登录响应缺少 musicid")?;
        let musickey = pick_str(top, "musickey")
            .or_else(|| pick_str(data, "musickey"))
            .ok_or("登录响应缺少 musickey")?;
        let nickname = pick_str(top, "nickname")
            .or_else(|| pick_str(data, "nickname"))
            .or_else(|| args.get(5).cloned());

        Ok(QqQrCheck {
            status: "success".into(),
            nickname,
            musicid: Some(musicid),
            musickey: Some(musickey),
        })
    }
}

fn rand_hex(len: usize) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..len).map(|_| format!("{:x}", rng.gen_range(0..16))).collect()
}

fn plain_get_with_cookie(url: &str, referer: &str, cookie: &str) -> Result<String, String> {
    http_agent().get(url)
        .set("Referer", referer)
        .set("User-Agent", UA)
        .set("Cookie", cookie)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("QQ 音乐接口请求失败: {e}"))?
        .into_string()
        .map_err(|e| format!("QQ 音乐响应读取失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let r = search("小情歌 苏打绿", 5).expect("search failed");
        for s in &r {
            println!("  [{}] {} - {} vip={} {}s", s.id, s.name, s.singer, s.vip, s.duration_ms / 1000);
        }
        assert!(!r.is_empty(), "search returned no songs");
        let any = &r[0];
        let lrc = lyric(&any.id).expect("lyric failed");
        println!(
            "lyric[{}] {:?}",
            any.id,
            lrc.as_ref().map(|t| t.chars().take(50).collect::<String>())
        );
    }
}
