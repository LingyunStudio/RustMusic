//! 扫码登录：QQ 与微信两条链路。

use super::playlists::*;
use super::url::*;
use super::*;

// ---------- 扫码登录 ----------

/// 生成 QQ 登录二维码，返回 (qrsig, qr_png_base64)
pub fn qr_create() -> Result<(String, String), String> {
    let url = format!(
        "https://ssl.ptlogin2.qq.com/ptqrshow?appid=716027609&e=2&l=M&s=3&d=72&v=4&t={}&daid=383&pt_3rd_aid=100497308",
        chrono_now_ms()
    );
    let resp = http_agent()
        .get(&url)
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
    let end = text[start..]
        .find(')')
        .map(|i| start + i)
        .unwrap_or(text.len());
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
    let text = plain_get_with_cookie(
        &url,
        "https://xui.ptlogin2.qq.com/",
        &format!("qrsig={qrsig}"),
    )?;
    let args = extract_ptui_args(&text);
    let code_str = args.first().cloned().unwrap_or_default();
    let code: i64 = code_str.parse().unwrap_or(-1);
    if code != 0 {
        let status = match code {
            65 => "expired",
            67 => "scanned",
            _ => "waiting",
        };
        return Ok(QqQrCheck {
            status: status.into(),
            nickname: None,
            musicid: None,
            musickey: None,
            encrypt_uin: None,
        });
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
        let resp = no_redirect_agent()
            .get(&check_sig_url)
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
        let resp = no_redirect_agent()
            .post("https://graph.qq.com/oauth2.0/authorize")
            .set("Content-Type", "application/x-www-form-urlencoded")
            .set("Referer", "https://graph.qq.com/")
            .set("User-Agent", UA)
            .set("Cookie", &cookie_header)
            .timeout(TIMEOUT)
            .send_string(&form)
            .map_err(|e| format!("OAuth 授权失败: {e}"))?;
        let location = resp.header("Location").unwrap_or("").to_string();
        let oauth_code = parse_query_param(&location, "code")
            .ok_or_else(|| format!("OAuth 授权未返回 code（HTTP {}）", resp.status()))?;

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
    (0..len)
        .map(|_| format!("{:x}", rng.gen_range(0..16)))
        .collect()
}

// ---------- 微信扫码登录 ----------
//
// 与 y.qq.com 网页版同链路（参考官方 login.html / wx_redirect.html 的实现）：
// qrconnect 页取 uuid → 展示 open.weixin.qq.com 二维码 → lp 长轮询拿 wx_code
// → musicu.fcg 的 music.login.LoginServer/Login 用 code 换播放凭证
//（响应 Set-Cookie 携带 qm_keyst / wxuin 登录态，与 QQ 扫码共用存储字段）。

/// QQ 音乐在微信开放平台的网站应用 appid
const WX_APPID: &str = "wx48db31d50e334801";
/// lp 长轮询服务端会挂起到状态变化（实测约 15s），客户端超时给足余量
const WX_LP_TIMEOUT: Duration = Duration::from_secs(35);

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WxQr {
    pub uuid: String,
    /// 二维码 data URL（微信返回 JFIF/JPEG）
    pub qr: String,
}

/// 生成微信登录二维码：uuid + JPEG data URL
pub fn wx_qr_create() -> Result<WxQr, String> {
    let url = format!(
        "https://open.weixin.qq.com/connect/qrconnect?appid={WX_APPID}&scope=snsapi_login&redirect_uri={}&state=rustmusic",
        form_encode("https://y.qq.com/wkframe/client/login_success.html")
    );
    let html = http_agent()
        .get(&url)
        .set("User-Agent", UA)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("获取微信登录页失败: {e}"))?
        .into_string()
        .map_err(|e| format!("微信登录页读取失败: {e}"))?;
    // 页面二维码 <img src="/connect/qrcode/{uuid}">（兜底：长轮询 URL 的 uuid 参数）
    let uuid = html
        .find("/connect/qrcode/")
        .map(|i| {
            let rest = &html[i + "/connect/qrcode/".len()..];
            match rest.find('"') {
                Some(e) => rest[..e].to_string(),
                None => String::new(),
            }
        })
        .filter(|u| !u.is_empty())
        .or_else(|| parse_query_param(&html, "uuid"))
        .unwrap_or_default();
    if uuid.is_empty() || uuid.len() > 64 || !uuid.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("微信二维码标识解析失败".into());
    }

    let img_url = format!("https://open.weixin.qq.com/connect/qrcode/{uuid}");
    let resp = http_agent()
        .get(&img_url)
        .set("User-Agent", UA)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("获取微信二维码失败: {e}"))?;
    let mut jpeg = Vec::new();
    resp.into_reader()
        .read_to_end(&mut jpeg)
        .map_err(|e| format!("微信二维码读取失败: {e}"))?;
    if jpeg.is_empty() {
        return Err("微信二维码内容为空".into());
    }
    Ok(WxQr {
        uuid,
        qr: format!("data:image/jpeg;base64,{}", b64_encode(&jpeg)),
    })
}

