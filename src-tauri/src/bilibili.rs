//! B 站（哔哩哔哩）视频音频解析模块：免登录走 Web 端公开接口。
//! 流程：BV/av/视频链接（含 b23.tv 短链、?p=N 分P）→ view 拿标题/UP主/封面/分P
//! → playurl(fnval=16) 拿 DASH 音频直链（m4s，AAC-LC fMP4）
//! → 交给引擎按在线音源缓存下载后解码播放（symphonia 支持 fragmented MP4）。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::engine::TrackInfo;

const VIEW_API: &str = "https://api.bilibili.com/x/web-interface/view";
const PLAYURL_API: &str = "https://api.bilibili.com/x/player/playurl";
const PLAYURL_API_WBI: &str = "https://api.bilibili.com/x/player/wbi/v2";
const REFERER: &str = "https://www.bilibili.com/";

/// 音频流偏好档（DASH id，高 → 低）：30280=192kbps、30232=132kbps、30216=64kbps。
/// 30250（杜比）/30251（Hi-Res FLAC）现有解码链不支持，跳过。
const AUDIO_PREF: &[i64] = &[30280, 30232, 30216];

/// 多P视频一次最多加入的分P数（防止超长合集刷爆列表）
const MAX_PAGES: usize = 100;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(20))
        .build()
}

fn re_bv() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"BV[0-9A-Za-z]{10}").unwrap())
}

fn re_av() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\bav([0-9]+)").unwrap())
}

fn re_page() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"[?&]p=([0-9]+)").unwrap())
}

/// 判断输入是否 B 站视频（完整链接 / b23.tv 短链 / BV 号 / av 号 / bili:// 存储链接）。
/// av 号只在明确 B 站语境下匹配，避免误伤含 "av+数字" 的普通链接。
pub fn looks_like_bili(input: &str) -> bool {
    let s = input.trim();
    let l = s.to_lowercase();
    if l.starts_with("bili://") || l.contains("bilibili.com") || l.contains("b23.tv") {
        return true;
    }
    if re_bv().is_match(s) {
        return true;
    }
    re_av().is_match(s) && l.starts_with("av")
}

/// 从输入提取 (bvid 或 aid, 分P页码)。支持完整链接、b23.tv 短链、纯 BV/av 号。
fn extract_id(input: &str) -> Result<(String, Option<u32>), String> {
    let mut s = input.trim().to_string();
    // 短链跟随重定向拿真实视频页地址（只取最终 URL，不读响应体）。
    // 不带协议头的 "b23.tv/xxx"（looks_like_bili 放行）补上 https://，
    // 否则 HTTP 客户端无法请求，与 www.bilibili.com 形式行为不一致
    if s.to_lowercase().contains("b23.tv/") {
        if !s.to_lowercase().starts_with("http://") && !s.to_lowercase().starts_with("https://") {
            s = format!("https://{s}");
        }
        let resp = agent()
            .get(&s)
            .call()
            .map_err(|e| format!("B 站短链解析失败: {e}"))?;
        s = resp.get_url().to_string();
    }
    let page = re_page()
        .captures(&s)
        .and_then(|c| c[1].parse::<u32>().ok())
        .filter(|p| *p >= 1);
    if let Some(m) = re_bv().find(&s) {
        return Ok((m.as_str().to_string(), page));
    }
    if let Some(c) = re_av().captures(&s) {
        return Ok((format!("av{}", &c[1]), page));
    }
    Err("无法从输入中识别 B 站视频（支持 BV 号 / av 号 / 视频链接）".into())
}

/// 接口响应内存缓存：B 站风控按请求频率触发（连续请求 412），缓存直接削减请求量
struct CacheEntry {
    value: Value,
    at: Instant,
}

/// view/playurl 响应 TTL：视频元数据稳定（30min）；音频直链本身有效期数小时（10min）
fn cache_ttl_for(url: &str) -> Option<Duration> {
    if url.contains("/player/playurl") {
        Some(Duration::from_secs(10 * 60))
    } else if url.contains("/web-interface/view") {
        Some(Duration::from_secs(30 * 60))
    } else {
        None
    }
}

/// 缓存容量上限：超限淘汰最旧条目。TTL 过期条目不会被读，但若不删除会永久驻留
/// （每次播放/搜索都会产生新条目），长会话下无上限增长
const CACHE_CAP: usize = 256;

fn with_cache<T>(f: impl FnOnce(&mut HashMap<String, CacheEntry>) -> T) -> Option<T> {
    static CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cache.lock().ok()?;
    Some(f(&mut guard))
}

fn cache_get(url: &str) -> Option<Value> {
    let ttl = cache_ttl_for(url)?;
    // 读到过期条目顺手删除：直链按 cid 各异，不清会越积越多
    with_cache(|map| {
        match map.get(url) {
            Some(e) if e.at.elapsed() < ttl => Some(Some(e.value.clone())),
            Some(_) => {
                map.remove(url);
                Some(None)
            }
            None => Some(None),
        }
    })?
    .flatten()
}

fn cache_put(url: &str, v: Value) {
    // 只有带 TTL 的响应会被 cache_get 读回；其余（搜索/字幕/用户数据等）
    // 存了也不会被读，纯属驻留浪费——这些请求体量大且永不淘汰
    if cache_ttl_for(url).is_none() {
        return;
    }
    with_cache(|map| {
        // 先清过期，再按插入时间淘汰最旧，保证 map 有界
        map.retain(|_, e| e.at.elapsed() < Duration::from_secs(30 * 60));
        while map.len() >= CACHE_CAP {
            if let Some(oldest) = map
                .iter()
                .min_by_key(|(_, e)| e.at)
                .map(|(k, _)| k.clone())
            {
                map.remove(&oldest);
            } else {
                break;
            }
        }
        map.insert(url.to_string(), CacheEntry { value: v, at: Instant::now() });
    });
}

/// 请求 B 站 API 并解析为 JSON。
/// 不携带任何伪装头：实测裸请求最稳——B 站风控会识别"浏览器 UA/Referer 但
/// TLS 指纹非浏览器"的请求并返回 412，裸请求反而稳定放行。
/// 对 412/352 风控（HTTP 或 JSON code）做退避重试，view/playurl 响应做内存缓存。
fn api_json(url: &str) -> Result<Value, String> {
    if let Some(v) = cache_get(url) {
        return Ok(v);
    }
    let mut last_err = String::new();
    for attempt in 0..3u32 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(1000 * attempt as u64));
        }
        let resp = match agent().get(url).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => {
                if (code == 412 || code == 352) && attempt < 3 {
                    last_err = format!("HTTP {code}");
                    continue;
                }
                return Err(format!("请求 B 站接口失败: HTTP {code}"));
            }
            Err(e) => return Err(format!("请求 B 站接口失败: {e}")),
        };
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("解析 B 站接口响应失败: {e}"))?;
        let code = v["code"].as_i64().unwrap_or(0);
        if (code == -412 || code == -352) && attempt < 3 {
            last_err = format!("接口风控 code {code}");
            continue;
        }
        if code == 0 {
            cache_put(url, v.clone());
        }
        return Ok(v);
    }
    Err(format!("请求 B 站接口失败（{last_err}），请稍后重试"))
}

