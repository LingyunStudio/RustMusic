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
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("获取登录二维码失败", "Failed to fetch login QR code")))?;
    let qrsig = resp
        .all("Set-Cookie")
        .into_iter()
        .find_map(|c| {
            c.split(';')
                .next()
                .and_then(|seg| seg.trim().strip_prefix("qrsig="))
                .map(|s| s.to_string())
        })
        .ok_or_else(|| crate::i18n::pick("未获取到登录标识（qrsig）", "Login token (qrsig) not returned").to_string())?;
    let mut png = Vec::new();
    resp.into_reader()
        .read_to_end(&mut png)
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("二维码读取失败", "Failed to read QR code")))?;
    if png.is_empty() {
        return Err(crate::i18n::pick("二维码内容为空", "QR code content is empty").into());
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

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct QqSession {
    pub musicid: String,
    pub musickey: String,
    /// 1 = 微信扫码（W_X_），2 = QQ 扫码（Q_H_L_）
    pub login_type: i64,
    pub openid: String,
    pub unionid: String,
    pub access_token: String,
    pub refresh_token: String,
    pub refresh_key: String,
    pub expired_at: i64,
    /// musickey 签发时间戳（Unix 秒）
    pub musickey_create_time: i64,
    /// musickey 有效时长（秒，服务端标准值为 259200 = 72 小时）
    pub key_expires_in: i64,
    pub encrypt_uin: String,
    pub nickname: String,
}

impl QqSession {
    /// QQ 音乐 musickey 默认有效期：72 小时（259200 秒）
    pub const DEFAULT_KEY_EXPIRES_IN: i64 = 259_200;
    /// 过期判定提前量（5 分钟），避免卡在临界秒数发出必败请求
    pub const EXPIRY_SAFETY_MARGIN_SECS: i64 = 300;
    /// 主动续期窗口：剩余寿命 ≤ 36 小时（即 72h 票据过半后）即后台静默换新
    pub const PROACTIVE_REFRESH_REMAINING_SECS: i64 = 36 * 3600;

    pub fn infer_login_type(explicit: i64, musickey: &str) -> i64 {
        if explicit == 1 || explicit == 2 {
            explicit
        } else if musickey.starts_with("W_X") {
            1
        } else {
            2
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.musicid.is_empty() && !self.musickey.is_empty()
    }

    /// 是否持有可用于续期的长效令牌（refresh_key 或 refresh_token）
    pub fn can_refresh(&self) -> bool {
        !self.musicid.is_empty()
            && (!self.refresh_key.is_empty() || !self.refresh_token.is_empty())
    }

    /// 按本地记录的签发时间判断 musickey 是否已过期。
    /// 旧版遗留会话（musickey_create_time == 0）无时间戳，返回 false 交由实际接口判定。
    pub fn is_expired_at(&self, now: i64) -> bool {
        if self.musickey_create_time <= 0 {
            return false;
        }
        let ttl = if self.key_expires_in > 0 {
            self.key_expires_in
        } else {
            Self::DEFAULT_KEY_EXPIRES_IN
        };
        let effective_ttl = (ttl - Self::EXPIRY_SAFETY_MARGIN_SECS).max(60);
        now >= self.musickey_create_time + effective_ttl
    }

    /// 是否进入主动续期窗口（剩余有效期 ≤ 36h 或 ≤ 总寿命一半；
    /// 若旧会话无签发时间但已有 refresh_token，也主动刷新一次以补齐时间戳）
    pub fn needs_proactive_refresh(&self, now: i64) -> bool {
        if !self.can_refresh() {
            return false;
        }
        if self.musickey_create_time <= 0 {
            return true;
        }
        let ttl = if self.key_expires_in > 0 {
            self.key_expires_in
        } else {
            Self::DEFAULT_KEY_EXPIRES_IN
        };
        let remaining = (self.musickey_create_time + ttl) - now;
        let threshold = Self::PROACTIVE_REFRESH_REMAINING_SECS.min(ttl / 2).max(1800);
        remaining <= threshold
    }

    /// 合并刷新响应：服务端未重发的长效字段（openid/unionid/refresh_token/refresh_key/encrypt_uin 等）
    /// 保留原值，绝不用空串覆盖有效凭据
    pub fn merge_refresh(&self, fresh: QqSession, now: i64) -> QqSession {
        let pick_str = |new_val: String, old_val: &str| -> String {
            if new_val.is_empty() {
                old_val.to_string()
            } else {
                new_val
            }
        };
        let musickey = pick_str(fresh.musickey, &self.musickey);
        let login_type = if fresh.login_type == 1 || fresh.login_type == 2 {
            fresh.login_type
        } else {
            Self::infer_login_type(self.login_type, &musickey)
        };
        QqSession {
            musicid: pick_str(fresh.musicid, &self.musicid),
            musickey,
            login_type,
            openid: pick_str(fresh.openid, &self.openid),
            unionid: pick_str(fresh.unionid, &self.unionid),
            access_token: pick_str(fresh.access_token, &self.access_token),
            refresh_token: pick_str(fresh.refresh_token, &self.refresh_token),
            refresh_key: pick_str(fresh.refresh_key, &self.refresh_key),
            expired_at: if fresh.expired_at > 0 {
                fresh.expired_at
            } else {
                self.expired_at
            },
            musickey_create_time: if fresh.musickey_create_time > 0 {
                fresh.musickey_create_time
            } else {
                now
            },
            key_expires_in: if fresh.key_expires_in > 0 {
                fresh.key_expires_in
            } else if self.key_expires_in > 0 {
                self.key_expires_in
            } else {
                Self::DEFAULT_KEY_EXPIRES_IN
            },
            encrypt_uin: pick_str(fresh.encrypt_uin, &self.encrypt_uin),
            nickname: pick_str(fresh.nickname, &self.nickname),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshError {
    /// 本地缺少 refresh_key / refresh_token（如升级前的旧登录态），无法静默续期
    MissingRefreshToken,
    /// 服务端明确拒绝（刷新凭证本身已过期、解绑或账号受限），需重新扫码
    AuthExpired(i64, String),
    /// 网络抖动或临时网关异常（应保留本地凭据，待下次重试）
    Transient(String),
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
    pub refresh_token: Option<String>,
    pub session: Option<QqSession>,
}

fn json_str_field(obj: &serde_json::Value, key: &str) -> Option<String> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn json_i64_field(obj: &serde_json::Value, key: &str) -> Option<i64> {
    obj.get(key).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_u64().and_then(|n| i64::try_from(n).ok()))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
    })
}

/// 从 QQLogin / LoginServer 响应 JSON 与 Set-Cookie 解析完整会话凭证
pub(super) fn parse_session_from_login_json(
    body: &serde_json::Value,
    cookies: &[String],
    default_login_type: i64,
) -> Option<QqSession> {
    let data = body
        .pointer("/req_1/data")
        .or_else(|| body.pointer("/req/data"))
        .or_else(|| body.pointer("/req_0/data"))
        .or_else(|| body.get("data"))
        .unwrap_or(body);

    let (cred_id, cred_key) = find_credential(body)
        .map(|(i, k)| (Some(i), Some(k)))
        .unwrap_or((None, None));

    let musicid = json_str_field(data, "str_musicid")
        .or_else(|| {
            json_i64_field(data, "musicid")
                .filter(|&n| n > 0)
                .map(|n| n.to_string())
        })
        .or(cred_id)
        .or_else(|| cookie_value(cookies, "wxuin"))
        .or_else(|| cookie_value(cookies, "uin"))
        .or_else(|| cookie_value(cookies, "qqmusic_uin"))
        .unwrap_or_default();

    let musickey = json_str_field(data, "musickey")
        .or(cred_key)
        .or_else(|| cookie_value(cookies, "qm_keyst"))
        .or_else(|| cookie_value(cookies, "qqmusic_key"))
        .unwrap_or_default();

    if musicid.is_empty() || musickey.is_empty() {
        return None;
    }

    let raw_lt = json_i64_field(data, "loginType")
        .or_else(|| json_i64_field(data, "login_type"))
        .unwrap_or(0);
    let login_type = QqSession::infer_login_type(
        if raw_lt == 1 || raw_lt == 2 {
            raw_lt
        } else {
            default_login_type
        },
        &musickey,
    );

    let openid = json_str_field(data, "openid")
        .or_else(|| find_str_anywhere(body, "openid"))
        .or_else(|| cookie_value(cookies, "wxopenid"))
        .unwrap_or_default();
    let unionid = json_str_field(data, "unionid")
        .or_else(|| find_str_anywhere(body, "unionid"))
        .unwrap_or_default();
    let access_token = json_str_field(data, "access_token")
        .or_else(|| find_str_anywhere(body, "access_token"))
        .unwrap_or_default();
    let refresh_token = json_str_field(data, "refresh_token")
        .or_else(|| json_str_anywhere_nonempty(body, &["refresh_token", "wxrefresh_token"]))
        .or_else(|| cookie_value(cookies, "wxrefresh_token"))
        .unwrap_or_default();
    let refresh_key = json_str_field(data, "refresh_key")
        .or_else(|| find_str_anywhere(body, "refresh_key"))
        .or_else(|| cookie_value(cookies, "qm_refresh_key"))
        .unwrap_or_default();
    let expired_at = json_i64_field(data, "expired_at").unwrap_or(0);
    let now = now_secs();
    let musickey_create_time = json_i64_field(data, "musickeyCreateTime")
        .filter(|&t| t > 0)
        .unwrap_or(now);
    let key_expires_in = json_i64_field(data, "keyExpiresIn")
        .filter(|&t| t > 0)
        .unwrap_or(QqSession::DEFAULT_KEY_EXPIRES_IN);
    let encrypt_uin = json_str_field(data, "encryptUin")
        .or_else(|| find_str_anywhere(body, "encryptUin"))
        .unwrap_or_default();
    let nickname = ["nick", "nickname", "nickName"]
        .iter()
        .find_map(|k| json_str_field(data, k).or_else(|| find_str_anywhere(body, k)))
        .unwrap_or_default();

    Some(QqSession {
        musicid,
        musickey,
        login_type,
        openid,
        unionid,
        access_token,
        refresh_token,
        refresh_key,
        expired_at,
        musickey_create_time,
        key_expires_in,
        encrypt_uin,
        nickname,
    })
}

fn json_str_anywhere_nonempty(body: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| find_str_anywhere(body, k))
}

/// 发送登录/续期 CGI 请求并同时捕获响应 JSON 与 Set-Cookie：
/// 优先走签名通道（musics.fcg），网关/解析失败再回退明文通道（musicu.fcg）
fn post_login_cgi(
    payload: &serde_json::Value,
    cookie: Option<&str>,
) -> Result<(serde_json::Value, Vec<String>), String> {
    let body = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    let call_endpoint = |url: &str| -> Result<(serde_json::Value, Vec<String>), String> {
        let resp = send_with_recover(|| {
            let mut req = http_agent()
                .post(url)
                .set("Content-Type", "application/json")
                .set("Referer", "https://y.qq.com/")
                .set("Origin", "https://y.qq.com")
                .set("User-Agent", UA)
                .timeout(TIMEOUT);
            if let Some(c) = cookie {
                req = req.set("Cookie", c);
            }
            req.send_string(&body)
        })?;
        let cookies: Vec<String> = resp
            .all("Set-Cookie")
            .into_iter()
            .map(|c| c.to_string())
            .collect();
        let text = resp.into_string().map_err(|e| e.to_string())?;
        let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
            let head: String = text.chars().take(150).collect();
            format!("{e} | body: {head}")
        })?;
        Ok((json, cookies))
    };

    if let Ok(sign) = zzc_sign(&body) {
        let signed_url = format!(
            "https://u.y.qq.com/cgi-bin/musics.fcg?_={}&sign={}",
            chrono_now_ms(),
            sign
        );
        if let Ok((json, cookies)) = call_endpoint(&signed_url) {
            if json.get("code").and_then(|c| c.as_i64()).unwrap_or(0) == 0 {
                return Ok((json, cookies));
            }
        }
    }
    call_endpoint("https://u.y.qq.com/cgi-bin/musicu.fcg")
}

