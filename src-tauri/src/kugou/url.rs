//! 播放直链取链（免费接口 + v5 网关，Hi-Res 探测）。

use super::*;

// ---------- 播放直链 ----------

/// 匿名播放信息（免费曲目 128k 直链），返回 (url, ext)
fn song_url_free(hash: &str, vip: bool) -> Result<(String, String), String> {
    let url = format!(
        "https://m.kugou.com/app/i/getSongInfo.php?cmd=playInfo&hash={}",
        form_encode(hash)
    );
    let v = get_json(&url, "https://m.kugou.com/", UA, "酷狗歌曲信息")?;
    let play_url = v
        .get("url")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();
    if play_url.is_empty() {
        let err = v.get("error").and_then(|e| e.as_str()).unwrap_or("");
        let privilege = v.get("privilege").and_then(|p| p.as_i64()).unwrap_or(0);
        return Err(if err.contains("付费") || privilege == 10 || vip {
            "该曲目需要酷狗 VIP 或付费购买（扫码登录后按会员权益播放）".into()
        } else {
            "该歌曲在酷狗暂无可播放链接（可能已下架）".into()
        });
    }
    let ext = url_ext(&play_url);
    Ok((play_url, ext))
}

/// 从直链路径推断扩展名
pub(super) fn url_ext(url: &str) -> String {
    let path = url.split('?').next().unwrap_or("");
    path.rsplit('.')
        .next()
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("mp3")
        .to_lowercase()
}

/// 最近一次 Hi-Res 取链失败的服务端原因（如 "no_url:20031"）；成功取到
/// Hi-Res 或未尝试时为 None。回退提示据此区分“没拿到 Hi-Res 文件标识”
/// 与“服务端拒绝了 Hi-Res 请求”
static HIRES_ERR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn set_last_hires_error(v: Option<String>) {
    *HIRES_ERR.lock().unwrap() = v;
}

pub fn last_hires_error() -> Option<String> {
    HIRES_ERR.lock().unwrap().clone()
}

