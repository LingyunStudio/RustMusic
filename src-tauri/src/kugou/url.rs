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

/// 最近一次高阶音质取链失败的服务端原因（如 "no_url:20031"）；成功取到
/// 或未尝试时为 None。回退提示据此区分“没拿到文件标识”与“服务端拒绝了请求”
static HIRES_ERR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn set_last_hires_error(v: Option<String>) {
    *HIRES_ERR.lock().unwrap() = v;
}

pub fn last_hires_error() -> Option<String> {
    HIRES_ERR.lock().unwrap().clone()
}

/// 权益接口（/v2/get_res_privilege/lite）relate_goods 解析出的各档位文件 hash
#[derive(Clone, Debug, Default)]
pub(super) struct PrivilegeHashes {
    pub hq_320: String,
    pub sq_flac: String,
    pub high: String,
    pub viper_clear: String,
    pub viper_atmos: String,
    pub viper_tape: String,
}

impl PrivilegeHashes {
    fn has_advanced(&self) -> bool {
        !self.high.is_empty()
            || !self.viper_clear.is_empty()
            || !self.viper_atmos.is_empty()
            || !self.viper_tape.is_empty()
    }
}

/// 查询 /v2/get_res_privilege/lite 获取曲目关联的全档位文件 hash（含 SVIP 蝰蛇母带 / 全景声 / 超清与 Hi-Res），带进程内 TTL 缓存
pub(super) fn privilege_hashes(
    hash: &str,
    album_id: u64,
    album_audio_id: u64,
) -> PrivilegeHashes {
    if hash.is_empty() {
        return PrivilegeHashes::default();
    }
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<String, (std::time::Instant, PrivilegeHashes)>,
        >,
    > = std::sync::OnceLock::new();
    const TTL_FULL: std::time::Duration = std::time::Duration::from_secs(30 * 60);
    const TTL_PARTIAL: std::time::Duration = std::time::Duration::from_secs(5 * 60);
    const CAP: usize = 256;

    let key = hash.to_lowercase();
    let slot = CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Ok(guard) = slot.lock() {
        if let Some((at, hit)) = guard.get(&key) {
            let ttl = if hit.has_advanced() {
                TTL_FULL
            } else {
                TTL_PARTIAL
            };
            if at.elapsed() < ttl {
                return hit.clone();
            }
        }
    }

    let fetched = fetch_privilege_hashes_uncached(hash, album_id, album_audio_id);
    if let Ok(mut guard) = slot.lock() {
        while guard.len() >= CAP {
            if let Some(oldest) = guard
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(k, _)| k.clone())
            {
                guard.remove(&oldest);
            } else {
                break;
            }
        }
        guard.insert(key, (std::time::Instant::now(), fetched.clone()));
    }
    fetched
}