/// 构造双渠道统一续期载荷（music.login.LoginServer / Login, loginMode=2）
pub(super) fn build_refresh_payload(prev: &QqSession) -> serde_json::Value {
    let login_type = QqSession::infer_login_type(prev.login_type, &prev.musickey);
    let param = if login_type == 1 {
        serde_json::json!({
            "openid": prev.openid,
            "refresh_token": prev.refresh_token,
            "str_musicid": prev.musicid,
            "musickey": prev.musickey,
            "unionid": prev.unionid,
            "refresh_key": prev.refresh_key,
            "loginMode": 2,
        })
    } else {
        let musicid_num = prev.musicid.parse::<i64>().unwrap_or(0);
        serde_json::json!({
            "openid": prev.openid,
            "access_token": prev.access_token,
            "refresh_token": prev.refresh_token,
            "expired_in": prev.expired_at,
            "musicid": musicid_num,
            "musickey": prev.musickey,
            "refresh_key": prev.refresh_key,
            "loginMode": 2,
        })
    };
    serde_json::json!({
        "comm": {
            "ct": "11",
            "cv": "13020508",
            "v": "13020508",
            "tmeAppID": "qqmusic",
            "tmeLoginType": login_type.to_string(),
            "qq": prev.musicid,
            "uin": prev.musicid,
            "authst": prev.musickey,
            "g_tk": hash33(&prev.musickey, 5381),
        },
        "req_1": {
            "module": "music.login.LoginServer",
            "method": "Login",
            "param": param,
        }
    })
}