/// 统一把 B 站业务错误码转成用户可读的中文提示
fn bili_error(v: &Value, ctx: &str) -> String {
    let code = v["code"].as_i64().unwrap_or(-1);
    let msg = v["message"].as_str().unwrap_or("");
    match code {
        -404 => format!("{ctx}：视频不存在或已被删除"),
        62002 => format!("{ctx}：稿件不可见"),
        62012 => format!("{ctx}：该视频仅 UP 主自己可见"),
        -403 => format!("{ctx}：无权访问（可能需要大会员或已下架）"),
        _ => format!("{ctx}失败（code {code}）：{msg}"),
    }
}

/// view 接口：拿标题 / UP主 / 封面 / 分P列表（免登录可用）
fn fetch_view(id: &str) -> Result<Value, String> {
    let query = match id.strip_prefix("av") {
        Some(aid) => format!("aid={aid}"),
        None => format!("bvid={id}"),
    };
    let v = api_json(&format!("{VIEW_API}?{query}"))?;
    if v["code"].as_i64() != Some(0) {
        return Err(bili_error(&v, "获取视频信息"));
    }
    Ok(v["data"].clone())
}

fn quality_label(id: i64) -> String {
    match id {
        30280 => "192k".into(),
        30232 => "132k".into(),
        30216 => "64k".into(),
        other => format!("{other}"),
    }
}

/// 解析内部曲目标识 rid：完整 "BVxxx-cid"，或纯 "BVxxx"（空间列表条目——
/// 播放/取字幕时再查 view 补 cid，bvid→cid 结果进程内缓存）。
pub fn parse_rid(rid: &str) -> Result<(String, i64), String> {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"^(BV[0-9A-Za-z]{10})-([0-9]+)$").unwrap());
    if let Some(c) = re.captures(rid.trim()) {
        let cid = c[2]
            .parse::<i64>()
            .map_err(|_| "B 站曲目 cid 无效".to_string())?;
        return Ok((c[1].to_string(), cid));
    }
    static RE_BV: OnceLock<regex::Regex> = OnceLock::new();
    let re_bv = RE_BV.get_or_init(|| regex::Regex::new(r"^BV[0-9A-Za-z]{10}$").unwrap());
    let s = rid.trim();
    if re_bv.is_match(s) {
        let bvid = s.to_string();
        static CACHE: OnceLock<Mutex<HashMap<String, i64>>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(m) = cache.lock() {
            if let Some(cid) = m.get(&bvid) {
                return Ok((bvid, *cid));
            }
        }
        let view = fetch_view(&bvid)?;
        let cid = view["cid"].as_i64().ok_or("视频缺少 cid")?;
        if let Ok(mut m) = cache.lock() {
            m.insert(bvid.clone(), cid);
        }
        return Ok((bvid, cid));
    }
    Err("B 站曲目标识无效（应为 BVxxx 或 BVxxx-cid）".into())
}

/// 取音频直链（bilibili_play / download_online 共用；playurl 响应有 10min 内存缓存）
pub fn audio_stream(bvid: &str, cid: i64) -> Result<(String, String), String> {
    fetch_audio(bvid, cid)
}

/// playurl(fnval=16)：返回 (音频直链, 音质标签)。免登录可用。
fn fetch_audio(bvid: &str, cid: i64) -> Result<(String, String), String> {
    let url = format!("{PLAYURL_API}?bvid={bvid}&cid={cid}&qn=0&fnval=16&fnver=0&fourk=1");
    let v = api_json(&url)?;
    if v["code"].as_i64() != Some(0) {
        return Err(bili_error(&v, "获取音频流"));
    }
    let data = &v["data"];
    // DASH 音频：按偏好档从高到低选（AAC-LC，symphonia 可直接解）
    if let Some(audio) = data["dash"]["audio"].as_array() {
        for &want in AUDIO_PREF {
            if let Some(a) = audio.iter().find(|a| a["id"].as_i64() == Some(want)) {
                if let Some(u) = a["baseUrl"].as_str() {
                    return Ok((u.to_string(), quality_label(want)));
                }
            }
        }
        // 偏好档都不可用：取第一个 AAC 流兜底
        if let Some(a) = audio
            .iter()
            .find(|a| a["codecs"].as_str().map(|c| c.starts_with("mp4a")).unwrap_or(false))
        {
            if let Some(u) = a["baseUrl"].as_str() {
                let id = a["id"].as_i64().unwrap_or(30216);
                return Ok((u.to_string(), quality_label(id)));
            }
        }
    }
    Err("该视频没有可用的独立音频流".into())
}

/// 解析输入为可加入音源列表的条目列表（多P视频每个分P一条）。
/// 返回 (存储链接, 标题)；存储链接用 bili:// 协议，播放时再解析真实音频直链
/// （B 站 CDN 直链带过期签名，不能落库）。
pub fn resolve_pages(input: &str) -> Result<Vec<(String, String)>, String> {
    let (id, _) = extract_id(input)?;
    let view = fetch_view(&id)?;
    let bvid = view["bvid"].as_str().unwrap_or(&id).to_string();
    let title = view["title"].as_str().unwrap_or("B站视频").to_string();
    let mut rows = Vec::new();
    let pages: Vec<Value> = view["pages"].as_array().cloned().unwrap_or_default();
    if pages.len() <= 1 {
        rows.push((format!("bili://{bvid}"), title));
    } else {
        for p in pages.iter().take(MAX_PAGES) {
            let n = p["page"].as_u64().unwrap_or(1);
            let part = p["part"].as_str().unwrap_or("");
            let name = if part.is_empty() {
                format!("{title} P{n}")
            } else {
                format!("{title} P{n}·{part}")
            };
            rows.push((format!("bili://{bvid}?p={n}"), name));
        }
    }
    Ok(rows)
}

