/// 网易云音乐接口客户端（eapi / weapi 协议）
///
/// 仅以官方客户端同款协议访问音乐平台：搜索与播放链接获取按用户账号自身
/// 权益进行（未登录可播放免费曲目，登录后按其会员权益），不包含任何绕过
/// 付费/版权限制的功能。
use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};
use md5::{Digest, Md5};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const WEAPI_PRESET: &[u8; 16] = b"0CoJUm6Qyw8W8jud";
const WEAPI_IV: &[u8; 16] = b"0102030405060708";
const WEAPI_RSA_E_HEX: &str = "010001";
const WEAPI_RSA_N_HEX: &str = "e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b725152b3ab17a876aea8a5aa76d2e417629ec4ee341f56135fccf695280104e0312ecbda92557c93870114af6c9d05c4f7f0c3685b7a46bee255932575cce10b424d813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462db0a22b8e7";
const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
const TIMEOUT: Duration = Duration::from_secs(12);

// ---------- 加密 ----------

#[allow(dead_code)]
fn md5_hex(data: &[u8]) -> String {
    let mut h = Md5::new();
    h.update(data);
    hex::encode(h.finalize())
}

#[allow(dead_code)]
fn aes_ecb_encrypt_hex(key: &[u8; 16], data: &[u8]) -> String {
    let cipher = aes::Aes128::new(GenericArray::from_slice(key));
    let mut padded = data.to_vec();
    let pad = 16 - (padded.len() % 16);
    padded.extend(std::iter::repeat(pad as u8).take(pad));
    let mut out = Vec::with_capacity(padded.len());
    for chunk in padded.chunks(16) {
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        out.extend_from_slice(&block);
    }
    hex::encode_upper(out)
}

/// AES-128-CBC + PKCS7（手写链接：c_i = E(p_i XOR c_{i-1})）
fn aes_cbc_encrypt(key: &[u8; 16], data: &[u8], iv: &[u8; 16]) -> Vec<u8> {
    let cipher = aes::Aes128::new(GenericArray::from_slice(key));
    let mut padded = data.to_vec();
    let pad = 16 - (padded.len() % 16);
    padded.extend(std::iter::repeat(pad as u8).take(pad));
    let mut prev = *iv;
    let mut out = Vec::with_capacity(padded.len());
    for chunk in padded.chunks_mut(16) {
        for (b, p) in chunk.iter_mut().zip(prev.iter()) {
            *b ^= p;
        }
        let mut block = GenericArray::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        prev.copy_from_slice(&block);
        out.extend_from_slice(&block);
    }
    out
}

fn b64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn rsa_encrypt_hex(input: &[u8]) -> Result<String, String> {
    let mut rev = input.to_vec();
    rev.reverse();
    // 文本按十六进制字节解读为大整数，裸 RSA（无填充，与平台约定一致）
    let m_bytes = hex::decode(&hex::encode(&rev)).map_err(|e| e.to_string())?;
    let e_bytes =
        hex::decode(WEAPI_RSA_E_HEX).map_err(|e| format!("RSA 指数配置错误: {e}"))?;
    let n_bytes = hex::decode(WEAPI_RSA_N_HEX).map_err(|e| format!("RSA 模数配置错误: {e}"))?;
    let m = BigUint::from_bytes_be(&m_bytes);
    let e = BigUint::from_bytes_be(&e_bytes);
    let n = BigUint::from_bytes_be(&n_bytes);
    let c = m.modpow(&e, &n);
    let s = hex::encode_upper(c.to_bytes_be());
    Ok(format!("{s:0>256}"))
}

fn random_secret() -> [u8; 16] {
    use rand::Rng;
    const CHARSET: &[u8] = b"poiuytrewqasdfghjklmnbvcxzQWERTYUIOPASDFGHJKLZXCVBNM0123456789";
    let mut rng = rand::thread_rng();
    let mut out = [0u8; 16];
    for b in out.iter_mut() {
        *b = CHARSET[rng.gen_range(0..CHARSET.len())];
    }
    out
}