/// 登录态按音质取直链：tracker /v5/url，从所选音质逐级回退到 128。
/// 返回 (url, ext, 音质标签)
fn song_url_v5(
    token: &str,
    userid: &str,
    album_audio_id: u64,
    album_id: u64,
    hash: &str,
    hq_hash: &str,
    sq_hash: &str,
    super_hash: &str,
    quality: &str,
) -> Result<(String, String, &'static str), String> {
    // 音质阶梯：越靠前越优先。hash 必须用对应档位文件的 hash（搜索/榜单
    // 返回的 320/sq/super hash），key 也按该 hash 计算，否则触发风控/无链接
    // 设置档位 = 上限：选无损就绝不向上够 Hi-Res。此前无损档与 Hi-Res 档
    // 共用同一条 super 优先链——hash 补齐修好后，选“无损”也会先命中
    // Hi-Res，两档失去区别（切音质永远显示 Hi-Res 的根因）
    let ladder: Vec<(&str, &str, &str)> = match quality {
        "standard" => vec![("128", hash, "标准")],
        // Hi-Res 档专属 super 优先链。v5/url 的 Hi-Res 档位参数是 "high"
        // （酷狗客户端与主流开源实现的既定约定：flac24bit → quality=high
        // + superFileHash）；"super" 是权益接口的标识，作为兜底再试一次
        "hires" => {
            let mut l = Vec::new();
            if !super_hash.is_empty() {
                l.push(("high", super_hash, "Hi-Res"));
                l.push(("super", super_hash, "Hi-Res"));
            }
            if !sq_hash.is_empty() {
                l.push(("flac", sq_hash, "无损"));
            }
            if !hq_hash.is_empty() {
                l.push(("320", hq_hash, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
        // 无损档：16bit FLAC 封顶，只向下回退
        "lossless" => {
            let mut l = Vec::new();
            if !sq_hash.is_empty() {
                l.push(("flac", sq_hash, "无损"));
            }
            if !hq_hash.is_empty() {
                l.push(("320", hq_hash, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
        _ => {
            let mut l = Vec::new();
            if !hq_hash.is_empty() {
                l.push(("320", hq_hash, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
    };
    set_last_hires_error(None);
    let mid = mid_of();
    let fetch = |q: &str, h: &str| -> Result<(String, String, &'static str), String> {
        let extra = vec![
            ("album_id".into(), album_id.to_string()),
            ("area_code".into(), "1".into()),
            ("hash".into(), h.to_string()),
            ("ssa_flag".into(), "is_fromtrack".into()),
            ("version".into(), "11430".into()),
            ("page_id".into(), "151369488".into()),
            ("quality".into(), q.to_string()),
            ("album_audio_id".into(), album_audio_id.to_string()),
            ("behavior".into(), "play".into()),
            ("pid".into(), "2".into()),
            ("cmd".into(), "26".into()),
            ("pidversion".into(), "3001".into()),
            ("IsFreePart".into(), "0".into()),
            ("ppage_id".into(), "463467626,350369493,788954147".into()),
            ("cdnBackup".into(), "1".into()),
            ("module".into(), String::new()),
            ("clientver".into(), "11430".into()),
            (
                "key".into(),
                md5_hex(&format!("{h}{SALT_URL_KEY}{APPID}{mid}{userid}")),
            ),
            ("token".into(), token.to_string()),
            ("userid".into(), userid.to_string()),
        ];
        let v = gateway("/v5/url", "trackercdn.kugou.com", &extra, None)?;
        // 响应为平铺结构：url（字符串或数组）/ backupUrl；兼容 data[0] 包裹
        let pick = |x: &serde_json::Value| -> String {
            match x {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Array(a) => a
                    .iter()
                    .filter_map(|s| s.as_str())
                    .find(|s| !s.is_empty())
                    .unwrap_or("")
                    .to_string(),
                _ => String::new(),
            }
        };
        let mut url = v.get("url").map(&pick).unwrap_or_default();
        if url.is_empty() {
            url = v
                .get("backupUrl")
                .and_then(|b| b.get(0))
                .map(&pick)
                .unwrap_or_default();
        }
        if url.is_empty() {
            url = v.pointer("/data/0/url").map(&pick).unwrap_or_default();
            if url.is_empty() {
                url = v
                    .pointer("/data/0/fallback/0/url")
                    .map(&pick)
                    .unwrap_or_default();
            }
        }
        let label = match q {
            "high" | "super" => "Hi-Res",
            "flac" => "无损",
            "320" => "HQ",
            _ => "标准",
        };
        if !url.is_empty() {
            let ext = url_ext(&url);
            Ok((url, ext, label))
        } else {
            let ec = v
                .get("errcode")
                .or_else(|| v.get("error_code"))
                .and_then(|c| c.as_i64())
                .unwrap_or(0);
            Err(format!(
                "no_url:{ec} resp={}",
                serde_json::to_string(&v)
                    .unwrap_or_default()
                    .chars()
                    .take(300)
                    .collect::<String>()
            ))
        }
    };

    let mut last_err = String::from("无可用链接");
    for (q, h, tier) in ladder.iter() {
        if h.is_empty() {
            continue;
        }
        match fetch(q, h) {
            Ok(ok) => {
                if *tier == "Hi-Res" {
                    set_last_hires_error(None);
                }
                return Ok(ok);
            }
            Err(e) => {
                // Hi-Res 档被拒：记下服务端错误码（回退提示用），沿阶梯继续回落
                if *tier == "Hi-Res" {
                    let brief = e.split(" resp=").next().unwrap_or(&e).to_string();
                    eprintln!("[kugou] Hi-Res 取链失败（quality={q}）：{brief}");
                    set_last_hires_error(Some(brief));
                }
                // errcode 20028 = SSA 风控：重新注册设备后重试一次本档
                if e.contains("no_url:20028") {
                    if let Some(_nd) = re_register() {
                        if let Ok(ok) = fetch(q, h) {
                            return Ok(ok);
                        }
                    }
                    last_err = "本次请求需要验证（设备风控）".into();
                } else {
                    last_err = e;
                }
            }
        }
    }
    Err(last_err)
}

/// 取播放直链：登录用户走 /v5/url 按音质取（失败自动回退匿名通道），
/// 匿名走 playInfo（仅免费曲目 128k）。返回 (url, 扩展名, 音质标签)
#[allow(clippy::too_many_arguments)]
pub fn song_url(
    hash: &str,
    album_audio_id: u64,
    album_id: u64,
    hq_hash: &str,
    sq_hash: &str,
    super_hash: &str,
    vip: bool,
    token: &str,
    userid: &str,
    quality: &str,
) -> Result<(String, String, String), String> {
    let mut v5_err = String::new();
    if !token.is_empty() && !userid.is_empty() {
        match song_url_v5(
            token,
            userid,
            album_audio_id,
            album_id,
            hash,
            hq_hash,
            sq_hash,
            super_hash,
            quality,
        ) {
            Ok((url, ext, label)) => return Ok((url, ext, label.to_string())),
            // 登录通道失败（风控/会话失效/权益不足）：继续走匿名通道兜底，
            // 免费曲目仍可 128k 播放；VIP 曲目会在此得到明确错误
            Err(e) => v5_err = e,
        }
    }
    match song_url_free(hash, vip) {
        Ok((url, ext)) => Ok((url, ext, "标准".into())),
        Err(e) => {
            // 双通道都失败：把登录通道的失败原因一并透出——用户才能区分
            // “没会员”与“登录态失效”（匿名通道的错误分不出二者）
            if !v5_err.is_empty() && vip {
                let brief: String = v5_err.chars().take(120).collect();
                return Err(format!(
                    "{e}；登录通道：{brief}（登录态可能已过期或权益不足，可尝试重新扫码登录）"
                ));
            }
            Err(e)
        }
    }
}