/// 播放解析：bili:// 存储链接或用户输入的 B 站链接 → (音频直链, 曲目元信息)。
/// 返回 Ok(None) 表示不是 B 站链接（调用方走原有通用音源逻辑）。
pub fn resolve(input: &str) -> Result<Option<(String, TrackInfo)>, String> {
    let s = input.trim();
    // bili://BVxxx?p=N → 还原成视频页链接统一解析
    let stored = match s.strip_prefix("bili://") {
        Some(rest) => format!("https://www.bilibili.com/video/{rest}"),
        None => s.to_string(),
    };
    if !looks_like_bili(&stored) {
        return Ok(None);
    }
    let (id, page) = extract_id(&stored)?;
    let view = fetch_view(&id)?;
    let bvid = view["bvid"].as_str().unwrap_or(&id).to_string();
    let video_title = view["title"].as_str().unwrap_or("B站视频").to_string();
    let artist = view["owner"]["name"].as_str().unwrap_or("B站UP主").to_string();
    // 封面统一转 https：WebView 下 http 图片可能被混合内容策略拦截
    let cover = view["pic"].as_str().unwrap_or("").replace("http://", "https://");

    // 定位分P（?p=N，默认 P1）；无 pages 数据时用顶层 cid 兜底
    let pages: Vec<Value> = view["pages"].as_array().cloned().unwrap_or_default();
    let want = page.unwrap_or(1).max(1) as usize;
    let (cid, part, duration_s, multi) = if pages.is_empty() {
        (
            view["cid"].as_i64().ok_or("视频信息异常：缺少 cid")?,
            "",
            view["duration"].as_u64().unwrap_or(0),
            false,
        )
    } else {
        let p = pages
            .get(want - 1)
            .ok_or_else(|| format!("分P {want} 不存在（该视频共 {} 个分P）", pages.len()))?;
        (
            p["cid"].as_i64().ok_or("分P信息异常：缺少 cid")?,
            p["part"].as_str().unwrap_or(""),
            p["duration"].as_u64().unwrap_or(0),
            pages.len() > 1,
        )
    };
    let title = if multi {
        if part.is_empty() {
            format!("{video_title} P{want}")
        } else {
            format!("{video_title} P{want}·{part}")
        }
    } else {
        video_title
    };

    let (audio_url, quality) = fetch_audio(&bvid, cid)?;

    let info = TrackInfo {
        id: None,
        kind: "bilibili".into(),
        path: String::new(),
        title,
        artist,
        album: "哔哩哔哩".into(),
        cover,
        duration_ms: duration_s * 1000,
        nid: None,
        // qid 承载 "bvid-cid" 作缓存键：同一分P同一音质只缓存一份
        //（CDN 直链每次签名都变，不能按 URL 哈希缓存）
        qid: Some(format!("{bvid}-{cid}")),
        kgid: None,
        quality: Some(quality),
    };
    Ok(Some((audio_url, info)))
}

// ---------- 登录（网页扫码）----------

const PASSPORT_API: &str = "https://passport.bilibili.com";

/// WBI 签名混淆表（B 站前端 JS 固定常量）
const WBI_TAB: &[usize] = &[
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19,
    29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4,
    22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

#[derive(Clone, Debug)]
pub struct BiliCookies {
    pub sessdata: String,
    pub buvid3: String,
}

impl BiliCookies {
    pub fn is_empty(&self) -> bool {
        self.sessdata.is_empty()
    }
    pub fn header(&self) -> String {
        let mut parts = Vec::new();
        if !self.sessdata.is_empty() {
            parts.push(format!("SESSDATA={}", self.sessdata));
        }
        if !self.buvid3.is_empty() {
            parts.push(format!("buvid3={}", self.buvid3));
        }
        parts.join("; ")
    }
}

/// 登录流程的设备指纹（buvid3）：generate 与 poll 必须携带同一个 buvid3
/// （浏览器行为即如此），否则确认事件可能对不上轮询会话。
pub fn login_buvid3() -> String {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            api_json_auth("https://api.bilibili.com/x/frontend/finger/spi", None)
                .ok()
                .and_then(|v| v["data"]["b_3"].as_str().map(|s| s.to_string()))
                .unwrap_or_default()
        })
        .clone()
}

fn qr_debug_log(msg: &str) {
    use std::io::Write;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::temp_dir().join("rustmusic_bili_qr.log"))
    {
        let _ = writeln!(f, "{ts} {msg}");
    }
}

/// 生成扫码登录二维码：返回 (qrcode_key, qr_png_data_url)。
/// 匿名可用；二维码内容是 passport 下发的登录页 URL（B 站 App 扫码确认）。
pub fn login_qr_create() -> Result<(String, String), String> {
    let buvid3 = login_buvid3();
    let v = api_json_auth(
        &format!("{PASSPORT_API}/x/passport-login/web/qrcode/generate"),
        Some(&BiliCookies {
            sessdata: String::new(),
            buvid3: buvid3.clone(),
        }),
    )?;
    if v["code"].as_i64() != Some(0) {
        return Err(format!("获取登录二维码失败: {v}"));
    }
    let key = v["data"]["qrcode_key"]
        .as_str()
        .ok_or("登录二维码响应缺少 qrcode_key")?
        .to_string();
    let url = v["data"]["url"]
        .as_str()
        .ok_or("登录二维码响应缺少 url")?
        .to_string();
    let code = qrcode::QrCode::with_error_correction_level(url.as_bytes(), qrcode::EcLevel::M)
        .map_err(|e| format!("生成二维码失败: {e:?}"))?;
    let img = code
        .render::<image::Luma<u8>>()
        .quiet_zone(true)
        .min_dimensions(240, 240)
        .build();
    let mut png = Vec::new();
    image::DynamicImage::ImageLuma8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut png),
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("编码二维码图片失败: {e}"))?;
    
    let mut b64 = String::new();
    b64_encode_into(&png, &mut b64);
    qr_debug_log(&format!("gen key={key}"));
    Ok((key, format!("data:image/png;base64,{b64}")))
}

fn b64_encode_into(data: &[u8], out: &mut String) {
    const TBL: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TBL[(n >> 18) as usize & 63] as char);
        out.push(TBL[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TBL[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TBL[n as usize & 63] as char
        } else {
            '='
        });
    }
}

#[derive(Clone, Debug)]
pub struct QrPoll {
    /// waiting | scanned | success | expired
    pub status: String,
    pub nickname: Option<String>,
    pub cookies: Option<BiliCookies>,
}