pub(super) fn is_auth_revoked_code(code: i64) -> bool {
    matches!(
        code,
        1000 | 104009 | 104400 | 104401 | 20272 | 20274 | 20277 | 20278 | 20450
    )
}

/// 用 refresh_key / refresh_token 向 QQ 音乐 LoginServer 静默换取新 musickey
pub fn refresh_session(prev: &QqSession) -> Result<QqSession, RefreshError> {
    if !prev.can_refresh() {
        return Err(RefreshError::MissingRefreshToken);
    }
    let login_type = QqSession::infer_login_type(prev.login_type, &prev.musickey);
    let payload = build_refresh_payload(prev);
    let cookie = credential_cookie(&prev.musicid, &prev.musickey);
    let (body, cookies) =
        post_login_cgi(&payload, Some(&cookie)).map_err(RefreshError::Transient)?;

    let outer_code = body.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if outer_code != 0 {
        return if is_auth_revoked_code(outer_code) {
            Err(RefreshError::AuthExpired(
                outer_code,
                format!("outer code={outer_code}"),
            ))
        } else {
            Err(RefreshError::Transient(format!("outer code={outer_code}")))
        };
    }

    let sub_code = body
        .pointer("/req_1/code")
        .and_then(|c| c.as_i64())
        .unwrap_or(-1);
    if sub_code != 0 {
        let msg = body
            .pointer("/req_1/data/errMsg")
            .or_else(|| body.pointer("/req_1/data/errtip"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        return if is_auth_revoked_code(sub_code) {
            Err(RefreshError::AuthExpired(sub_code, msg))
        } else {
            Err(RefreshError::Transient(format!(
                "LoginServer code={sub_code} {msg}"
            )))
        };
    }

    let fresh = parse_session_from_login_json(&body, &cookies, login_type).ok_or_else(|| {
        RefreshError::Transient("refresh response missing musickey/musicid".into())
    })?;
    Ok(prev.merge_refresh(fresh, now_secs()))
}

/// 向 QQ 音乐个人主页轻量接口核实当前 musickey 是否在服务端已失效。
/// 网络异常返回 None（绝不把断网误判为登录过期）。
pub fn check_session_expired(musicid: &str, musickey: &str) -> Option<bool> {
    if musicid.is_empty() || musickey.is_empty() {
        return Some(true);
    }
    let g_tk = hash33(musickey, 5381);
    let url = format!(
        "https://c6.y.qq.com/rsc/fcgi-bin/fcg_get_profile_homepage.fcg?g_tk={g_tk}&format=json&inCharset=utf-8&outCharset=utf-8&notice=0&cid=205360838&needNewCode=0&loginUin={}&hostUin=0&userid={}&reqfrom=1",
        form_encode(musicid),
        form_encode(musicid),
    );
    let text =
        plain_get_with_cookie(&url, "https://y.qq.com/", &credential_cookie(musicid, musickey))
            .ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let code = v.get("code").and_then(|c| c.as_i64())?;
    if code == 0 {
        return Some(false);
    }
    // 仅对明确代表登录失效/未登录的错误码判定过期；其它临时服务端错误返回 None，避免误清有效凭据
    if matches!(code, 1000 | 1001 | 104009 | 40001) {
        Some(true)
    } else {
        None
    }
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
            refresh_token: None,
            session: None,
        });
    }

    {
        if args.len() < 3 {
            return Err(crate::i18n::pick("登录响应缺少必要参数", "Login response missing required parameters").into());
        }
        let check_url = &args[2];
        let uin = parse_query_param(check_url, "uin")
            .ok_or(crate::i18n::pick("登录响应缺少 uin", "Login response missing uin"))?;
        let sigx = parse_query_param(check_url, "ptsigx")
            .ok_or(crate::i18n::pick("登录响应缺少 ptsigx", "Login response missing ptsigx"))?;

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
            .map_err(|e| format!("{}: {e}", crate::i18n::pick("check_sig 请求失败", "check_sig request failed")))?;
        let check_cookies: Vec<String> = resp
            .all("Set-Cookie")
            .into_iter()
            .map(|c| c.split(';').next().unwrap_or("").trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
        let p_skey = check_cookies
            .iter()
            .find_map(|c| c.strip_prefix("p_skey="))
            .ok_or(crate::i18n::pick("未获取到 p_skey", "Failed to obtain p_skey"))?
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
            .map_err(|e| format!("{}: {e}", crate::i18n::pick("OAuth 授权失败", "OAuth authorization failed")))?;
        let location = resp.header("Location").unwrap_or("").to_string();
        let oauth_code = parse_query_param(&location, "code").ok_or_else(|| {
            let status = resp.status();
            if crate::i18n::is_en() {
                format!("OAuth did not return code (HTTP {status})")
            } else {
                format!("OAuth 授权未返回 code（HTTP {status}）")
            }
        })?;

        // Step 3: QQLogin —— 用 code 换取完整会话凭证（含 refresh_key / refresh_token）
        let payload = serde_json::json!({
            "comm": {"tmeLoginType": 2},
            "req_1": {
                "module": "QQConnectLogin.LoginServer",
                "method": "QQLogin",
                "param": {"code": oauth_code}
            }
        });
        let (login_body, login_cookies) = post_login_cgi(&payload, None).map_err(|e| {
            format!(
                "{}: {e}",
                crate::i18n::pick("QQ 登录凭证交换失败", "Failed to exchange QQ login credentials")
            )
        })?;
        let mut sess = parse_session_from_login_json(&login_body, &login_cookies, 2).ok_or_else(|| {
            let raw_log = serde_json::to_string(&login_body).unwrap_or_default();
            format!(
                "{} | {}",
                crate::i18n::pick("登录响应缺少凭证", "Login response missing credentials"),
                raw_log.chars().take(200).collect::<String>()
            )
        })?;
        let ptui_nick = args.get(5).cloned().filter(|s| !s.is_empty());
        if sess.nickname.is_empty() {
            if let Some(n) = &ptui_nick {
                sess.nickname = n.clone();
            }
        }
        let nickname = if !sess.nickname.is_empty() {
            Some(sess.nickname.clone())
        } else {
            ptui_nick
        };
        let musicid = Some(sess.musicid.clone());
        let musickey = Some(sess.musickey.clone());
        let encrypt_uin = (!sess.encrypt_uin.is_empty()).then(|| sess.encrypt_uin.clone());
        let refresh_token = (!sess.refresh_token.is_empty()).then(|| sess.refresh_token.clone());
        Ok(QqQrCheck {
            status: "success".into(),
            nickname,
            musicid,
            musickey,
            encrypt_uin,
            refresh_token,
            session: Some(sess),
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
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("获取微信登录页失败", "Failed to fetch WeChat login page")))?
        .into_string()
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("微信登录页读取失败", "Failed to read WeChat login page")))?;
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
        return Err(crate::i18n::pick("微信二维码标识解析失败", "Failed to parse WeChat QR code ID").into());
    }

    let img_url = format!("https://open.weixin.qq.com/connect/qrcode/{uuid}");
    let resp = http_agent()
        .get(&img_url)
        .set("User-Agent", UA)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("获取微信二维码失败", "Failed to fetch WeChat QR code")))?;
    let mut jpeg = Vec::new();
    resp.into_reader()
        .read_to_end(&mut jpeg)
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("微信二维码读取失败", "Failed to read WeChat QR code")))?;
    if jpeg.is_empty() {
        return Err(crate::i18n::pick("微信二维码内容为空", "WeChat QR code content is empty").into());
    }
    Ok(WxQr {
        uuid,
        qr: format!("data:image/jpeg;base64,{}", b64_encode(&jpeg)),
    })
}

