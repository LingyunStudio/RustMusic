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
/// 客户端 guid：每进程随机生成一次（10 位数字，与官方客户端格式一致），
/// 避免所有安装共用固定值触发风控
fn guid() -> &'static str {
    use std::sync::OnceLock;
    static GUID: OnceLock<String> = OnceLock::new();
    GUID.get_or_init(|| {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut s = String::with_capacity(10);
        s.push(char::from(b'1' + rng.gen_range(0..9) as u8));
        for _ in 0..9 {
            s.push(char::from(b'0' + rng.gen_range(0..10) as u8));
        }
        s
    })
}

/// 读取环境变量中的代理配置（与 ureq 内建逻辑一致）
pub fn system_proxy() -> Option<ureq::Proxy> {
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
pub fn http_agent() -> &'static ureq::Agent {
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
    /// 媒体文件 mid（构造 vkey filename 用）
    pub media_mid: String,
    pub duration_ms: u64,
    pub vip: bool,
}

// ---------- 搜索（匿名可用） ----------

/// 从搜索条目解析 QqSong（新旧通道字段兼容：
/// 新=mid/name/singer/album.mid、旧=songmid/songname/albummid）
fn song_from_search_json(s: &serde_json::Value) -> Option<QqSong> {
    let id = s
        .get("mid")
        .or_else(|| s.get("songmid"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if id.is_empty() {
        return None;
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
    Some(QqSong {
        id: id.to_string(),
        name: s
            .get("name")
            .or_else(|| s.get("songname"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        singer,
        album: s
            .pointer("/album/name")
            .or_else(|| s.get("albumname"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        album_mid: s
            .pointer("/album/mid")
            .or_else(|| s.get("albummid"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        media_mid: s
            .get("media_mid")
            .or_else(|| s.pointer("/file/media_mid"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        duration_ms: s.get("interval").and_then(|v| v.as_i64()).unwrap_or(0) as u64 * 1000,
        vip: s
            .pointer("/pay/pay_play")
            .or_else(|| s.pointer("/pay/payplay"))
            .and_then(|v| v.as_i64())
            .map(|v| v != 0)
            .unwrap_or(false),
    })
}

pub fn search(keyword: &str, limit: i64, page: i64) -> Result<Vec<QqSong>, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(vec![]);
    }
    // 主通道：soso 明文接口 search_for_qq_cp（匿名 GET 可用）。
    // 同族 client_search_cp 已 500，签名版 SearchCgiService 对部分网络
    // 返回 500003（拒绝匿名请求），故以明文 soso 为主。
    let url = format!(
        "https://c.y.qq.com/soso/fcgi-bin/search_for_qq_cp?w={}&format=json&n={}&p={}&g_tk=5381",
        form_encode(keyword),
        limit,
        page
    );
    let mut last_err: String = match plain_get(&url, "https://y.qq.com/") {
        Err(e) => e,
        Ok(text) => {
            let resp: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => return Err(format!("QQ 音乐搜索解析失败: {e}")),
            };
            if resp.get("code").and_then(|c| c.as_i64()) == Some(0) {
                if let Some(list) = resp.pointer("/data/song/list").and_then(|v| v.as_array()) {
                    let out: Vec<QqSong> =
                        list.iter().filter_map(song_from_search_json).collect();
                    if !out.is_empty() {
                        return Ok(out);
                    }
                }
            }
            format!(
                "soso 通道无数据（code {}）",
                resp.get("code").and_then(|c| c.as_i64()).unwrap_or(-1)
            )
        }
    };

    // 备用通道：musicu 的 SearchCgiService（网页版现行搜索，部分网络可用）
    let payload = serde_json::json!({
        "comm": {"ct": 19, "cv": 1859},
        "req_1": {
            "module": "music.adsense.SearchCgiService",
            "method": "DoSearchForQmicMusic",
            "param": {
                "search_type": 0,
                "query": keyword,
                "page_num": page,
                "num_per_page": limit
            }
        }
    });
    if let Ok(resp) = musicu_signed(&payload, None) {
        let code = resp.pointer("/req_1/code").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code == 0 {
            if let Some(list) = resp
                .pointer("/req_1/data/body/song/list")
                .and_then(|v| v.as_array())
            {
                let out: Vec<QqSong> =
                    list.iter().filter_map(song_from_search_json).collect();
                if !out.is_empty() {
                    return Ok(out);
                }
            }
            last_err = format!("{last_err}，签名通道无歌曲数据");
        } else {
            last_err = format!("{last_err}，签名通道 code {code}");
        }
    } else {
        last_err = format!("{last_err}，签名通道请求失败");
    }
    Err(format!("QQ 音乐搜索失败: {last_err}"))
}

// ---------- 播放链接（需登录 cookie） ----------

fn credential_cookie(musicid: &str, musickey: &str) -> String {
    format!(
        "uin={id}; qqmusic_uin={id}; qm_keyst={key}; qqmusic_key={key}",
        id = musicid,
        key = musickey
    )
}

/// 在 QQLogin 响应的任意层级查找同时含 musicid 与 musickey 的对象
fn find_str_anywhere(v: &serde_json::Value, key: &str) -> Option<String> {
    if let Some(obj) = v.as_object() {
        if let Some(x) = obj.get(key) {
            if let Some(s) = x.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        for (_, child) in obj {
            if let Some(found) = find_str_anywhere(child, key) {
                return Some(found);
            }
        }
    }
    if let Some(arr) = v.as_array() {
        for child in arr {
            if let Some(found) = find_str_anywhere(child, key) {
                return Some(found);
            }
        }
    }
    None
}

fn find_credential(v: &serde_json::Value) -> Option<(String, String)> {
    if let Some(obj) = v.as_object() {
        let get = |k: &str| -> Option<String> {
            obj.get(k).and_then(|x| {
                x.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| x.as_i64().map(|n| n.to_string()))
            })
        };
        if let (Some(id), Some(key)) = (get("musicid").or_else(|| get("str_musicid")), get("musickey")) {
            if !id.is_empty() && !key.is_empty() {
                return Some((id, key));
            }
        }
        for (_, child) in obj {
            if let Some(found) = find_credential(child) {
                return Some(found);
            }
        }
    }
    if let Some(arr) = v.as_array() {
        for child in arr {
            if let Some(found) = find_credential(child) {
                return Some(found);
            }
        }
    }
    None
}

/// 按音质请求播放直链，从所选音质逐级回退；返回 (url, ext)
///
/// `vip`：前端缓存里该曲目的 VIP 标志（来自搜索结果的 pay.pay_play）。
/// 用于失败分类——VIP 曲目拿不到链接一律按"权益不足"报错（登录/VIP 状态
/// 变化后可恢复），绝不误报无版权；非 VIP 曲目在会话有效仍拿不到链接时
/// 才判为无版权/下架。
pub fn song_url(
    songmid: &str,
    media_mid: &str,
    musicid: &str,
    musickey: &str,
    quality: &str,
    vip: bool,
) -> Result<(String, String), String> {
    // 前缀：M500=128k M800=320k F000=flac；无 media_mid 时按官方规则用 songmid 拼接
    let ladder: Vec<(&str, &str)> = match quality {
        "lossless" => vec![("F000", "flac"), ("M800", "mp3"), ("M500", "mp3")],
        "standard" => vec![("M500", "mp3")],
        _ => vec![("M800", "mp3"), ("M500", "mp3")],
    };
    let mut last_resp = String::new();
    // 所有尝试都返回 code==0（会话有效、接口无异常）但始终没有链接
    let mut all_ok_but_no_url = true;
    for (prefix, ext) in ladder {
        let file_base = if media_mid.is_empty() {
            format!("{songmid}{songmid}")
        } else {
            format!("{media_mid}{songmid}")
        };
        let payload = serde_json::json!({
            "comm": {"ct": 19, "cv": 1859},
            "req_1": {
                "module": "music.vkey.GetVkey",
                "method": "UrlGetVkey",
                "param": {
                    "uin": musicid,
                    "filename": [format!("{prefix}{file_base}.{ext}")],
                    "guid": guid(),
                    "songmid": [songmid],
                    "songtype": [0],
                    "ctx": 0,
                }
            }
        });
        let resp = musicu_signed(
            &payload,
            Some(&credential_cookie(musicid, musickey)),
        )?;
        // 104009 = 会话无效（未登录 / musickey 已过期）。
        // 换音质也无法挽回，立即中止并让前端引导重新登录。
        if resp.pointer("/req_1/code").and_then(|c| c.as_i64()) == Some(104009) {
            return Err("QQ 音乐登录已过期，请重新登录".into());
        }
        if resp.pointer("/req_1/code").and_then(|c| c.as_i64()) != Some(0) {
            all_ok_but_no_url = false;
        }
        let purl = resp
            .pointer("/req_1/data/midurlinfo/0/purl")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !purl.is_empty() {
            let url = if purl.starts_with("http") {
                purl.to_string()
            } else {
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
                format!("{}{}", sip, purl)
            };
            return Ok((url, ext.to_string()));
        }
        last_resp = serde_json::to_string(&resp).unwrap_or_default();
    }
    // 会话有效仍拿不到链接：按 VIP 标志分类——VIP 曲目是权益不足（可随
    // 登录/会员状态恢复），非 VIP 曲目才是真无版权/下架（永久）。
    Err(
        if all_ok_but_no_url {
            if vip {
                "该曲目需要 QQ 音乐 VIP 权益（请确认已登录且会员状态有效）".into()
            } else {
                "该歌曲在 QQ 音乐无版权或已下架".into()
            }
        } else {
            format!(
                "该歌曲暂无可播放链接（接口异常，可稍后重试）| {}",
                last_resp.chars().take(120).collect::<String>()
            )
        },
    )
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

/// 逐字歌词（QRC）：musicu 的 PlayLyricInfo 模块，需登录凭证。
/// lyric 字段为 hex 编码的 QQ 魔改 Triple-DES + zlib + XML 数据，
/// 解密提取 LyricContent 后转换为增强 LRC；无逐字数据返回 None 回落行级。
pub fn lyric_qrc(
    songmid: &str,
    musicid: &str,
    musickey: &str,
) -> Result<Option<String>, String> {
    let payload = serde_json::json!({
        "comm": {"uin": musicid, "format": "json", "ct": 19, "cv": 0},
        "req_1": {
            "module": "music.musichallSong.PlayLyricInfo",
            "method": "GetPlayLyricInfo",
            "param": {
                "songMid": songmid,
                "qrc": 1,            // 1 = 请求逐字（QRC），0 = 行级
                "qrc_tts": 0,
                "romalrc": 0,
                "trans": 0
            }
        }
    });
    let cookie = credential_cookie(musicid, musickey);
    let resp = musicu_signed(&payload, Some(&cookie))?;
    let code = resp
        .pointer("/req_1/code")
        .and_then(|c| c.as_i64())
        .unwrap_or(0);
    if code != 0 {
        return Ok(None);
    }
    // qrc=1 → lyric 为逐字（加密）；qrc=0 → 行级 LRC（无逐字可回落）
    let is_qrc = resp
        .pointer("/req_1/data/qrc")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        == 1;
    if !is_qrc {
        return Ok(None);
    }
    let encoded = resp
        .pointer("/req_1/data/lyric")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let Some(encoded) = encoded else { return Ok(None) };
    // 解密失败（格式变化等）时返回 None，调用方回落行级 LRC
    let xml = match crate::qrc::decrypt(encoded) {
        Ok(x) => x,
        Err(_) => return Ok(None),
    };
    let Some(content) = crate::qrc::extract_lyric_content(&xml) else {
        return Ok(None);
    };
    // XML 属性里的换行是转义文本：还原为真实行
    let content = content.replace("\\n", "\n");
    let enhanced = crate::qrc::to_enhanced_lrc(&content);
    if enhanced.is_empty() {
        return Ok(None);
    }
    Ok(Some(enhanced))
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
    pub encrypt_uin: Option<String>,
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
        return Ok(QqQrCheck { status: status.into(), nickname: None, musicid: None, musickey: None, encrypt_uin: None });
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
        // 先走签名通道（musics.fcg），失败再试明文通道（musicu.fcg）
        let make_payload = || {
            serde_json::json!({
                "comm": {"tmeLoginType": 2},
                "req_1": {
                    "module": "QQConnectLogin.LoginServer",
                    "method": "QQLogin",
                    "param": {"code": oauth_code}
                }
            })
        };
        let mut raw_log = String::new();
        let mut login_resp: Option<serde_json::Value> = None;
        let mut credential: Option<(String, String)> = None;

        if let Ok(v) = musicu_signed(&make_payload(), None) {
            login_resp = Some(v.clone());
            match find_credential(&v) {
                Some(c) => credential = Some(c),
                None => raw_log = serde_json::to_string(&v).unwrap_or_default(),
            }
        }
        if credential.is_none() {
            let url = "https://u.y.qq.com/cgi-bin/musicu.fcg";
            if let Ok(body) = serde_json::to_string(&make_payload()) {
                let sent = ureq::post(url)
                    .set("Content-Type", "application/json")
                    .set("Referer", "https://y.qq.com/")
                    .set("User-Agent", UA)
                    .timeout(TIMEOUT)
                    .send_string(&body);
                if let Ok(r) = sent {
                    if let Ok(text) = r.into_string() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                            login_resp = Some(v.clone());
                            match find_credential(&v) {
                                Some(c) => credential = Some(c),
                                None => raw_log = text,
                            }
                        }
                    }
                }
            }
        }

        let (musicid, musickey) = credential.ok_or_else(|| {
            format!(
                "登录响应缺少凭证 | {}",
                raw_log.chars().take(200).collect::<String>()
            )
        })?;
        let nickname = args.get(5).cloned();
        let encrypt_uin = login_resp
            .as_ref()
            .and_then(|v| find_str_anywhere(v, "encryptUin"));

        Ok(QqQrCheck {
            status: "success".into(),
            nickname,
            musicid: Some(musicid),
            musickey: Some(musickey),
            encrypt_uin,
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


/// 用户创建的歌单列表
pub fn user_playlists(musicid: &str, musickey: &str) -> Result<Vec<crate::models::UserPlaylistMeta>, String> {
    let payload = serde_json::json!({
        "comm": {"ct": 19, "cv": 1859},
        "req_1": {
            "module": "music.musicasset.PlaylistBaseRead",
            "method": "GetPlaylistByUin",
            "param": {"uin": musicid}
        }
    });
    let resp = musicu_signed(&payload, Some(&credential_cookie(musicid, musickey)))?;
    let code = resp.pointer("/req_1/code").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 0 {
        return Err(if code == 104009 {
            "QQ 音乐登录已过期，请重新登录".into()
        } else {
            format!("获取歌单列表失败（code {code}）")
        });
    }
    let mut out = Vec::new();
    if let Some(list) = resp.pointer("/req_1/data/v_playlist").and_then(|v| v.as_array()) {
        for p in list {
            // 响应为驼峰命名：tid / dirName / songNum（蛇形为兼容备选）
            let id = p
                .get("tid")
                .or_else(|| p.get("dirId"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let name = p
                .get("dirName")
                .or_else(|| p.get("diss_name"))
                .or_else(|| p.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let count = p
                .get("songNum")
                .or_else(|| p.get("song_num"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            // “我喜欢/收藏”(dirId=201) 是 QQ 的特殊收藏夹：没有可用的
            // 普通 dissid，统一用 201 作哨兵 id 导出，playlist_tracks
            // 按 201 走收藏夹专用参数拉取
            let dir_id = p
                .get("dirId")
                .or_else(|| p.get("dirid"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let id = if dir_id == 201 { 201 } else { id };
            if id != 0 && !name.is_empty() {
                out.push(crate::models::UserPlaylistMeta { id, name, track_count: count });
            }
        }
    }
    Ok(out)
}

/// 加密 uin（CgiGetDiss 拉收藏夹需要）：登录响应里带原值 encryptUin，
/// 老登录态没存时按官方规则推导（3 个 NUL 前缀 + uin 的 base64）
pub fn encrypt_uin_of(musicid: &str, stored: &str) -> String {
    if !stored.is_empty() {
        return stored.to_string();
    }
    let mut buf = vec![0u8, 0, 0];
    buf.extend_from_slice(musicid.as_bytes());
    b64_encode(&buf)
}

/// 歌单内曲目（CgiGetDiss，分页拉全）。
/// disstid=201 为哨兵：拉取“我喜欢/收藏”夹（dirId=201，disstid 需传 0，
/// 并附 enc_host_uin），普通歌单照旧传真实 dissid。
pub fn playlist_tracks(
    disstid: i64,
    musicid: &str,
    musickey: &str,
    encrypt_uin: &str,
) -> Result<Vec<QqSong>, String> {
    let fav_mode = disstid == 201;
    let mut all = Vec::new();
    let mut begin = 0i64;
    let mut last_first_mid = String::new();
    loop {
        let param = if fav_mode {
            serde_json::json!({
                "disstid": 0,
                "dirid": 201,
                "tag": true,
                "song_begin": begin,
                "song_num": 100,
                "userinfo": false,
                "orderlist": true,
                "onlysonglist": 0,
                "enc_host_uin": encrypt_uin,
            })
        } else {
            serde_json::json!({
                "disstid": disstid,
                "dirid": 1,
                "tag": false,
                "song_begin": begin,
                "song_num": 100,
                "userinfo": false,
                "orderlist": true,
                "onlysonglist": 0
            })
        };
        let payload = serde_json::json!({
            "comm": {"ct": 19, "cv": 1859},
            "req_1": {
                "module": "music.srfDissInfo.DissInfo",
                "method": "CgiGetDiss",
                "param": param
            }
        });
        let resp = musicu_signed(&payload, Some(&credential_cookie(musicid, musickey)))?;
        let code = resp.pointer("/req_1/code").and_then(|c| c.as_i64()).unwrap_or(0);
        if code != 0 {
            return Err(if code == 104009 {
                "QQ 音乐登录已过期，请重新登录".into()
            } else {
                format!("获取歌单详情失败（code {code}）")
            });
        }
        let list = resp
            .pointer("/req_1/data/songlist")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let got = list.len() as i64;
        for t in &list {
            let mid = t.get("mid").or_else(|| t.get("songmid")).and_then(|v| v.as_str()).unwrap_or("");
            if mid.is_empty() {
                continue;
            }
            let media_mid = t
                .get("media_mid")
                .and_then(|v| v.as_str())
                .or_else(|| t.pointer("/file/media_mid").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            all.push(QqSong {
                id: mid.to_string(),
                name: t.get("name").or_else(|| t.get("songname")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
                singer: t
                    .get("singer")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                            .collect::<Vec<_>>()
                            .join(" / ")
                    })
                    .unwrap_or_default(),
                album: t
                    .pointer("/album/name")
                    .or_else(|| t.get("albumname"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                album_mid: t
                    .pointer("/album/mid")
                    .or_else(|| t.get("albummid"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                media_mid,
                duration_ms: t.get("interval").and_then(|v| v.as_i64()).unwrap_or(0) as u64 * 1000,
                vip: t
                    .pointer("/pay/pay_play")
                    .and_then(|v| v.as_i64())
                    .map(|v| v != 0)
                    .unwrap_or(false),
            });
        }
        if got < 100 || begin > 10000 {
            break;
        }
        // 防呆：接口在某个深度开始重复返回同一页（不再前进）时立即停，
        // 避免把同一批歌重复写入导入结果
        let first_mid = list
            .first()
            .and_then(|t| t.get("mid").or_else(|| t.get("songmid")).and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();
        if !first_mid.is_empty() && first_mid == last_first_mid {
            break;
        }
        last_first_mid = first_mid;
        begin += 100;
    }
    Ok(all)
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
        let r = search("小情歌 苏打绿", 5, 1).expect("search failed");
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