/// 轮询扫码状态。成功时从 data.url 的登录参数里提取 SESSDATA，
/// 并顺手领一个设备指纹 buvid3（后续字幕/播放接口的 cookie 搭档），
/// 再用 nav 接口拿昵称。
pub fn login_qr_check(key: &str) -> Result<QrPoll, String> {
    let buvid3 = login_buvid3();
    // poll 需要自管请求：成功时凭证在 Set-Cookie 头里，api_json_auth 拿不到
    let mut req = agent().get(&format!(
        "{PASSPORT_API}/x/passport-login/web/qrcode/poll?qrcode_key={key}"
    ));
    req = req
        .set("User-Agent", UA_WEB)
        .set("Referer", REFERER);
    if !buvid3.is_empty() {
        req = req.set("Cookie", &format!("buvid3={buvid3}"));
    }
    let resp = req
        .call()
        .map_err(|e| format!("登录状态查询失败: {e}"))?;
    let set_cookies: Vec<String> = resp.all("Set-Cookie").into_iter().map(String::from).collect();
    let v: Value = resp
        .into_json()
        .map_err(|e| format!("解析登录响应失败: {e}"))?;
    if v["code"].as_i64() != Some(0) {
        return Err(format!("登录状态查询失败: {v}"));
    }
    let code = v["data"]["code"].as_i64().unwrap_or(-1);
    qr_debug_log(&format!(
        "poll key_tail={} code={} msg={} setcookie_n={} url={}",
        &key[key.len().saturating_sub(6)..],
        code,
        v["data"]["message"].as_str().unwrap_or(""),
        set_cookies.len(),
        v["data"]["url"].as_str().unwrap_or("")
    ));
    let status = match code {
        0 => "success",
        86090 => "scanned",
        86038 => "expired",
        _ => "waiting",
    };
    if status != "success" {
        return Ok(QrPoll {
            status: status.into(),
            nickname: None,
            cookies: None,
        });
    }
    // 凭证优先从 Set-Cookie 取（官方机制）；data.url 查询串兜底
    let mut sessdata = set_cookies
        .iter()
        .find_map(|c| {
            c.split(';')
                .next()
                .and_then(|seg| seg.trim().strip_prefix("SESSDATA="))
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();
    if sessdata.is_empty() {
        let login_url = v["data"]["url"].as_str().unwrap_or("");
        for pair in login_url.split(['?', '&']) {
            let mut kv = pair.splitn(2, '=');
            if kv.next() == Some("SESSDATA") {
                sessdata = kv.next().unwrap_or("").to_string();
            }
        }
    }
    if sessdata.is_empty() {
        return Err("登录成功但未取得会话凭证（SESSDATA）".into());
    }
    let cookies = BiliCookies { sessdata, buvid3 };
    let nickname = nav_uname(&cookies);
    Ok(QrPoll {
        status: "success".into(),
        nickname,
        cookies: Some(cookies),
    })
}

/// 当前登录用户昵称（未登录返回 None）
fn nav_uname(cookies: &BiliCookies) -> Option<String> {
    if cookies.is_empty() {
        return None;
    }
    let v = api_json_auth(
        "https://api.bilibili.com/x/web-interface/nav",
        Some(cookies),
    )
    .ok()?;
    let uname = v["data"]["uname"].as_str()?;
    (!uname.is_empty()).then(|| uname.to_string())
}

/// WBI 签名密钥（img+sub 混排前 32 位），缓存 1 小时
fn wbi_mixin_key() -> Result<String, String> {
    static CACHE: OnceLock<Mutex<Option<(String, Instant)>>> = OnceLock::new();
    let slot = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = slot.lock().map_err(|_| "wbi 缓存锁失败")?;
    if let Some((key, at)) = guard.as_ref() {
        if at.elapsed() < Duration::from_secs(3600) {
            return Ok(key.clone());
        }
    }
    let v = api_json("https://api.bilibili.com/x/web-interface/nav")?;
    let img = v["data"]["wbi_img"]["img_url"]
        .as_str()
        .and_then(|u| u.rsplit('/').next())
        .unwrap_or("")
        .trim_end_matches(".png");
    let sub = v["data"]["wbi_img"]["sub_url"]
        .as_str()
        .and_then(|u| u.rsplit('/').next())
        .unwrap_or("")
        .trim_end_matches(".png");
    if img.is_empty() || sub.is_empty() {
        return Err("获取 WBI 密钥失败".into());
    }
    let raw: Vec<u8> = format!("{img}{sub}").into_bytes();
    let mut key = String::new();
    for &i in WBI_TAB {
        if let Some(&c) = raw.get(i) {
            key.push(c as char);
        }
    }
    key.truncate(32);
    *guard = Some((key.clone(), Instant::now()));
    Ok(key)
}

fn md5_hex(s: &str) -> String {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

/// 对 player/wbi/v2 的参数做 WBI 签名，返回完整查询串（含 wts/w_rid）
fn wbi_sign(params: &str) -> Result<String, String> {
    let key = wbi_mixin_key()?;
    let wts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut pairs: Vec<(String, String)> = params
        .split('&')
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .collect();
    pairs.push(("wts".into(), wts.to_string()));
    pairs.sort();
    let query = pairs
        .iter()
        .map(|(k, v)| {
            // B 站签名规则：值里 !'()* 转十六进制
            let v: String = v
                .chars()
                .map(|c| {
                    if "!\'()*".contains(c) {
                        format!("%{:02X}", c as u32)
                    } else {
                        c.to_string()
                    }
                })
                .collect();
            format!("{k}={v}")
        })
        .collect::<Vec<_>>()
        .join("&");
    let w_rid = md5_hex(&format!("{query}{key}"));
    Ok(format!("{query}&w_rid={w_rid}"))
}

/// 带 Cookie 请求 B 站 API 并解析 JSON（登录态接口用）
fn api_json_auth(url: &str, cookies: Option<&BiliCookies>) -> Result<Value, String> {
    api_json_auth_ref(url, cookies, REFERER)
}

/// 同上，可指定 Referer；对 412/352 风控退避重试（空间接口按来源页校验）
fn api_json_auth_ref(
    url: &str,
    cookies: Option<&BiliCookies>,
    referer: &str,
) -> Result<Value, String> {
    let mut last_err = String::new();
    // 空间接口风控较凶：4 次尝试、递增退避（含尾部 3.5s）
    for attempt in 0..4u32 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(1000 * attempt as u64));
        }
        let mut req = agent().get(url);
        if let Some(c) = cookies {
            if !c.is_empty() {
                req = req.set("Cookie", &c.header());
            }
        }
        req = req.set("User-Agent", UA_WEB).set("Referer", referer);
        let resp = match req.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => {
                if (code == 412 || code == 352) && attempt < 2 {
                    last_err = format!("HTTP {code}");
                    continue;
                }
                return Err(format!("请求 B 站接口失败: HTTP {code}"));
            }
            Err(e) => return Err(format!("请求 B 站接口失败: {e}")),
        };
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("解析 B 站接口响应失败: {e}"))?;
        let code = v["code"].as_i64().unwrap_or(0);
        if (code == -412 || code == -352) && attempt < 2 {
            last_err = format!("接口风控 code {code}");
            continue;
        }
        return Ok(v);
    }
    Err(format!("请求 B 站接口失败（{last_err}），请稍后重试"))
}

const SPACE_REFERER: &str = "https://space.bilibili.com/";