fn fetch_privilege_hashes_uncached(
    hash: &str,
    album_id: u64,
    album_audio_id: u64,
) -> PrivilegeHashes {
    let body = serde_json::json!({
        "appid": APPID.parse::<i64>().unwrap_or(1005),
        "area_code": 1,
        "behavior": "play",
        "clientver": CLIENTVER.parse::<i64>().unwrap_or(20489),
        "need_hash_offset": 1,
        "relate": 1,
        "support_verify": 1,
        "resource": [{
            "type": "audio",
            "page_id": 0,
            "hash": hash.to_uppercase(),
            "album_id": album_id,
            "album_audio_id": album_audio_id,
        }],
        "qualities": [
            "128",
            "320",
            "flac",
            "high",
            "viper_atmos",
            "viper_tape",
            "viper_clear",
            "super"
        ],
    });
    let Ok(v) = gateway(
        "/v2/get_res_privilege/lite",
        "media.store.kugou.com",
        &[],
        Some(&body),
    ) else {
        return PrivilegeHashes::default();
    };

    let mut out = PrivilegeHashes::default();
    let mut raw_tape_hash = String::new();
    let mut raw_tape_br: u64 = 0;

    if let Some(goods) = v.pointer("/data/0/relate_goods").and_then(|g| g.as_array()) {
        for g in goods {
            let q = g
                .get("quality")
                .or_else(|| g.get("level"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let h = g
                .get("hash")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if h.is_empty() {
                continue;
            }
            let br = g
                .pointer("/info/bitrate")
                .or_else(|| g.get("bitRate"))
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            match q {
                "320" if out.hq_320.is_empty() => out.hq_320 = h,
                "flac" if out.sq_flac.is_empty() => out.sq_flac = h,
                "high" if out.high.is_empty() => out.high = h,
                "super" if out.high.is_empty() => out.high = h,
                "viper_clear" if out.viper_clear.is_empty() => out.viper_clear = h,
                "viper_atmos" if out.viper_atmos.is_empty() => out.viper_atmos = h,
                "viper_tape" if raw_tape_hash.is_empty() => {
                    raw_tape_hash = h;
                    raw_tape_br = br;
                }
                _ => {}
            }
        }
    }

    // 部分曲目（如周杰伦《晴天》）relate_goods 中的 viper_tape 条目直接复用了 320k MP3 的 hash（br=320），
    // 而真正的 96kHz/24bit 母带级无损流放在 viper_clear（3611kbps）。剔除伪造的 320k viper_tape，
    // 让阶梯自然落到真正的 96kHz/24bit 蝰蛇超清或 Hi-Res。
    if !raw_tape_hash.is_empty()
        && !raw_tape_hash.eq_ignore_ascii_case(hash)
        && !raw_tape_hash.eq_ignore_ascii_case(&out.hq_320)
        && (raw_tape_br == 0 || raw_tape_br > 400)
    {
        out.viper_tape = raw_tape_hash;
    }

    out
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
    // 若请求高阶音质（SVIP 蝰蛇系列 / Hi-Res）或基础档位 hash 缺失，通过权益接口补齐各档真实 hash
    let need_priv = matches!(
        quality,
        "viper_tape" | "master" | "viper_atmos" | "viper_clear" | "hires"
    ) || (quality == "lossless" && sq_hash.is_empty())
        || (quality == "high" && hq_hash.is_empty());

    let priv_h = if need_priv {
        privilege_hashes(hash, album_id, album_audio_id)
    } else {
        PrivilegeHashes::default()
    };

    let hq = if !hq_hash.is_empty() {
        hq_hash
    } else {
        priv_h.hq_320.as_str()
    };
    let sq = if !sq_hash.is_empty() {
        sq_hash
    } else {
        priv_h.sq_flac.as_str()
    };
    let hi = if !priv_h.high.is_empty() {
        priv_h.high.as_str()
    } else {
        super_hash
    };
    let vc = priv_h.viper_clear.as_str();
    let va = priv_h.viper_atmos.as_str();
    let vt = priv_h.viper_tape.as_str();

    // 音质阶梯：越靠前越优先。设置档位 = 上限：选无损就绝不向上够 Hi-Res 或 SVIP 档位
    let ladder: Vec<(&str, &str, &'static str)> = match quality {
        "standard" => vec![("128", hash, "标准")],
        // 蝰蛇母带（SVIP）：优先尝试独立母带档 viper_tape，若该曲母带级 96kHz/24bit 流承载于 viper_clear 则紧随其后
        "viper_tape" | "master" => {
            let mut l = Vec::new();
            if !vt.is_empty() {
                l.push(("viper_tape", vt, "蝰蛇母带"));
            }
            if !vc.is_empty() {
                l.push(("viper_clear", vc, "蝰蛇超清"));
            }
            if !hi.is_empty() {
                l.push(("high", hi, "Hi-Res"));
                l.push(("super", hi, "Hi-Res"));
            }
            if !sq.is_empty() {
                l.push(("flac", sq, "无损"));
            }
            if !hq.is_empty() {
                l.push(("320", hq, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
        // 蝰蛇全景声（SVIP）：多声道空间音频 FLAC，回落蝰蛇超清 / Hi-Res / 无损
        "viper_atmos" => {
            let mut l = Vec::new();
            if !va.is_empty() {
                l.push(("viper_atmos", va, "蝰蛇全景声"));
            }
            if !vc.is_empty() {
                l.push(("viper_clear", vc, "蝰蛇超清"));
            }
            if !hi.is_empty() {
                l.push(("high", hi, "Hi-Res"));
                l.push(("super", hi, "Hi-Res"));
            }
            if !sq.is_empty() {
                l.push(("flac", sq, "无损"));
            }
            if !hq.is_empty() {
                l.push(("320", hq, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
        // 蝰蛇超清（SVIP）：96kHz/24bit 超清无损 FLAC，回落 Hi-Res / 无损
        "viper_clear" => {
            let mut l = Vec::new();
            if !vc.is_empty() {
                l.push(("viper_clear", vc, "蝰蛇超清"));
            }
            if !hi.is_empty() {
                l.push(("high", hi, "Hi-Res"));
                l.push(("super", hi, "Hi-Res"));
            }
            if !sq.is_empty() {
                l.push(("flac", sq, "无损"));
            }
            if !hq.is_empty() {
                l.push(("320", hq, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
        // Hi-Res 档（VIP/SVIP）：v5/url 的 24bit FLAC 参数为 quality=high + high_hash
        "hires" => {
            let mut l = Vec::new();
            if !hi.is_empty() {
                l.push(("high", hi, "Hi-Res"));
                l.push(("super", hi, "Hi-Res"));
            }
            if !sq.is_empty() {
                l.push(("flac", sq, "无损"));
            }
            if !hq.is_empty() {
                l.push(("320", hq, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
        // 无损档（VIP）：16bit FLAC 封顶，只向下回退
        "lossless" => {
            let mut l = Vec::new();
            if !sq.is_empty() {
                l.push(("flac", sq, "无损"));
            }
            if !hq.is_empty() {
                l.push(("320", hq, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
        _ => {
            let mut l = Vec::new();
            if !hq.is_empty() {
                l.push(("320", hq, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
    };
    set_last_hires_error(None);
    let mid = mid_of();
    let fetch = |q: &str, h: &str, tier: &'static str| -> Result<(String, String, &'static str), String> {
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
        // 档位标签直接用阶梯元组
        if !url.is_empty() {
            let ext = url_ext(&url);
            Ok((url, ext, tier))
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
        match fetch(q, h, tier) {
            Ok(ok) => {
                if matches!(*tier, "Hi-Res" | "蝰蛇超清" | "蝰蛇全景声" | "蝰蛇母带") {
                    set_last_hires_error(None);
                }
                return Ok(ok);
            }
            Err(e) => {
                // 高阶档被拒：记下服务端错误码（回退提示用），沿阶梯继续回落
                if matches!(*tier, "Hi-Res" | "蝰蛇超清" | "蝰蛇全景声" | "蝰蛇母带") {
                    let brief = e.split(" resp=").next().unwrap_or(&e).to_string();
                    eprintln!("[kugou] {tier} 取链失败（quality={q}）：{brief}");
                    set_last_hires_error(Some(brief));
                }
                // errcode 20028 = SSA 风控：重新注册设备后重试一次本档
                if e.contains("no_url:20028") {
                    if let Some(_nd) = re_register() {
                        if let Ok(ok) = fetch(q, h, tier) {
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
            // 匿名通道 playInfo 失败时（如频繁请求被限流 1002 或下架），尝试官方 v5/url 匿名通道兜底
            if let Ok((url, ext, label)) = song_url_v5(
                "",
                "0",
                album_audio_id,
                album_id,
                hash,
                hq_hash,
                sq_hash,
                super_hash,
                "standard",
            ) {
                return Ok((url, ext, label.to_string()));
            }
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
