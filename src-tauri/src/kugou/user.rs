//! 用户歌单与扫码登录（酷狗账号）。

use super::*;

// ---------- 用户歌单（需登录） ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KgUserPlaylist {
    /// global_collection_id（字符串，可直接用于 playlist_tracks 导入曲目）
    pub id: String,
    pub name: String,
    pub track_count: i64,
}

/// 账号下全部自建/收藏歌单（cloudlist 服务，普通网关签名即可）
pub fn user_playlists(token: &str, userid: &str) -> Result<Vec<KgUserPlaylist>, String> {
    if token.is_empty() || userid.is_empty() {
        return Err("请先扫码登录酷狗账号".into());
    }
    let mut out = Vec::new();
    let pagesize = 50i64;
    // 全量分页（同网易云版）：封顶 5 页会把 250 首之后的歌单静默截断。
    // 加个上限防服务端异常时无限翻页
    for page in 1..=100i64 {
        let body = serde_json::json!({
            "userid": userid.parse::<i64>().unwrap_or(0),
            "token": token,
            "total_ver": 979,
            "type": 2,
            "page": page,
            "pagesize": pagesize,
        });
        let extra = vec![
            ("plat".into(), "1".into()),
            ("userid".into(), userid.to_string()),
            ("token".into(), token.to_string()),
        ];
        let v = gateway_ex(
            "/v7/get_all_list",
            "cloudlist.service.kugou.com",
            &extra,
            Some(&body),
            &[],
        )
        .map_err(|e| format!("获取用户歌单失败: {e}"))?;
        let list = v
            .pointer("/data/info")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        let got = list.len() as i64;
        for p in &list {
            // 已删除的歌单跳过
            if p.get("is_del").and_then(|x| x.as_i64()) == Some(1) {
                continue;
            }
            let id = p
                .get("global_collection_id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let name = p
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if id.is_empty() || name.is_empty() {
                continue;
            }
            out.push(KgUserPlaylist {
                id,
                name,
                track_count: p.get("count").and_then(|x| x.as_i64()).unwrap_or(0),
            });
        }
        if got < pagesize {
            break;
        }
    }
    Ok(out)
}

// ---------- 扫码登录 ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KgQrCheck {
    /// waiting | scanned | success | expired
    pub status: String,
    pub nickname: Option<String>,
    pub avatar: Option<String>,
    pub token: Option<String>,
    pub userid: Option<String>,
    pub vip_type: Option<i64>,
}

/// 生成登录二维码：login-user v2/qrcode（Web 端签名），返回 (key, 二维码 png dataURL)
pub fn qr_create() -> Result<(String, String), String> {
    let pairs = vec![
        ("appid".into(), "1001".into()),
        ("type".into(), "1".into()),
        ("plat".into(), "4".into()),
        (
            "qrcode_txt".into(),
            "https://h5.kugou.com/apps/loginQRCode/html/index.html?appid=1005&".into(),
        ),
        ("srcappid".into(), "2919".into()),
        ("dfid".into(), dfid_of()),
        ("mid".into(), mid_of()),
        ("uuid".into(), "-".into()),
        ("clientver".into(), CLIENTVER.into()),
        ("clienttime".into(), now_secs().to_string()),
    ];
    let mut pairs = pairs;
    pairs.push(("signature".into(), sign_web(&pairs)));
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", form_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let v = get_json(
        &format!("https://login-user.kugou.com/v2/qrcode?{query}"),
        "https://h5.kugou.com/",
        UA,
        "酷狗登录二维码",
    )?;
    let key = v
        .pointer("/data/qrcode")
        .and_then(|s| s.as_str())
        .ok_or("未获取到二维码标识")?
        .to_string();
    // 接口直接返回二维码图片（dataURL）；缺失时前端按 key 自行提示刷新
    let img = v
        .pointer("/data/qrcode_img")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    Ok((key, img))
}

/// 轮询扫码状态：0 过期 / 1 等待 / 2 待确认 / 4 成功（带 token+userid）
pub fn qr_check(key: &str) -> Result<KgQrCheck, String> {
    let pairs = vec![
        ("plat".into(), "4".into()),
        ("appid".into(), APPID.into()),
        ("srcappid".into(), "2919".into()),
        ("qrcode".into(), key.to_string()),
        ("dfid".into(), dfid_of()),
        ("mid".into(), mid_of()),
        ("uuid".into(), "-".into()),
        ("clientver".into(), CLIENTVER.into()),
        ("clienttime".into(), now_secs().to_string()),
    ];
    let mut pairs = pairs;
    pairs.push(("signature".into(), sign_web(&pairs)));
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", form_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let v = get_json(
        &format!("https://login-user.kugou.com/v2/get_userinfo_qrcode?{query}"),
        "https://h5.kugou.com/",
        UA,
        "酷狗扫码状态",
    )?;
    let status_code = v
        .pointer("/data/status")
        .and_then(|s| s.as_i64())
        .unwrap_or(1);
    let status = match status_code {
        0 => "expired",
        2 => "scanned",
        4 => "success",
        _ => "waiting",
    };
    let get_str = |k: &str| -> Option<String> {
        v.pointer(&format!("/data/{k}"))
            .and_then(|x| {
                x.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| x.as_i64().map(|n| n.to_string()))
            })
            .filter(|s| !s.is_empty())
    };
    if status == "success" {
        let token = get_str("token");
        let userid = get_str("userid");
        if token.is_none() || userid.is_none() {
            return Err("登录响应缺少凭证（token/userid）".into());
        }
        return Ok(KgQrCheck {
            status: "success".into(),
            nickname: get_str("nickname"),
            avatar: get_str("avatar"),
            token,
            userid,
            vip_type: v.pointer("/data/vip_type").and_then(|x| x.as_i64()),
        });
    }
    Ok(KgQrCheck {
        status: status.into(),
        nickname: None,
        avatar: None,
        token: None,
        userid: None,
        vip_type: None,
    })
}