const UA_WEB: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// 拉取视频字幕并转成逐行歌词。返回 None = 没有可用字幕。
/// 字幕列表必须登录（匿名/WBI 签名一律返回空），故需传入登录 Cookie。
pub fn subtitle_lyrics(
    bvid: &str,
    cid: &str,
    cookies: Option<&BiliCookies>,
) -> Result<Option<crate::models::LyricsPayload>, String> {
    let signed = wbi_sign(&format!("bvid={bvid}&cid={cid}"))?;
    let v = api_json_auth(&format!("{PLAYURL_API_WBI}?{signed}"), cookies)?;
    if v["code"].as_i64() != Some(0) {
        return Err(bili_error(&v, "获取字幕列表"));
    }
    let subs = v["data"]["subtitle"]["subtitles"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if subs.is_empty() {
        return Ok(None);
    }
    // 语言优先级：简中人工 > 简中 AI > 其他
    let pick = subs
        .iter()
        .find(|s| s["lan"].as_str() == Some("zh-Hans"))
        .or_else(|| subs.iter().find(|s| s["lan"].as_str() == Some("zh-CN")))
        .or_else(|| subs.iter().find(|s| s["lan"].as_str().unwrap_or("").starts_with("ai-zh")))
        .or_else(|| subs.iter().find(|s| s["lan"].as_str().unwrap_or("").starts_with("zh")))
        .or_else(|| subs.first());
    let sub_url = pick
        .and_then(|s| s["subtitle_url"].as_str())
        .map(|u| {
            if u.starts_with("//") {
                format!("https:{u}")
            } else {
                u.to_string()
            }
        })
        .ok_or("字幕项缺少下载地址")?;
    let mut req = agent().get(&sub_url);
    req = req
        .set("User-Agent", UA_WEB)
        .set("Referer", REFERER);
    let sub: Value = req
        .call()
        .map_err(|e| format!("下载字幕失败: {e}"))?
        .into_json()
        .map_err(|e| format!("解析字幕失败: {e}"))?;
    let body = sub["body"].as_array().cloned().unwrap_or_default();
    let mut lines: Vec<crate::models::LyricLine> = body
        .iter()
        .filter_map(|b| {
            let from = b["from"].as_f64()?;
            let text = b["content"].as_str()?.trim().to_string();
            if text.is_empty() {
                return None;
            }
            Some(crate::models::LyricLine {
                time_ms: Some((from * 1000.0).round() as u64),
                text,
                words: None,
                trans: None,
            })
        })
        .collect();
    if lines.is_empty() {
        return Ok(None);
    }
    lines.sort_by_key(|l| l.time_ms.unwrap_or(0));
    Ok(Some(crate::models::LyricsPayload::new(true, lines)))
}

// ---------- UP 主空间 ----------

const CARD_API: &str = "https://api.bilibili.com/x/web-interface/card";
const SPACE_SEARCH_API: &str = "https://api.bilibili.com/x/space/wbi/arc/search";
const SEASONS_LIST_API: &str = "https://api.bilibili.com/x/polymer/web-space/seasons_series_list";
const SEASON_ARCHIVES_API: &str =
    "https://api.bilibili.com/x/polymer/web-space/seasons_archives_list";
const SERIES_ARCHIVES_API: &str = "https://api.bilibili.com/x/series/archives";

/// 空间视频条目（rid 为纯 bvid，播放时按需解析 cid）
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceVideo {
    pub bvid: String,
    pub title: String,
    pub cover: String,
    pub duration_ms: u64,
    pub play: u64,
    pub created: i64,
}

/// UP 主信息（card 接口，匿名可用）
pub fn space_card(mid: &str) -> Result<(String, String, String, u64), String> {
    let v = api_json(&format!("{CARD_API}?mid={mid}&photo=true"))?;
    if v["code"].as_i64() != Some(0) {
        return Err(bili_error(&v, "获取 UP 主信息"));
    }
    let name = v["data"]["card"]["name"].as_str().unwrap_or("未知UP主");
    let face = v["data"]["card"]["face"]
        .as_str()
        .unwrap_or("")
        .replace("http://", "https://");
    let fans = v["data"]["follower"]
        .as_u64()
        .or_else(|| v["data"]["card"]["fans"].as_u64())
        .unwrap_or(0);
    let total = v["data"]["archive_count"].as_u64().unwrap_or(0);
    Ok((name.into(), face, format_fans(fans), total))
}

fn format_fans(n: u64) -> String {
    if n >= 10_000 {
        format!("{:.1}万粉丝", n as f64 / 10_000.0)
    } else {
        format!("{n}粉丝")
    }
}

/// 解析空间输入：完整链接 / 纯 mid 数字
pub fn parse_space(input: &str) -> Result<String, String> {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"space\.bilibili\.com/(\d+)").unwrap());
    if let Some(c) = re.captures(input.trim()) {
        return Ok(c[1].to_string());
    }
    let s = input.trim();
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        return Ok(s.to_string());
    }
    Err("无法从输入识别 UP 主（支持 space.bilibili.com 链接或纯数字 UID）".into())
}

/// "MM:SS" / "HH:MM:SS" → 毫秒
fn parse_length(s: &str) -> u64 {
    let parts: Vec<u64> = s.split(':').filter_map(|p| p.parse().ok()).collect();
    match parts.len() {
        2 => (parts[0] * 60 + parts[1]) * 1000,
        3 => (parts[0] * 3600 + parts[1] * 60 + parts[2]) * 1000,
        _ => 0,
    }
}

fn parse_vlist(v: &Value) -> Vec<SpaceVideo> {
    (v["list"]["vlist"]
        .as_array()
        .cloned()
        .unwrap_or_default())
    .iter()
    .map(|it| SpaceVideo {
        bvid: it["bvid"].as_str().unwrap_or("").to_string(),
        title: it["title"].as_str().unwrap_or("").to_string(),
        cover: it["pic"]
            .as_str()
            .unwrap_or("")
            .replace("http://", "https://"),
        duration_ms: parse_length(it["length"].as_str().unwrap_or("")),
        play: it["play"].as_u64().unwrap_or(0),
        created: it["created"].as_i64().unwrap_or(0),
    })
    .filter(|s| !s.bvid.is_empty())
    .collect()
}