fn form_encode(s: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

/// weapi 请求体（AES-CBC 双层 + RSA），返回 "params=...&encSecKey=..."
fn weapi_body(payload: &str) -> Result<String, String> {
    let secret = random_secret();
    let first = b64_encode(&aes_cbc_encrypt(WEAPI_PRESET, payload.as_bytes(), WEAPI_IV));
    let second = b64_encode(&aes_cbc_encrypt(&secret, first.as_bytes(), WEAPI_IV));
    let enc_sec = rsa_encrypt_hex(&secret)?;
    Ok(format!(
        "params={}&encSecKey={}",
        form_encode(&second),
        enc_sec
    ))
}

// ---------- HTTP ----------

fn cookie_header(music_u: Option<&str>) -> String {
    match music_u {
        Some(u) if !u.is_empty() => format!("os=pc; appver=2.10.13; MUSIC_U={u}"),
        _ => "os=pc; appver=2.10.13".to_string(),
    }
}

fn post_form(url: &str, body: &str, music_u: Option<&str>) -> Result<serde_json::Value, String> {
    let resp = ureq::post(url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("User-Agent", UA)
        .set("Cookie", &cookie_header(music_u))
        .timeout(TIMEOUT)
        .send_string(body)
        .map_err(|e| format!("网易云接口请求失败: {e}"))?;
    let status = resp.status();
    let text = resp
        .into_string()
        .map_err(|e| format!("网易云接口响应读取失败: {e}"))?;
    serde_json::from_str(&text).map_err(|e| {
        let head: String = text.chars().take(180).collect();
        format!("网易云接口响应异常（HTTP {status}）: {e} | body: {head}")
    })
}

fn weapi_post(path: &str, payload: &str, music_u: Option<&str>) -> Result<serde_json::Value, String> {
    let url = format!("https://music.163.com{path}");
    let body = weapi_body(payload)?;
    post_form(&url, &body, music_u)
}

// ---------- 数据模型 ----------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetSong {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub ar: Vec<NetArtist>,
    #[serde(default, alias = "artists", skip_serializing)]
    pub ar_legacy: Vec<NetArtist>,
    #[serde(default)]
    pub al: NetAlbum,
    #[serde(default, alias = "album", skip_serializing)]
    pub al_legacy: serde_json::Value,
    #[serde(default)]
    pub dt: i64,
    #[serde(default, alias = "duration", skip_serializing)]
    pub dt_legacy: i64,
    #[serde(default)]
    pub fee: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct NetArtist {
    #[serde(default)]
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct NetAlbum {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "picUrl")]
    pub pic_url: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetSearchResult {
    pub total: i64,
    pub songs: Vec<NetSong>,
}

#[allow(dead_code)]
impl NetSong {
    pub fn artist_str(&self) -> String {
        let artists = if self.ar.is_empty() {
            &self.ar_legacy
        } else {
            &self.ar
        };
        artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(" / ")
    }

    pub fn duration_ms(&self) -> i64 {
        if self.dt > 0 {
            self.dt
        } else {
            self.dt_legacy
        }
    }

    pub fn album_name(&self) -> String {
        if !self.al.name.is_empty() {
            return self.al.name.clone();
        }
        self.al_legacy
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn cover_url(&self) -> Option<String> {
        if let Some(u) = &self.al.pic_url {
            return Some(u.clone());
        }
        self.al_legacy
            .get("picUrl")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

// ---------- 业务接口 ----------

/// 搜索歌曲（标准 web 搜索接口，匿名可用，字段兼容新旧两套）
pub fn search(
    keyword: &str,
    limit: i64,
    offset: i64,
    music_u: Option<&str>,
) -> Result<NetSearchResult, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(NetSearchResult { total: 0, songs: vec![] });
    }
    let payload = serde_json::json!({
        "s": keyword, "type": 1, "offset": offset,
        "limit": limit, "total": true
    })
    .to_string();

    let resp = weapi_post("/weapi/search/get", &payload, music_u)?;
    let code = resp.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 200 {
        return Err(format!("网易云搜索失败（code {code}）"));
    }
    let songs_json = resp
        .pointer("/result/songs")
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();
    let total = resp
        .pointer("/result/songCount")
        .and_then(|c| c.as_i64())
        .unwrap_or(songs_json.len() as i64);
    let mut songs: Vec<NetSong> = songs_json
        .into_iter()
        .filter_map(|s| serde_json::from_value(s).ok())
        .map(|mut s: NetSong| {
            // 归一化：旧版字段（artists/album/duration）并入新版字段（ar/al/dt），
            // 保证前端始终读到一致的字段
            if s.ar.is_empty() {
                s.ar = std::mem::take(&mut s.ar_legacy);
            }
            if s.dt == 0 {
                s.dt = s.dt_legacy;
            }
            if s.al.name.is_empty() {
                s.al.name = s
                    .al_legacy
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            if s.al.pic_url.is_none() {
                s.al.pic_url = s
                    .al_legacy
                    .get("picUrl")
                    .and_then(|v| v.as_str())
                    .map(|x| x.to_string());
            }
            s
        })
        .collect();
    // 搜索接口的专辑对象常常缺少封面，批量补一次歌曲详情
    enrich_covers(&mut songs, music_u);
    Ok(NetSearchResult { total, songs })
}

/// 批量拉取歌曲详情，补齐缺失的专辑封面 picUrl
fn enrich_covers(songs: &mut [NetSong], music_u: Option<&str>) {
    let missing: Vec<i64> = songs
        .iter()
        .filter(|s| s.al.pic_url.is_none() && s.al_legacy.get("picUrl").is_none())
        .map(|s| s.id)
        .collect();
    if missing.is_empty() {
        return;
    }
    let ids_json = serde_json::to_string(&missing).unwrap_or_default();
    let c_json = serde_json::to_string(
        &missing
            .iter()
            .map(|id| serde_json::json!({ "id": id }))
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default();
    let payload = serde_json::json!({ "c": c_json, "ids": ids_json, "csrf_token": "" }).to_string();
    if let Ok(resp) = weapi_post("/weapi/v3/song/detail", &payload, music_u) {
        if let Some(details) = resp.get("songs").and_then(|s| s.as_array()) {
            for d in details {
                let id = d.get("id").and_then(|v| v.as_i64());
                let pic = d
                    .pointer("/al/picUrl")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                if let (Some(id), Some(pic)) = (id, pic) {
                    if let Some(s) = songs.iter_mut().find(|s| s.id == id) {
                        if s.al.pic_url.is_none() {
                            s.al.pic_url = Some(pic);
                        }
                    }
                }
            }
        }
    }
}

/// 按音质请求播放直链（weapi，需登录 cookie），从所选音质逐级回退
/// 返回 (url, br, ext)
pub fn song_url(
    id: i64,
    music_u: Option<&str>,
    quality: &str,
) -> Result<Option<(String, i64, String)>, String> {
    let music_u = music_u.filter(|s| !s.is_empty());
    if music_u.is_none() {
        return Err("未登录网易云账号，无法获取播放链接，请先扫码登录".into());
    }
    let ladder: Vec<i64> = match quality {
        "lossless" => vec![999000, 320000, 128000],
        "standard" => vec![128000],
        _ => vec![320000, 128000],
    };
    let mut last_hint = String::from("该歌曲没有可播放的音频");
    for br in ladder {
        let payload = serde_json::json!({
            "ids": format!("[{id}]"), "br": br, "csrf_token": ""
        })
        .to_string();
        let resp = weapi_post("/weapi/song/enhance/player/url", &payload, music_u)?;
        let code = resp.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
        if code != 200 {
            return Err(format!("获取播放链接失败（code {code}）"));
        }
        let first = resp
            .pointer("/data/0")
            .cloned()
            .ok_or_else(|| "响应中没有该歌曲的数据".to_string())?;
        let item_code = first.get("code").and_then(|c| c.as_i64()).unwrap_or(200);
        let url = first
            .get("url")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty());
        if let Some(u) = url {
            let ext = u
                .split('?')
                .next()
                .unwrap_or("")
                .rsplit('.')
                .next()
                .unwrap_or("mp3")
                .to_lowercase();
            let real_br = first.get("br").and_then(|b| b.as_i64()).unwrap_or(br);
            return Ok(Some((u, real_br, ext)));
        }
        last_hint = match item_code {
            404 => "该歌曲暂无版权音频（可能需要 VIP 或已下架）".into(),
            402 => "该内容需要付费".into(),
            600 => "该歌曲需要 VIP 权益".into(),
            _ => last_hint,
        };
    }
    Err(last_hint)
}

/// 获取歌词（LRC 文本，含逐行时间标签）
pub fn lyric(id: i64, music_u: Option<&str>) -> Result<Option<String>, String> {
    let music_u = music_u.filter(|s| !s.is_empty());
    let payload = serde_json::json!({
        "id": id.to_string(), "tv": "-1", "lv": "-1",
        "rv": "-1", "kv": "-1", "csrf_token": ""
    })
    .to_string();
    let resp = weapi_post("/weapi/song/lyric", &payload, music_u)?;
    let code = resp.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 200 {
        return Err(format!("获取歌词失败（code {code}）"));
    }
    let lrc = resp
        .pointer("/lrc/lyric")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty());
    Ok(lrc)
}

/// 从 music_u 反查账号 ID（老版本登录时未存 uid 的兜底）
pub fn resolve_uid(music_u: &str) -> Result<i64, String> {
    let url = "https://music.163.com/api/nuser/account/get?csrf_token=";
    let resp = ureq::get(url)
        .set("Cookie", &cookie_header(Some(music_u)))
        .set("User-Agent", UA)
        .set("Referer", "https://music.163.com/")
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("账号信息请求失败: {e}"))?;
    let text = resp
        .into_string()
        .map_err(|e| format!("账号信息读取失败: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("账号信息解析失败: {e}"))?;
    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 200 {
        return Err(format!("账号信息获取失败（code {code}）"));
    }
    v.pointer("/account/id")
        .and_then(|x| x.as_i64())
        .ok_or_else(|| "响应中缺少账号 ID".into())
}

/// 获取登录账号的歌单列表
pub fn user_playlists(uid: i64, music_u: &str) -> Result<Vec<crate::models::UserPlaylistMeta>, String> {
    let payload = serde_json::json!({ "uid": uid.to_string(), "offset": "0", "limit": "60" }).to_string();
    let resp = weapi_post("/weapi/user/playlist", &payload, Some(music_u))?;
    let code = resp.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 200 {
        return Err(format!("获取歌单列表失败（code {code}）"));
    }
    let mut out = Vec::new();
    if let Some(list) = resp.pointer("/playlist").and_then(|v| v.as_array()) {
        for p in list {
            let id = p.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let count = p.get("trackCount").and_then(|v| v.as_i64()).unwrap_or(0);
            if id != 0 && !name.is_empty() {
                out.push(crate::models::UserPlaylistMeta { id, name, track_count: count });
            }
        }
    }
    Ok(out)
}

/// 获取歌单内的全部歌曲（明文 API v6，字段与搜索一致）
pub fn playlist_tracks(pid: i64, music_u: &str) -> Result<Vec<NetSong>, String> {
    let url = format!(
        "https://music.163.com/api/v6/playlist/detail?id={pid}&n=1000&csrf_token="
    );
    let resp = ureq::get(&url)
        .set("Cookie", &cookie_header(Some(music_u)))
        .set("User-Agent", UA)
        .set("Referer", "https://music.163.com/")
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("获取歌单详情失败: {e}"))?;
    let text = resp
        .into_string()
        .map_err(|e| format!("歌单详情读取失败: {e}"))?;
    let v: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("歌单详情解析失败: {e} | body: {}", text.chars().take(120).collect::<String>()))?;
    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 200 {
        return Err(format!("获取歌单详情失败（code {code}）"));
    }
    let mut out = Vec::new();
    if let Some(list) = v.pointer("/playlist/trackIds").and_then(|x| x.as_array()) {
        // trackIds 全量；详情在 playlist/tracks（可能截断）——用统一归一化解析
        if let Some(tracks) = v.pointer("/playlist/tracks").and_then(|x| x.as_array()) {
            for t in tracks {
                if let Ok(song) = serde_json::from_value::<NetSong>(t.clone()) {
                    out.push(song);
                }
            }
        }
        if out.len() < list.len() {
            // tracks 被截断时按 trackIds 计数提示（v6 通常一次性给全）
            eprintln!(
                "[netease] playlist {pid}: got {} tracks, ids {}",
                out.len(),
                list.len()
            );
        }
    }
    Ok(out)
}

/// 创建扫码登录二维码，返回 (unikey, qr_png_base64)
pub fn qr_create() -> Result<(String, String), String> {
    let resp = weapi_post("/weapi/login/qrcode/unikey", r#"{"type":"1"}"#, None)?;
    let unikey = resp
        .get("unikey")
        .and_then(|v| v.as_str())
        .or_else(|| resp.pointer("/data/unikey").and_then(|v| v.as_str()))
        .ok_or_else(|| format!("获取登录二维码失败: {}", resp))?
        .to_string();

    let text = format!("https://music.163.com/login?codekey={unikey}");
    let code = qrcode::QrCode::with_error_correction_level(
        text.as_bytes(),
        qrcode::EcLevel::M,
    )
    .map_err(|e| format!("生成二维码失败: {e:?}"))?;
    let img = code
        .render::<image::Luma<u8>>()
        .quiet_zone(true)
        .min_dimensions(240, 240)
        .build();
    let mut png_bytes = Vec::new();
    image::DynamicImage::ImageLuma8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("编码二维码图片失败: {e}"))?;
    let b64 = b64_encode(&png_bytes);
    Ok((unikey, format!("data:image/png;base64,{b64}")))
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QrCheckResult {
    /// waiting | scanned | success | expired
    pub status: String,
    pub nickname: Option<String>,
    pub music_u: Option<String>,
    pub user_id: Option<i64>,
}

/// 轮询扫码状态；成功时从 Set-Cookie 提取 MUSIC_U
pub fn qr_check(key: &str) -> Result<QrCheckResult, String> {
    let payload = serde_json::json!({ "key": key, "type": "1" }).to_string();
    let url = "https://music.163.com/weapi/login/qrcode/client/login";
    let resp = ureq::post(url)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("User-Agent", UA)
        .set("Cookie", &cookie_header(None))
        .timeout(TIMEOUT)
        .send_string(&weapi_body(&payload)?)
        .map_err(|e| format!("登录状态查询失败: {e}"))?;

    let set_cookies: Vec<String> = resp
        .all("Set-Cookie")
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let text = resp
        .into_string()
        .map_err(|e| format!("登录响应读取失败: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("登录响应解析失败: {e} | body: {}", &text.chars().take(120).collect::<String>()))?;

    let code = json.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    let status = match code {
        800 => "expired",
        801 => "waiting",
        802 => "scanned",
        803 => "success",
        _ => "waiting",
    };
    if status != "success" {
        return Ok(QrCheckResult { status: status.into(), nickname: None, music_u: None, user_id: None });
    }
    let music_u = set_cookies
        .iter()
        .find_map(|c| {
            let seg = c.split(';').next()?;
            seg.trim().strip_prefix("MUSIC_U=").map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty());
    let nickname = json
        .pointer("/profile/nickname")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let user_id = json.pointer("/profile/userId").and_then(|n| n.as_i64());
    if music_u.is_none() {
        return Err("登录成功但未取到登录凭证，请重试".into());
    }
    Ok(QrCheckResult { status: "success".into(), nickname, music_u, user_id })
}

/// 获取账号“我喜欢”列表的歌曲 ID
pub fn like_list(uid: i64, music_u: &str) -> Result<Vec<i64>, String> {
    let payload = serde_json::json!({ "uid": uid.to_string(), "csrf_token": "" }).to_string();
    let resp = weapi_post("/weapi/song/likelist/get", &payload, Some(music_u))?;
    let code = resp.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 200 {
        return Err(format!("获取喜欢列表失败（code {code}）"));
    }
    Ok(resp
        .get("ids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default())
}

/// 收藏 / 取消收藏（写入账号的“我喜欢”）
pub fn like(id: i64, like: bool, music_u: &str) -> Result<(), String> {
    let payload = serde_json::json!({
        "trackId": id.to_string(),
        "like": if like { "true" } else { "false" },
        "time": "3", "csrf_token": ""
    })
    .to_string();
    let resp = weapi_post("/weapi/song/like?alg=RT&time=25", &payload, Some(music_u))?;
    let code = resp.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 200 {
        let msg = resp
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("未知错误");
        return Err(format!("收藏失败（code {code}）: {msg}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search() {
        let r = search("晴天", 8, 0, None).expect("search failed");
        println!("total={} songs={}", r.total, r.songs.len());
        for s in &r.songs {
            println!(
                "  [{}] {} - {} (fee={}, dt={}ms, al={:?})",
                s.id, s.name, s.artist_str(), s.fee, s.duration_ms(), s.album_name()
            );
        }
        assert!(!r.songs.is_empty(), "search returned no songs");
    }
}

#[cfg(test)]
mod cover_lyric_tests {
    use super::*;

    #[test]
    fn test_covers_and_lyric() {
        let r = search("晴天", 5, 0, None).expect("search failed");
        for s in &r.songs {
            println!("  [{}] {} pic={:?}", s.id, s.name, s.al.pic_url.as_deref().map(|u| &u[..u.len().min(48)]));
        }
        assert!(r.songs.iter().any(|s| s.al.pic_url.is_some()), "no covers resolved");

        // 用有封面核对的第一首验证歌词（匿名即可拿大部分歌词）
        let any_id = r.songs[0].id;
        let lrc = lyric(any_id, None).expect("lyric failed");
        match &lrc {
            Some(t) => println!("lyric[{}] first 80 chars: {}", any_id, &t.chars().take(80).collect::<String>()),
            None => println!("lyric[{}]: none", any_id),
        }
    }
}

#[cfg(test)]
mod playlist_tests {
    use super::*;

    #[test]
    fn test_playlist_detail_anonymous() {
        // 匿名也能拿公开歌单（用云音乐官方示例歌单 ID）
        let r = playlist_tracks(60198, "");
        match r {
            Ok(songs) => println!("got {} songs, first: {:?}", songs.len(), songs.first().map(|s| (&s.name, &s.ar))),
            Err(e) => println!("ERR: {e}"),
        }
    }
}