/// 从长轮询响应提取 (errcode, wx_code)：`window.wx_errcode=405;window.wx_code='xxx';`
pub(super) fn parse_wx_lp(text: &str) -> (i32, String) {
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
pub(super) fn cookie_value(cookies: &[String], name: &str) -> Option<String> {
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
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("微信登录凭证交换失败", "Failed to exchange WeChat login credentials")))?;
    let cookies: Vec<String> = resp
        .all("Set-Cookie")
        .into_iter()
        .map(|c| c.to_string())
        .collect();
    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("微信登录响应解析失败", "Failed to parse WeChat login response")))?;

    let sess = parse_session_from_login_json(&body, &cookies, 1).ok_or_else(|| {
        format!(
            "{} | {}",
            crate::i18n::pick("微信登录响应缺少凭证", "WeChat login response missing credentials"),
            serde_json::to_string(&body)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect::<String>()
        )
    })?;
    let nickname = (!sess.nickname.is_empty()).then(|| sess.nickname.clone());
    let musicid = Some(sess.musicid.clone());
    let musickey = Some(sess.musickey.clone());
    let encrypt_uin = (!sess.encrypt_uin.is_empty()).then(|| sess.encrypt_uin.clone());
    let refresh_token = (!sess.refresh_token.is_empty()).then(|| sess.refresh_token.clone());
    Ok(QqQrCheck {
        status: "success".into(),
        nickname,
        musicid,
        musickey,
        encrypt_uin,
        refresh_token,
        session: Some(sess),
    })
}