/// UP 主投稿列表（WBI 签名；order: pubdate|click|stow，服务端对全部投稿排序）。
/// stow（最多收藏）匿名会被风控 -352，登录态可用。
pub fn space_videos(
    mid: &str,
    order: &str,
    pn: u32,
    cookies: Option<&BiliCookies>,
) -> Result<(Vec<SpaceVideo>, u64, bool), String> {
    if !matches!(order, "pubdate" | "click" | "stow") {
        return Err("未知排序方式".into());
    }
    let signed = wbi_sign(&format!("mid={mid}&order={order}&pn={pn}&ps=30"))?;
    let v = api_json_auth_ref(&format!("{SPACE_SEARCH_API}?{signed}"), cookies, SPACE_REFERER)?;
    if v["code"].as_i64() != Some(0) {
        if v["code"].as_i64() == Some(-352) {
            return Err("按收藏排序需要先登录 B 站（右上角「B站登录」）".into());
        }
        return Err(bili_error(&v, "获取 UP 主视频列表"));
    }
    let items = parse_vlist(&v["data"]);
    // 新版响应不再带 list.count：total=0 时按“返回满 30 条”判断还有更多
    let total = v["data"]["list"]["count"].as_u64().unwrap_or(0);
    let has_more = if total > 0 {
        (pn as u64) * 30 < total
    } else {
        items.len() as u64 >= 30
    };
    Ok((items, total, has_more))
}

/// 合集/系列条目
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpaceCollection {
    pub id: i64,
    pub kind: String, // "season" | "series"
    pub title: String,
    pub total: u64,
}

/// UP 主的合集与系列列表（WBI 签名；无合集返回空数组）
pub fn space_seasons(
    mid: &str,
    cookies: Option<&BiliCookies>,
) -> Result<Vec<SpaceCollection>, String> {
    let signed = wbi_sign(&format!("mid={mid}&page_num=1&page_size=20"))?;
    let v = api_json_auth_ref(&format!("{SEASONS_LIST_API}?{signed}"), cookies, SPACE_REFERER)?;
    if v["code"].as_i64() != Some(0) {
        return Err(bili_error(&v, "获取合集列表"));
    }
    let mut out: Vec<SpaceCollection> = Vec::new();
    for s in v["data"]["seasons"].as_array().cloned().unwrap_or_default() {
        out.push(SpaceCollection {
            id: s["season_id"].as_i64().unwrap_or(0),
            kind: "season".into(),
            title: s["name"].as_str().unwrap_or("合集").to_string(),
            total: s["total"].as_u64().unwrap_or(0),
        });
    }
    for s in v["data"]["series"].as_array().cloned().unwrap_or_default() {
        out.push(SpaceCollection {
            id: s["series_id"].as_i64().unwrap_or(0),
            kind: "series".into(),
            title: s["name"].as_str().unwrap_or("系列").to_string(),
            total: s["total"].as_u64().unwrap_or(0),
        });
    }
    Ok(out)
}

/// 合集（season）视频列表，分页
pub fn space_season_archives(
    mid: &str,
    season_id: i64,
    pn: u32,
    cookies: Option<&BiliCookies>,
) -> Result<(Vec<SpaceVideo>, u64, bool), String> {
    let signed = wbi_sign(&format!(
        "mid={mid}&page_num={pn}&page_size=30&season_id={season_id}"
    ))?;
    let v = api_json_auth_ref(&format!("{SEASON_ARCHIVES_API}?{signed}"), cookies, SPACE_REFERER)?;
    if v["code"].as_i64() != Some(0) {
        return Err(bili_error(&v, "获取合集视频"));
    }
    let items: Vec<SpaceVideo> = (v["data"]["archives"].as_array().cloned().unwrap_or_default())
        .iter()
        .map(|it| SpaceVideo {
            bvid: it["bvid"].as_str().unwrap_or("").to_string(),
            title: it["title"].as_str().unwrap_or("").to_string(),
            cover: it["pic"]
                .as_str()
                .unwrap_or("")
                .replace("http://", "https://"),
            duration_ms: (it["duration"].as_u64().unwrap_or(0)) * 1000,
            play: it["stat"]["view"].as_u64().unwrap_or(0),
            created: it["pubdate"].as_i64().unwrap_or(0),
        })
        .filter(|s| !s.bvid.is_empty())
        .collect();
    let total = v["data"]["items_total"]
        .as_u64()
        .unwrap_or(items.len() as u64);
    let has_more = (pn as u64) * 30 < total;
    Ok((items, total, has_more))
}

/// 系列（series）视频列表，分页
pub fn space_series_archives(
    mid: &str,
    series_id: i64,
    pn: u32,
    cookies: Option<&BiliCookies>,
) -> Result<(Vec<SpaceVideo>, u64, bool), String> {
    let v = api_json_auth_ref(
        &format!("{SERIES_ARCHIVES_API}?mid={mid}&pn={pn}&ps=30&series_id={series_id}"),
        cookies,
        SPACE_REFERER,
    )?;
    if v["code"].as_i64() != Some(0) {
        return Err(bili_error(&v, "获取系列视频"));
    }
    let items: Vec<SpaceVideo> = (v["data"]["archives"].as_array().cloned().unwrap_or_default())
        .iter()
        .map(|it| SpaceVideo {
            bvid: it["bvid"].as_str().unwrap_or("").to_string(),
            title: it["title"].as_str().unwrap_or("").to_string(),
            cover: it["pic"]
                .as_str()
                .unwrap_or("")
                .replace("http://", "https://"),
            duration_ms: (it["duration"].as_u64().unwrap_or(0)) * 1000,
            play: it["stat"]["view"].as_u64().unwrap_or(0),
            created: it["pubdate"].as_i64().unwrap_or(0),
        })
        .filter(|s| !s.bvid.is_empty())
        .collect();
    let total = v["data"]["total"].as_u64().unwrap_or(items.len() as u64);
    let has_more = (pn as u64) * 30 < total;
    Ok((items, total, has_more))
}

// ---------- 登录用户的收藏夹 ----------

const NAV_API: &str = "https://api.bilibili.com/x/web-interface/nav";
const FAV_FOLDER_API: &str = "https://api.bilibili.com/x/v3/fav/folder/created/list";
const FAV_RESOURCE_API: &str = "https://api.bilibili.com/x/v3/fav/resource/list";
const FAV_REFERER: &str = "https://space.bilibili.com";

/// 登录用户信息（nav 接口，需有效 SESSDATA）
pub struct NavUser {
    pub mid: String,
    pub uname: String,
    pub face: String,
}

pub fn nav_user(cookies: &BiliCookies) -> Result<NavUser, String> {
    let v = api_json_auth_ref(NAV_API, Some(cookies), "https://www.bilibili.com")?;
    if v["code"].as_i64() != Some(0) {
        return Err("获取 B 站登录信息失败（登录可能已过期，请重新扫码）".into());
    }
    let mid = v["data"]["mid"].as_u64().unwrap_or(0);
    if mid == 0 {
        return Err("B 站未登录".into());
    }
    Ok(NavUser {
        mid: mid.to_string(),
        uname: v["data"]["uname"].as_str().unwrap_or("").to_string(),
        face: v["data"]["face"]
            .as_str()
            .unwrap_or("")
            .replace("http://", "https://"),
    })
}