/// 从长轮询响应提取 (errcode, wx_code)：`window.wx_errcode=405;window.wx_code='xxx';`
fn parse_wx_lp(text: &str) -> (i32, String) {
    let errcode = text
        .find("wx_errcode=")
        .and_then(|i| {
            let rest = &text[i + "wx_errcode=".len()..];
            let end = rest.find(';').unwrap_or(rest.len());
            rest[..end].trim().parse::<i32>().ok()
        })
        .unwrap_or(-1);
    let code = text
        .find("wx_code='")
        .and_then(|i| {
            let rest = &text[i + "wx_code='".len()..];
            rest.find('\'').map(|e| rest[..e].to_string())
        })
        .unwrap_or_default();
    (errcode, code)
}

/// 从 Set-Cookie 列表提取 name=value（值可能含 '='，按首个 '=' 切）
fn cookie_value(cookies: &[String], name: &str) -> Option<String> {
    cookies.iter().find_map(|c| {
        let kv = c.split(';').next()?.trim();
        if kv.starts_with(&format!("{name}=")) {
            Some(kv[name.len() + 1..].to_string())
        } else {
            None
        }
    })
}

/// 用 wx_code 换播放凭证：musicu.fcg / music.login.LoginServer.Login。
/// 凭证优先取响应 JSON（musicid/musickey），缺失时回退 Set-Cookie
///（qm_keyst=key，wxuin=id），两条路都空才算失败
fn wx_exchange(code: &str) -> Result<QqQrCheck, String> {
    let payload = serde_json::json!({
        "comm": {
            "tmeAppID": "qqmusic",
            "tmeLoginType": "1",
            "g_tk": 5381,
            "platform": "yqq",
            "ct": 24,
            "cv": 0
        },
        "req": {
            "module": "music.login.LoginServer",
            "method": "Login",
            "param": {"strAppid": WX_APPID, "code": code}
        }
    });
    let resp = http_agent()
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .set("Content-Type", "application/json")
        .set("Referer", "https://y.qq.com/")
        .set("Origin", "https://y.qq.com")
        .set("User-Agent", UA)
        .timeout(TIMEOUT)
        .send_string(&payload.to_string())
        .map_err(|e| format!("微信登录凭证交换失败: {e}"))?;
    let cookies: Vec<String> = resp
        .all("Set-Cookie")
        .into_iter()
        .map(|c| c.to_string())
        .collect();
    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("微信登录响应解析失败: {e}"))?;

    let credential = find_credential(&body);
    let (musicid, musickey) = match credential {
        Some(c) => c,
        None => {
            let key = cookie_value(&cookies, "qm_keyst");
            let id = cookie_value(&cookies, "wxuin").or_else(|| cookie_value(&cookies, "uin"));
            match (id, key) {
                (Some(id), Some(key)) if !id.is_empty() && !key.is_empty() => (id, key),
                _ => {
                    return Err(format!(
                        "微信登录响应缺少凭证 | {}",
                        serde_json::to_string(&body)
                            .unwrap_or_default()
                            .chars()
                            .take(200)
                            .collect::<String>()
                    ))
                }
            }
        }
    };
    let nickname = ["nick", "nickname", "nickName"]
        .iter()
        .find_map(|k| find_str_anywhere(&body, k));
    let encrypt_uin = find_str_anywhere(&body, "encryptUin");
    Ok(QqQrCheck {
        status: "success".into(),
        nickname,
        musicid: Some(musicid),
        musickey: Some(musickey),
        encrypt_uin,
    })
}

/// 长轮询一次微信扫码状态（调用方循环调用）：
/// 408=等待 / 404=已扫码待确认 / 405=已确认（带 wx_code，接交换）/ 402=已过期
pub fn wx_qr_check(uuid: &str) -> Result<QqQrCheck, String> {
    if uuid.is_empty() || uuid.len() > 64 || !uuid.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("微信登录标识无效".into());
    }
    let url = format!("https://lp.open.weixin.qq.com/connect/l/qrconnect?uuid={uuid}");
    let text = http_agent()
        .get(&url)
        .set("Referer", "https://open.weixin.qq.com/")
        .set("User-Agent", UA)
        .timeout(WX_LP_TIMEOUT)
        .call()
        .map_err(|e| format!("微信扫码状态查询失败: {e}"))?
        .into_string()
        .map_err(|e| format!("微信扫码状态读取失败: {e}"))?;
    let (errcode, code) = parse_wx_lp(&text);
    match errcode {
        408 => Ok(QqQrCheck {
            status: "waiting".into(),
            nickname: None,
            musicid: None,
            musickey: None,
            encrypt_uin: None,
        }),
        404 => Ok(QqQrCheck {
            status: "scanned".into(),
            nickname: None,
            musicid: None,
            musickey: None,
            encrypt_uin: None,
        }),
        402 => Ok(QqQrCheck {
            status: "expired".into(),
            nickname: None,
            musicid: None,
            musickey: None,
            encrypt_uin: None,
        }),
        405 if !code.is_empty() => wx_exchange(&code),
        _ => {
            if errcode == 405 {
                Err("微信授权未返回 code".into())
            } else {
                Ok(QqQrCheck {
                    status: "waiting".into(),
                    nickname: None,
                    musicid: None,
                    musickey: None,
                    encrypt_uin: None,
                })
            }
        }
    }
}