/// 长轮询一次微信扫码状态（调用方循环调用）：
/// 408=等待 / 404=已扫码待确认 / 405=已确认（带 wx_code，接交换）/ 402=已过期
pub fn wx_qr_check(uuid: &str) -> Result<QqQrCheck, String> {
    if uuid.is_empty() || uuid.len() > 64 || !uuid.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(crate::i18n::pick("微信登录标识无效", "Invalid WeChat login token").into());
    }
    let url = format!("https://lp.open.weixin.qq.com/connect/l/qrconnect?uuid={uuid}");
    let text = http_agent()
        .get(&url)
        .set("Referer", "https://open.weixin.qq.com/")
        .set("User-Agent", UA)
        .timeout(WX_LP_TIMEOUT)
        .call()
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("微信扫码状态查询失败", "Failed to check WeChat QR status")))?
        .into_string()
        .map_err(|e| format!("{}: {e}", crate::i18n::pick("微信扫码状态读取失败", "Failed to read WeChat QR status")))?;
    let (errcode, code) = parse_wx_lp(&text);
    match errcode {
        408 => Ok(QqQrCheck {
            status: "waiting".into(),
            nickname: None,
            musicid: None,
            musickey: None,
            encrypt_uin: None,
            refresh_token: None,
            session: None,
        }),
        404 => Ok(QqQrCheck {
            status: "scanned".into(),
            nickname: None,
            musicid: None,
            musickey: None,
            encrypt_uin: None,
            refresh_token: None,
            session: None,
        }),
        402 => Ok(QqQrCheck {
            status: "expired".into(),
            nickname: None,
            musicid: None,
            musickey: None,
            encrypt_uin: None,
            refresh_token: None,
            session: None,
        }),
        405 if !code.is_empty() => wx_exchange(&code),
        _ => {
            if errcode == 405 {
                Err(crate::i18n::pick("微信授权未返回 code", "WeChat authorization did not return a code").into())
            } else {
                Ok(QqQrCheck {
                    status: "waiting".into(),
                    nickname: None,
                    musicid: None,
                    musickey: None,
                    encrypt_uin: None,
                    refresh_token: None,
                    session: None,
                })
            }
        }
    }
}