/// 收藏夹（用户自建）
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavFolder {
    pub id: i64,
    pub title: String,
    pub total: u64,
}

/// 登录用户创建的收藏夹列表（需 SESSDATA，私密收藏夹一并可见）。
/// 同时带回登录用户信息（昵称/头像，供结果区展示）
pub fn fav_folders(cookies: &BiliCookies) -> Result<(NavUser, Vec<FavFolder>), String> {
    let user = nav_user(cookies)?;
    let v = api_json_auth_ref(
        &format!("{FAV_FOLDER_API}?up_mid={}&pn=1&ps=100", user.mid),
        Some(cookies),
        FAV_REFERER,
    )?;
    if v["code"].as_i64() != Some(0) {
        return Err(bili_error(&v, "获取收藏夹列表"));
    }
    let mut out = Vec::new();
    for f in v["data"]["list"].as_array().cloned().unwrap_or_default() {
        let id = f["id"].as_i64().unwrap_or(0);
        let title = f["title"].as_str().unwrap_or("").trim().to_string();
        if id == 0 || title.is_empty() {
            continue;
        }
        out.push(FavFolder {
            id,
            title,
            total: f["media_count"].as_u64().unwrap_or(0),
        });
    }
    Ok((user, out))
}

/// 收藏夹内容分页：只收普通视频（type=2，带 bvid），音频/合集条目不可播。
/// 返回与合集一致的 (条目, 总数, 是否还有更多)
pub fn fav_folder_videos(
    media_id: i64,
    pn: u32,
    cookies: &BiliCookies,
) -> Result<(Vec<SpaceVideo>, u64, bool), String> {
    let v = api_json_auth_ref(
        &format!(
            "{FAV_RESOURCE_API}?media_id={media_id}&pn={pn}&ps=30&keyword=&order=mtime&type=0&tid=0&platform=web"
        ),
        Some(cookies),
        FAV_REFERER,
    )?;
    if v["code"].as_i64() != Some(0) {
        return Err(bili_error(&v, "获取收藏夹内容"));
    }
    let items: Vec<SpaceVideo> = (v["data"]["medias"].as_array().cloned().unwrap_or_default())
        .iter()
        .filter(|it| it["type"].as_i64() == Some(2))
        .map(|it| SpaceVideo {
            bvid: it["bv_id"].as_str().unwrap_or("").to_string(),
            title: it["title"].as_str().unwrap_or("").to_string(),
            cover: it["cover"]
                .as_str()
                .unwrap_or("")
                .replace("http://", "https://"),
            duration_ms: (it["duration"].as_u64().unwrap_or(0)) * 1000,
            play: it["cnt_info"]["play"].as_u64().unwrap_or(0),
            created: it["pubdate"].as_i64().unwrap_or(0),
        })
        .filter(|s| !s.bvid.is_empty())
        .collect();
    let total = v["data"]["info"]["media_count"]
        .as_u64()
        .unwrap_or(items.len() as u64);
    let has_more = (pn as u64) * 30 < total;
    Ok((items, total, has_more))
}

/// rid 为 "BVxxx-cid"。不落库——列表只展示当前解析结果。
/// 返回 (rid, title, artist, cover, duration_ms) 列表。
pub fn video_rows(input: &str) -> Result<Vec<(String, String, String, String, u64)>, String> {
    let (id, _) = extract_id(input)?;
    let view = fetch_view(&id)?;
    let bvid = view["bvid"].as_str().unwrap_or(&id).to_string();
    let artist = view["owner"]["name"]
        .as_str()
        .unwrap_or("哔哩哔哩")
        .to_string();
    let cover = view["pic"]
        .as_str()
        .unwrap_or("")
        .replace("http://", "https://");
    let video_title = view["title"].as_str().unwrap_or("B站视频").to_string();
    let pages: Vec<Value> = view["pages"].as_array().cloned().unwrap_or_default();
    let mut rows = Vec::new();
    if pages.len() <= 1 {
        let cid = view["cid"].as_i64().unwrap_or(0);
        let dur = view["duration"].as_u64().unwrap_or(0);
        rows.push((
            format!("{bvid}-{cid}"),
            video_title,
            artist,
            cover,
            dur * 1000,
        ));
    } else {
        for p in pages.iter().take(MAX_PAGES) {
            let n = p["page"].as_u64().unwrap_or(1);
            let cid = p["cid"].as_i64().unwrap_or(0);
            let dur = p["duration"].as_u64().unwrap_or(0);
            let part = p["part"].as_str().unwrap_or("");
            let title = if part.is_empty() {
                format!("{video_title} P{n}")
            } else {
                format!("{video_title} P{n}·{part}")
            };
            rows.push((
                format!("{bvid}-{cid}"),
                title,
                artist.clone(),
                cover.clone(),
                dur * 1000,
            ));
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod login_tests {
    use super::*;

    /// 实证：生成二维码后连续轮询，应持续 waiting（86101）而非 expired（86038）。
    #[test]
    #[ignore]
    fn qr_generate_poll_waiting() {
        let (key, qr) = login_qr_create().expect("生成失败");
        println!("qr prefix: {}", &qr[..40.min(qr.len())]);
        for i in 0..4 {
            let r = login_qr_check(&key).expect("轮询失败");
            println!("poll {}: status={}", i, r.status);
            assert_eq!(r.status, "waiting", "第 {i} 次轮询应为 waiting");
            std::thread::sleep(Duration::from_secs(2));
        }
    }

    /// 空间模块：card + 投稿列表（最新/播放排序）+ 合集列表。
    /// 匿名在连续调用下会被风控，用应用已保存的登录态跑。
    #[test]
    #[ignore]
    fn e2e_space() {
        // 从应用库读登录 cookie（未登录也能跑，只是可能撞风控）
        let cookies = (|| -> Option<BiliCookies> {
            let db = std::env::var("APPDATA").ok()?;
            let conn = rusqlite::Connection::open_with_flags(
                std::path::Path::new(&db).join("com.rustmusic.app/library.db"),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .ok()?;
            let get = |k: &str| -> String {
                conn.query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    [k],
                    |r| r.get(0),
                )
                .unwrap_or_default()
            };
            let sessdata = get("bili_sessdata");
            if sessdata.is_empty() {
                return None;
            }
            Some(BiliCookies {
                sessdata,
                buvid3: get("bili_buvid3"),
            })
        })();
        let mid = parse_space("https://space.bilibili.com/229733301").expect("解析 mid 失败");
        let (name, _face, fans, total) = space_card(&mid).expect("card 失败");
        println!("card: {name} {fans} total={total}");
        let (rows, _t, has_more) = space_videos(&mid, "pubdate", 1, cookies.as_ref()).expect("列表失败");
        assert!(!rows.is_empty(), "应有视频");
        assert!(has_more, "629 个视频第一页应有更多");
        assert!(!rows[0].title.is_empty());
        assert!(rows[0].duration_ms > 0, "时长解析: {:?}", rows[0]);
        println!("pubdate first: {} play={}", rows[0].title, rows[0].play);
        let (rows2, _t2, _h2) = space_videos(&mid, "click", 1, cookies.as_ref()).expect("播放排序失败");
        assert!(!rows2.is_empty());
        println!("click first: {} play={}", rows2[0].title, rows2[0].play);
        let seasons = space_seasons(&mid, cookies.as_ref()).expect("合集失败");
        println!("collections: {}", seasons.len());
    }

    /// EOS 实证：完整解码缓存里真实播放过的 m4s 到末尾，迭代器必须干净终止
    /// （EOS 坏了的表现恰是：歌放完了但 ended 事件不触发 → 不自动切下一首）。
    #[test]
    #[ignore]
    fn e2e_symdec_eos_terminates() {
        let dir = std::env::var("APPDATA").unwrap();
        let dir = std::path::Path::new(&dir).join("com.rustmusic.app/downloads");
        let mut found = None;
        for e in std::fs::read_dir(&dir).expect("downloads 目录").flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("m4s") {
                found = Some(p);
                break;
            }
        }
        let path = found.expect("缓存里没有 m4s");
        println!("file: {}", path.display());
        use rodio::Source as _;
        let mut src =
            crate::symdec::SymphoniaSource::open(path.to_str().unwrap()).expect("打开失败");
        let (sr, ch) = (src.sample_rate().get(), src.channels().get());
        let mut n: u64 = 0;
        for s in src.by_ref() {
            let _ = s;
            n += 1;
        }
        let secs = n as f64 / (sr as f64 * ch as f64);
        println!("eos ok: samples={n} ≈{secs:.1}s (sr={sr} ch={ch})");
        assert!(secs > 30.0, "解码时长异常偏短: {secs:.1}s");
    }

    /// 自动连播链路实证：空间列表首行（裸 BV rid）→ parse_rid（查 cid）
    /// → audio_stream（直链）。自动切歌时每首都走这条，任何一环失败即断链。
    #[test]
    #[ignore]
    fn e2e_bare_bv_autonext_chain() {
        // 读应用登录态（匿名会撞风控）
        let cookies = (|| -> Option<BiliCookies> {
            let db = std::env::var("APPDATA").ok()?;
            let conn = rusqlite::Connection::open_with_flags(
                std::path::Path::new(&db).join("com.rustmusic.app/library.db"),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .ok()?;
            let get = |k: &str| -> String {
                conn.query_row("SELECT value FROM settings WHERE key = ?1", [k], |r| r.get(0))
                    .unwrap_or_default()
            };
            let sessdata = get("bili_sessdata");
            if sessdata.is_empty() {
                return None;
            }
            Some(BiliCookies { sessdata, buvid3: get("bili_buvid3") })
        })();
        let mid = parse_space("https://space.bilibili.com/229733301").expect("解析 mid 失败");
        let (rows, _t, _h) = space_videos(&mid, "pubdate", 1, cookies.as_ref()).expect("列表失败");
        assert!(rows.len() >= 2, "至少两首才能验证连播");
        for r in rows.iter().take(2) {
            println!("--- rid={}", r.bvid);
            let (bvid, cid) = parse_rid(&r.bvid).expect("parse_rid 失败");
            println!("    cid={cid}");
            let (url, _q) = audio_stream(&bvid, cid).expect("audio_stream 失败");
            println!("    audio ok: {}...", &url[..40.min(url.len())]);
            // 第二首应命中 bvid→cid 缓存（同进程内无重复网络调用）
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    /// 端到端（需联网）：解析用户给的示例视频 → 拿音频直链 → 下载到临时文件
    /// → SymphoniaSource 解码，验证真实音频数据。
    #[test]
    #[ignore] // cargo test -- --ignored
    fn e2e_resolve_download_decode() {
        // 1. 多P/单P解析 + 分P展开
        let rows = resolve_pages("https://www.bilibili.com/video/BV1PFe26bEzt?t=185.7")
            .expect("resolve_pages 失败");
        assert!(!rows.is_empty(), "至少解析出一个分P");
        assert!(rows[0].0.starts_with("bili://"), "存储链接应为 bili:// 协议");

        // 2. 播放解析：直链 + 元数据
        let (audio_url, info) = resolve(&rows[0].0)
            .expect("resolve 失败")
            .expect("应为 B 站链接");
        assert!(audio_url.starts_with("https://"), "音频直链: {audio_url}");
        assert!(!info.title.is_empty(), "标题: {}", info.title);
        assert!(!info.artist.is_empty(), "UP主: {}", info.artist);
        assert!(info.duration_ms > 0, "时长: {}", info.duration_ms);
        assert!(info.qid.is_some(), "缓存键载体 qid: {:?}", info.qid);
        assert_eq!(info.kind, "bilibili");

        // 3. CDN 下载（引擎同款：带 Referer/UA，部分边缘节点裸请求 403）
        const CDN_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
        let mut resp = agent()
            .get(&audio_url)
            .set("Referer", "https://www.bilibili.com/")
            .set("User-Agent", CDN_UA)
            .call()
            .expect("CDN 下载失败");
        let mut buf = Vec::new();
        // 必须完整下载：fMP4 的 sidx 引用全文件偏移，截断文件探针会报 end of stream
        resp.into_reader()
            .read_to_end(&mut buf)
            .expect("读取音频流失败");
        assert!(buf.len() > 2 * 1024 * 1024, "音频流过短: {} 字节", buf.len());
        assert_eq!(&buf[4..8], b"ftyp", "应为 MP4 容器");

        // 4. 缓存为 m4s 并用引擎同款解码源验证
        let tmp = std::env::temp_dir().join("rustmusic_bili_test.m4s");
        std::fs::write(&tmp, &buf).unwrap();
        let path = tmp.to_string_lossy().into_owned();
        let mut src = crate::symdec::SymphoniaSource::open(&path).expect("SymphoniaSource 打开失败");
        let mut n = 0usize;
        let mut peak = 0f32;
        for s in src.by_ref() {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            n += 1;
            if n >= 3_000_000 {
                break;
            }
        }
        let _ = std::fs::remove_file(&tmp);
        assert!(n > 2_000_000, "解码样本过少: {n}");
        assert!(peak > 0.05, "峰值过低: {peak}");
        println!("e2e ok: samples={n} peak={peak:.3} title={} dur={}ms", info.title, info.duration_ms);
    }
}
