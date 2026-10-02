//! 酷狗音乐接口客户端（搜索 / 播放链接 / KRC 逐字歌词 / 排行榜 / 歌单广场 / 扫码登录）
//!
//! 匿名访问其公共接口：免费曲目直接返回可播直链；付费/VIP 曲目需用户
//! 扫码登录自己的账号后按会员权益获取播放链接，不包含任何绕过付费 /
//! 版权限制的功能。
//!
//! 接口协议参考开源实现 MakcRe/KuGouMusicApi（MIT）：
//! - 网关请求（gateway.kugou.com + x-router）：Android 客户端参数 +
//!   MD5 签名（参数排序 k=v 拼接 + 盐）；
//! - 老明文接口（搜索/榜单/歌词/播放信息）：免签名直连；
//! - 登录（login-user.kugou.com）：Web 端盐值签名。
//!
//! 设备标识（guid/mid/dfid）为进程内随机生成，仅用于标识本次安装，
//! 不涉及任何用户隐私数据。

use base64::Engine as _;
use flate2::read::ZlibDecoder;
use md5::{Digest as Md5Digest, Md5};
use num_bigint::BigUint;
use rand::Rng;
use serde::Serialize;
use std::io::Read;
use std::time::Duration;

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
/// 网关接口要求的安卓客户端 UA
const ANDROID_UA: &str = "Android15-1070-11083-46-0-DiscoveryDRADProtocol-wifi";
const TIMEOUT: Duration = Duration::from_secs(12);
const SALT_ANDROID: &str = "OIlwieks28dk2k092lksi2UIkp";
const SALT_WEB: &str = "NVPh5oo715z5DIWAeQlhMDsWXXQV4hwt";
/// /v5/url 的 key 盐（tracker 取链接签名）
const SALT_URL_KEY: &str = "57ae12eb6890223e355ccfcb74edf70d";
const APPID: &str = "1005";
const CLIENTVER: &str = "20489";

fn md5_hex(s: &str) -> String {
    let mut h = Md5::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------- 设备标识 / dfid（注册制，见 register_dev） ----------
//
// 注意：服务端把 dfid 与 mid 绑定校验（不匹配报 "err appid(srcappid) or
// clientver or mid or dfid"），二者必须成对持久化、成对注入。

static REGISTERED_MID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
static REGISTERED_DFID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
static REGISTERED_ACCOUNT: std::sync::Mutex<(String, String)> =
    std::sync::Mutex::new((String::new(), String::new()));

/// 注入已持久化的 mid + dfid 对（命令层从 settings 读出后调用）。
/// 只注入其中一个都是错的——服务端校验二者绑定。
pub fn set_device(mid: &str, dfid: &str) {
    if !mid.is_empty() {
        *REGISTERED_MID.lock().unwrap() = Some(mid.to_string());
    }
    if !dfid.is_empty() {
        *REGISTERED_DFID.lock().unwrap() = Some(dfid.to_string());
    }
}

/// 命令层注入登录账号（register_dev 的 p 参数带 uid/token 提升信任度）
pub fn set_account(token: &str, userid: &str) {
    *REGISTERED_ACCOUNT.lock().unwrap() = (token.to_string(), userid.to_string());
}

/// 生成的 mid 缓存：可重置——服务端按 mid 记忆设备，空 dfid 重试
/// 必须换全新 mid，OnceLock 永不重置会导致同一 mid 无限重试
static GEN_MID: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// 生成一个新 mid（32 位十六进制含连字符，转十进制大整数）
fn generate_mid() -> String {
    let mut rng = rand::thread_rng();
    let uuid = (0..32)
        .map(|i| {
            if [8, 12, 16, 20].contains(&i) {
                '-'
            } else {
                char::from(b'0' + rng.gen_range(0..16))
            }
        })
        .collect::<String>();
    let guid = md5_hex(&uuid);
    BigUint::parse_bytes(guid.as_bytes(), 16)
        .map(|n| n.to_string())
        .unwrap_or(guid)
}

/// 当前 mid：优先用注入值；未注入时生成一次并缓存（与 dfid_of 同生命周期）
fn mid_of() -> String {
    if let Some(m) = REGISTERED_MID.lock().unwrap().as_ref() {
        return m.clone();
    }
    let mut gen = GEN_MID.lock().unwrap();
    if let Some(m) = gen.as_ref() {
        return m.clone();
    }
    let m = generate_mid();
    *gen = Some(m.clone());
    m
}

/// 丢弃注入值与生成的 mid 缓存，下次 mid_of 生成全新值
fn reset_mid() {
    *REGISTERED_MID.lock().unwrap() = None;
    *GEN_MID.lock().unwrap() = None;
}

/// 当前生效的 mid（命令层持久化用；与 dfid 配对）
pub fn current_mid() -> String {
    mid_of()
}

/// 当前生效的 dfid（命令层持久化用；未注册时为空串）
pub fn current_dfid() -> String {
    REGISTERED_DFID
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_default()
}

/// 当前 dfid；未注册时立即走 register_dev 注册
fn dfid_of() -> String {
    if let Some(d) = REGISTERED_DFID.lock().unwrap().as_ref() {
        return d.clone();
    }
    match register_dev() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[kugou] register_dev 失败，回退随机 dfid: {e}");
            rand_dfid()
        }
    }
}

/// 强制重新注册（风控 20028 后调用），成功返回新 dfid
fn re_register() -> Option<String> {
    register_dev().ok()
}

/// 设备注册（userservice 风控接口）：POST 一段 AES-128-CBC 加密的设备
/// 信息 JSON，URL 参数 p 为 RSA(PKCS1v1.5) 加密的会话串（aes key + uid + token），
/// 响应体同为 AES 密文，解出 data.dfid。匿名（token 空）也可注册。
pub fn register_dev() -> Result<String, String> {
    // 服务端对已注册过的 mid 返回空 data：进程内有 dfid 缓存则复用；
    // 缓存也没有（服务端记得此 mid，但本进程从未拿到过 dfid）→
    // 换全新 mid 再试一次。最多重试一轮：mid 缓存已可重置，超过一轮
    // 仍空 dfid 说明服务端异常，报错而非继续请求
    for _ in 0..2 {
        match register_dev_once()? {
            Some(dfid) => return Ok(dfid),
            None => reset_mid(),
        }
    }
    Err("register_dev: 换新 mid 后服务端仍返回空 dfid".into())
}

/// 单次注册尝试；Ok(None) = 服务端认识该 mid 但未下发 dfid 且无缓存可复用
fn register_dev_once() -> Result<Option<String>, String> {
    const RSA_N_HEX: &str = "c8006ed03842d2628209bd314984ca5ed6cfe06e30c95f9d4704d9c49791d7a935ba950ecb0bc8ebf5f5994f0bac927a7eb151b3c1de343303fa539c83136eccfd7d7e511e2dbce18eaa9f784c9b50d443e75865979e0a5e216e46c684066a8d6b998580bbaa22d73f5790286bb14742e83244e44db6d707ffe162c5c7002d45";
    const RSA_E: u32 = 65537;
    let (token, userid) = REGISTERED_ACCOUNT.lock().unwrap().clone();
    let mut rng = rand::thread_rng();
    let guid: String = (0..32)
        .map(|i| {
            if [8, 12, 16, 20].contains(&i) {
                '-'
            } else {
                char::from(b'0' + rng.gen_range(0..16))
            }
        })
        .collect();

    // AES 会话：key = 6 位随机串，密钥/IV = md5(key) 的前后 16 字节
    let session: String = (0..6)
        .map(|_| char::from(b'a' + rng.gen_range(0..26)))
        .collect();
    let k = md5_hex(&session);
    let (aes_key, aes_iv) = (&k[0..16], &k[16..32]);

    let device_info = serde_json::json!({
        "availableRamSize": 4983533568i64, "availableRomSize": 48114719i64,
        "availableSDSize": 48114717i64, "basebandVer": "", "batteryLevel": 100,
        "batteryStatus": 3, "brand": "Redmi", "buildSerial": "unknown", "device": "marble",
        "imei": guid, "imsi": "", "manufacturer": "Xiaomi", "uuid": guid,
        "accelerometer": false, "accelerometerValue": "", "gravity": false, "gravityValue": "",
        "gyroscope": false, "gyroscopeValue": "", "light": false, "lightValue": "",
        "magnetic": false, "magneticValue": "", "orientation": false, "orientationValue": "",
        "pressure": false, "pressureValue": "", "step_counter": false, "step_counterValue": "",
        "temperature": false, "temperatureValue": "",
    });
    let plain = serde_json::to_string(&device_info).map_err(|e| e.to_string())?;
    let body = aes_128_cbc_encrypt(plain.as_bytes(), aes_key, aes_iv);
    let body_b64 = base64::engine::general_purpose::STANDARD.encode(&body);

    // p = RSA(0x00||0x02||PS||0x00||{"aes":session,"uid":userid,"token":token})
    let portrait = serde_json::json!({ "aes": session, "uid": userid, "token": token });
    let pplain = serde_json::to_string(&portrait).map_err(|e| e.to_string())?;
    let p = rsa_pkcs1_encrypt(pplain.as_bytes(), RSA_N_HEX, RSA_E)?;

    let clienttime = now_secs().to_string();
    let mut pairs: Vec<(String, String)> = vec![
        ("dfid".into(), "-".into()),
        ("mid".into(), mid_of()),
        ("uuid".into(), "-".into()),
        ("appid".into(), APPID.into()),
        ("clientver".into(), CLIENTVER.into()),
        ("clienttime".into(), clienttime.clone()),
        ("part".into(), "1".into()),
        ("platid".into(), "1".into()),
        ("p".into(), p),
    ];
    pairs.push(("signature".into(), sign_android(&pairs, &body_b64)));
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", form_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let resp = ureq::post(&format!(
        "https://userservice.kugou.com/risk/v2/r_register_dev?{query}"
    ))
    .set("User-Agent", ANDROID_UA)
    .set("mid", &mid_of())
    .timeout(TIMEOUT)
    .send_string(&body_b64)
    .map_err(|e| format!("register_dev 请求失败: {e}"))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .read_to_end(&mut buf)
        .map_err(|e| format!("register_dev 响应读取失败: {e}"))?;
    let v = aes_128_cbc_decrypt(&buf, aes_key, aes_iv)?;
    if let Some(d) = v.pointer("/data/dfid").and_then(|x| x.as_str()) {
        let dfid = d.to_string();
        *REGISTERED_DFID.lock().unwrap() = Some(dfid.clone());
        return Ok(Some(dfid));
    }
    // 同一 mid 重复注册时服务端返回空 data：复用进程内缓存的 dfid
    if let Some(d) = REGISTERED_DFID.lock().unwrap().as_ref() {
        return Ok(Some(d.clone()));
    }
    Ok(None)
}

/// AES-128-CBC 加密（PKCS7）
fn aes_128_cbc_encrypt(plain: &[u8], key: &str, iv: &str) -> Vec<u8> {
    use aes::cipher::{BlockEncrypt, KeyInit};
    use aes::Block;
    let mut key_buf = [0u8; 16];
    key_buf.copy_from_slice(key.as_bytes());
    let mut iv_buf = [0u8; 16];
    iv_buf.copy_from_slice(iv.as_bytes());
    let cipher = aes::Aes128::new((&key_buf).into());
    let pad = 16 - (plain.len() % 16);
    let mut data = plain.to_vec();
    data.extend(std::iter::repeat(pad as u8).take(pad));
    let mut prev = iv_buf;
    let mut out = Vec::with_capacity(data.len());
    for chunk in data.chunks(16) {
        let mut block = [0u8; 16];
        block.copy_from_slice(chunk);
        for (b, p) in block.iter_mut().zip(prev.iter()) {
            *b ^= p;
        }
        let mut gb = Block::from(block);
        cipher.encrypt_block(&mut gb);
        prev.copy_from_slice(gb.as_slice());
        out.extend_from_slice(gb.as_slice());
    }
    out
}

/// AES-128-CBC 解密（PKCS7），解析为 JSON
fn aes_128_cbc_decrypt(cipher_bytes: &[u8], key: &str, iv: &str) -> Result<serde_json::Value, String> {
    use aes::cipher::{BlockDecrypt, KeyInit};
    use aes::Block;
    if cipher_bytes.is_empty() || cipher_bytes.len() % 16 != 0 {
        return Err("register_dev 响应长度异常".into());
    }
    let mut key_buf = [0u8; 16];
    key_buf.copy_from_slice(key.as_bytes());
    let mut iv_buf = [0u8; 16];
    iv_buf.copy_from_slice(iv.as_bytes());
    let cipher = aes::Aes128::new((&key_buf).into());
    let mut out = Vec::with_capacity(cipher_bytes.len());
    let mut prev = iv_buf;
    for chunk in cipher_bytes.chunks(16) {
        let mut gb = Block::clone_from_slice(chunk);
        cipher.decrypt_block(&mut gb);
        for (b, p) in gb.as_slice().iter().zip(prev.iter()) {
            out.push(b ^ p);
        }
        prev.copy_from_slice(chunk);
    }
    if let Some(&pad) = out.last() {
        if (1..=16).contains(&pad) && out.len() >= pad as usize {
            out.truncate(out.len() - pad as usize);
        }
    }
    parse_json(&String::from_utf8_lossy(&out), "register_dev 响应解密")
}

/// RSA PKCS1 v1.5 加密（固定公钥，e=65537），返回小写 hex
fn rsa_pkcs1_encrypt(data: &[u8], n_hex: &str, e: u32) -> Result<String, String> {
    let n = BigUint::parse_bytes(n_hex.as_bytes(), 16).ok_or("RSA modulus 解析失败")?;
    let key_len = (n.bits() as usize + 7) / 8;
    if data.len() + 11 > key_len {
        return Err("RSA 明文过长".into());
    }
    // EM = 0x00 || 0x02 || PS(非零随机) || 0x00 || data
    let mut em = Vec::with_capacity(key_len);
    em.push(0u8);
    em.push(2u8);
    let ps_len = key_len - data.len() - 3;
    let mut rng = rand::thread_rng();
    for _ in 0..ps_len {
        let mut b;
        loop {
            b = rng.gen::<u8>();
            if b != 0 {
                break;
            }
        }
        em.push(b);
    }
    em.push(0u8);
    em.extend_from_slice(data);
    let m = BigUint::from_bytes_be(&em);
    let c = m.modpow(&BigUint::from(e), &n);
    let ch = c.to_bytes_be();
    let mut full = vec![0u8; key_len - ch.len()];
    full.extend_from_slice(&ch);
    Ok(hex::encode(full))
}

fn rand_dfid() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| char::from(b'a' + rng.gen_range(0..26)))
        .collect()
}

// ---------- HTTP 基础 ----------

fn form_encode(s: &str) -> String {
    // RFC3986 unreserved（字母数字与 - _ . ~）不编码，与官方客户端一致；
    // 之前的 NON_ALPHANUMERIC 会把 _ 编码成 %5F，酷狗网关按原始 query
    // 验签，直接报参数错误（err appid/clientver/mid/dfid）
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 读取响应体文本
fn read_body(resp: ureq::Response) -> Result<String, String> {
    let mut buf = String::new();
    resp.into_reader()
        .read_to_string(&mut buf)
        .map_err(|e| format!("酷狗响应读取失败: {e}"))?;
    Ok(buf)
}

fn parse_json(text: &str, what: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(text).map_err(|e| {
        format!(
            "{what}解析失败: {e} | body: {}",
            text.chars().take(150).collect::<String>()
        )
    })
}

/// GET 并返回响应体文本（非 2xx 时也读取响应体，便于错误分类）
fn get_text(url: &str, referer: &str, ua: &str) -> Result<String, String> {
    match ureq::get(url)
        .set("User-Agent", ua)
        .set("Referer", referer)
        .timeout(TIMEOUT)
        .call()
    {
        Ok(resp) => read_body(resp),
        Err(ureq::Error::Status(_, resp)) => read_body(resp),
        Err(e) => Err(format!("请求酷狗接口失败: {e}")),
    }
}

fn get_json(url: &str, referer: &str, ua: &str, what: &str) -> Result<serde_json::Value, String> {
    let text = get_text(url, referer, ua)?;
    parse_json(&text, what)
}

// ---------- 签名与网关请求 ----------

/// Android 网关签名：盐 + "k=v"（按 key 排序，重复 key 保持出现顺序）拼接
/// + 请求体 + 盐 取 MD5。
/// 注意必须对 key 排序而非对整个 "k=v" 串排序：/v5/url 同时带设备
/// clientver 与取链 clientver 两个同名参数，按值排序会得到不同顺序，
/// 服务端验签直接报参数错误。
fn sign_android(pairs: &[(String, String)], data: &str) -> String {
    let mut kv: Vec<(String, String)> = pairs.to_vec();
    kv.sort_by(|a, b| a.0.cmp(&b.0)); // sort_by 稳定：重复 key 保持插入顺序
    let s: String = kv.iter().map(|(k, v)| format!("{k}={v}")).collect();
    md5_hex(&format!("{SALT_ANDROID}{s}{data}{SALT_ANDROID}"))
}

/// Web 端签名（登录接口）：盐 + 排序 "k=v" 拼接 + 盐
fn sign_web(pairs: &[(String, String)]) -> String {
    let mut kv: Vec<(String, String)> = pairs.to_vec();
    kv.sort_by(|a, b| a.0.cmp(&b.0));
    let s: String = kv.iter().map(|(k, v)| format!("{k}={v}")).collect();
    md5_hex(&format!("{SALT_WEB}{s}{SALT_WEB}"))
}

/// 网关请求（gateway.kugou.com）：自动注入设备参数并计算 signature。
/// `router` 为 x-router 目标服务；`body` 非 None 时发 POST JSON；
/// `extra_headers` 为附加协议头（如 kmr 接口的 kg-tid）。
fn gateway(
    path: &str,
    router: &str,
    extra: &[(String, String)],
    body: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    gateway_ex(path, router, extra, body, &[])
}

fn gateway_ex(
    path: &str,
    router: &str,
    extra: &[(String, String)],
    body: Option<&serde_json::Value>,
    extra_headers: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    let dfid = dfid_of();
    let clienttime = now_secs().to_string();
    let mut pairs: Vec<(String, String)> = vec![
        ("dfid".into(), dfid.clone()),
        ("mid".into(), mid_of()),
        ("uuid".into(), "-".into()),
        ("appid".into(), APPID.into()),
        ("clientver".into(), CLIENTVER.into()),
        ("clienttime".into(), clienttime.clone()),
    ];
    // extra 按名覆盖默认参数（与参考实现的 Object.assign 语义一致）：
    // /v5/url 的取链 clientver=11430 必须覆盖设备默认 20489，否则出现
    // 重复参数，服务端直接报参数错误
    for (k, v) in extra.iter() {
        match pairs.iter_mut().find(|(ek, _)| ek == k) {
            Some(slot) => slot.1 = v.clone(),
            None => pairs.push((k.clone(), v.clone())),
        }
    }
    let data = body.map(|b| serde_json::to_string(b).unwrap_or_default()).unwrap_or_default();
    pairs.push(("signature".into(), sign_android(&pairs, &data)));

    let query = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", form_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let url = format!("https://gateway.kugou.com{path}?{query}");

    let mut req = match body {
        Some(_) => ureq::post(&url),
        None => ureq::get(&url),
    }
    .set("User-Agent", ANDROID_UA)
    .set("dfid", &dfid)
    .set("clienttime", &clienttime)
    .set("mid", &mid_of())
    .set("kg-rc", "1")
    .set("kg-thash", "5d816a0")
    .set("kg-rec", "1")
    .set("kg-rf", "B9EDA08A64250DEFFBCADDEE00F8F25F")
    .timeout(TIMEOUT);
    if !router.is_empty() {
        req = req.set("x-router", router);
    }
    for (k, v) in extra_headers {
        req = req.set(k, v);
    }
    let resp = if let Some(b) = body {
        req.set("Content-Type", "application/json")
            .send_string(&serde_json::to_string(b).unwrap_or_default())
    } else {
        req.call()
    };
    let text = match resp {
        Ok(r) => read_body(r)?,
        Err(ureq::Error::Status(_, r)) => read_body(r)?,
        Err(e) => return Err(format!("酷狗网关请求失败: {e}")),
    };
    let v = parse_json(&text, "酷狗网关响应")?;
    if v.get("status").and_then(|s| s.as_i64()) == Some(0) {
        let msg = v
            .get("error_msg")
            .or_else(|| v.get("errmsg"))
            .or_else(|| v.get("message"))
            .or_else(|| v.get("error"))
            .and_then(|m| m.as_str())
            .unwrap_or("");
        let code = v
            .get("error_code")
            .and_then(|c| c.as_i64())
            .unwrap_or(0);
        return Err(format!("酷狗接口请求失败（{code} {msg}）"));
    }
    Ok(v)
}

// ---------- 数据模型 ----------

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KgSong {
    /// 歌曲 hash（酷狗的曲目唯一标识，128k 档）
    pub id: String,
    pub name: String,
    pub singer: String,
    pub album: String,
    pub duration_ms: u64,
    pub cover: String,
    /// 付费/VIP 曲目：匿名不可播，界面上打 VIP 角标
    pub vip: bool,
    /// 专辑音频 ID（取链接 / 歌词检索要用）
    #[serde(default)]
    pub album_audio_id: u64,
    #[serde(default)]
    pub album_id: u64,
    /// 高音质文件 hash（搜索/榜单通道附带；匿名取不到链接，登录后可用）
    #[serde(default)]
    pub hq_hash: String,
    #[serde(default)]
    pub sq_hash: String,
    #[serde(default)]
    pub super_hash: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KgToplist {
    pub id: i64,
    pub name: String,
    pub pic: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KgPublicPlaylist {
    /// 歌单 ID（global_collection_id，字符串）
    pub id: String,
    pub name: String,
    pub cover: String,
    pub play_count: i64,
    pub creator: String,
    pub songs: Vec<KgSong>,
    /// 服务端声称的曲目总数（0 = 未知；详情拉取后与 songs.len() 对照可发现截断）
    #[serde(default)]
    pub total_count: i64,
}

/// 数值字段兼容解析（酷狗各接口数字/字符串混用）
fn json_num(v: Option<&serde_json::Value>) -> u64 {
    match v {
        Some(serde_json::Value::Number(n)) => n.as_u64().unwrap_or(0),
        Some(serde_json::Value::String(s)) => s.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

fn cover_sized(url: &str, size: u32) -> String {
    if url.is_empty() {
        String::new()
    } else {
        url.replace("{size}", &size.to_string())
    }
}

// ---------- 搜索 ----------

/// 搜索（每页 30 条）。主通道为老 mobilecdn 明文接口——**songsearch_v2
/// 匿名请求会把 VIP 曲目整体过滤掉**（实测 pay_type==3 的歌不出现在结果
/// 里），必须以它为主才能展示 VIP 曲目；songsearch_v2 仅按 hash 做元数据
/// 增强（高音质文件 hash、更全的封面），失败不影响搜索结果。
pub fn search(keyword: &str, page: i64) -> Result<Vec<KgSong>, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(vec![]);
    }
    let mut songs = search_fallback(keyword, page)?;
    enrich_from_songsearch(&mut songs, keyword, page);
    Ok(songs)
}

/// 存档行（最近播放/收藏/歌单）只落库了 128 hash：按"标题+歌手"反查补齐
/// 各档，已有值保持不变（engine/commands 的 HQ/无损取链前置步骤）。
/// artist 为空时仅用标题——同名翻唱可能导致匹配不到原曲，调用方应尽量传入
pub fn enrich_hashes(
    hash: &str,
    title: &str,
    artist: &str,
    hq_hash: &str,
    sq_hash: &str,
    super_hash: &str,
) -> (String, String, String) {
    if !sq_hash.is_empty() && !hq_hash.is_empty() {
        return (hq_hash.to_string(), sq_hash.to_string(), super_hash.to_string());
    }
    let keyword = if artist.trim().is_empty() {
        title.to_string()
    } else {
        format!("{title} {artist}")
    };
    match quality_hashes_by_search(hash, &keyword) {
        Some((h, s, sup)) => (
            if hq_hash.is_empty() { h } else { hq_hash.to_string() },
            if sq_hash.is_empty() { s } else { sq_hash.to_string() },
            if super_hash.is_empty() { sup } else { super_hash.to_string() },
        ),
        None => (hq_hash.to_string(), sq_hash.to_string(), super_hash.to_string()),
    }
}

/// 按 128 hash 反查各档质量 hash：最近播放/收藏/歌单等存档行只落库了
/// 128 hash，播放/下载时补齐，音质设置（HQ/无损）才能生效。
/// keyword 必须含歌手名（只按标题搜会命中同名翻唱、匹配不到原曲行）。
/// 主通道用 v3 搜索（结果自带 320hash/sqhash，且与存档 hash 同源），
/// 按 FileHash 精确匹配；未命中再试 song_search_v2。
pub fn quality_hashes_by_search(hash: &str, keyword: &str) -> Option<(String, String, String)> {
    if hash.is_empty() || keyword.trim().is_empty() {
        return None;
    }
    if let Ok(songs) = search_fallback(keyword, 1) {
        if let Some(s) = songs.iter().find(|s| s.id.eq_ignore_ascii_case(hash)) {
            if !s.hq_hash.is_empty() || !s.sq_hash.is_empty() {
                return Some((s.hq_hash.clone(), s.sq_hash.clone(), s.super_hash.clone()));
            }
        }
    }
    let url = format!(
        "https://songsearch.kugou.com/song_search_v2?keyword={}&page=1&pagesize=30",
        form_encode(keyword)
    );
    let v = get_json(&url, "https://www.kugou.com/", UA, "酷狗搜索增强").ok()?;
    let list = v.pointer("/data/lists").and_then(|x| x.as_array())?;
    let t = list.iter().find(|t| {
        t.get("FileHash")
            .and_then(|h| h.as_str())
            .map(|h| h.eq_ignore_ascii_case(hash))
            .unwrap_or(false)
    })?;
    let g = |k: &str| {
        t.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_lowercase()
    };
    Some((g("HQFileHash"), g("SQFileHash"), g("SuperFileHash")))
}

/// 用 songsearch_v2 的结果按 hash 补充多音质 hash 与缺失封面
fn enrich_from_songsearch(songs: &mut [KgSong], keyword: &str, page: i64) {
    let url = format!(
        "https://songsearch.kugou.com/song_search_v2?keyword={}&page={page}&pagesize=30",
        form_encode(keyword)
    );
    let Ok(v) = get_json(&url, "https://www.kugou.com/", UA, "酷狗搜索增强") else {
        return;
    };
    let Some(list) = v.pointer("/data/lists").and_then(|x| x.as_array()) else {
        return;
    };
    let by_hash: std::collections::HashMap<String, &serde_json::Value> = list
        .iter()
        .filter_map(|t| {
            t.get("FileHash")
                .and_then(|h| h.as_str())
                .map(|h| (h.to_lowercase(), t))
        })
        .collect();
    for s in songs.iter_mut() {
        let Some(t) = by_hash.get(&s.id.to_lowercase()) else {
            continue;
        };
        let get_str = |k: &str| -> String {
            t.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_lowercase()
        };
        if s.hq_hash.is_empty() {
            s.hq_hash = get_str("HQFileHash");
        }
        if s.sq_hash.is_empty() {
            s.sq_hash = get_str("SQFileHash");
        }
        if s.super_hash.is_empty() {
            s.super_hash = get_str("SuperFileHash");
        }
        if s.cover.is_empty() {
            s.cover = cover_sized(t.get("Image").and_then(|x| x.as_str()).unwrap_or(""), 240);
        }
    }
}

/// 备用搜索：mobilecdn v3 明文接口（无多音质 hash）
fn search_fallback(keyword: &str, page: i64) -> Result<Vec<KgSong>, String> {
    let url = format!(
        "http://mobilecdn.kugou.com/api/v3/search/song?format=json&keyword={}&page={page}&pagesize=30",
        form_encode(keyword)
    );
    let v = get_json(&url, "http://mobilecdn.kugou.com/", UA, "酷狗搜索")?;
    if v.get("status").and_then(|s| s.as_i64()) != Some(1) {
        return Err("酷狗搜索失败".into());
    }
    let mut out = Vec::new();
    if let Some(list) = v.pointer("/data/info").and_then(|x| x.as_array()) {
        for t in list {
            let hash = t.get("hash").and_then(|v| v.as_str()).unwrap_or("");
            let name = t
                .get("songname")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if hash.is_empty() || name.is_empty() {
                continue;
            }
            out.push(KgSong {
                id: hash.to_string(),
                name,
                singer: t
                    .get("singername")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                album: t
                    .get("album_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                duration_ms: t.get("duration").and_then(|v| v.as_i64()).unwrap_or(0).max(0) as u64
                    * 1000,
                cover: cover_sized(
                    t.pointer("/trans_param/union_cover")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    240,
                ),
                vip: t.get("pay_type").and_then(|v| v.as_i64()).unwrap_or(0) == 3,
                album_audio_id: json_num(t.get("album_audio_id")),
                album_id: json_num(t.get("album_id")),
                // v3 搜索自带各档 hash（320hash/sqhash/viphash），登录后按音质取链必需
                hq_hash: t.get("320hash").and_then(|v| v.as_str()).unwrap_or("").to_lowercase(),
                sq_hash: t.get("sqhash").and_then(|v| v.as_str()).unwrap_or("").to_lowercase(),
                super_hash: t.get("viphash").and_then(|v| v.as_str()).unwrap_or("").to_lowercase(),
            });
        }
    }
    Ok(out)
}

// ---------- 播放直链 ----------

/// 匿名播放信息（免费曲目 128k 直链），返回 (url, ext)
fn song_url_free(hash: &str, vip: bool) -> Result<(String, String), String> {
    let url = format!(
        "http://m.kugou.com/app/i/getSongInfo.php?cmd=playInfo&hash={}",
        form_encode(hash)
    );
    let v = get_json(&url, "http://m.kugou.com/", UA, "酷狗歌曲信息")?;
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
fn url_ext(url: &str) -> String {
    let path = url.split('?').next().unwrap_or("");
    path.rsplit('.')
        .next()
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("mp3")
        .to_lowercase()
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
    let ladder: Vec<(&str, &str, &str)> = match quality {
        "standard" => vec![("128", hash, "标准")],
        "lossless" => {
            let mut l = Vec::new();
            if !super_hash.is_empty() {
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
        _ => {
            let mut l = Vec::new();
            if !hq_hash.is_empty() {
                l.push(("320", hq_hash, "HQ"));
            }
            l.push(("128", hash, "标准"));
            l
        }
    };
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
            ("key".into(), md5_hex(&format!("{h}{SALT_URL_KEY}{APPID}{mid}{userid}"))),
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
            "super" => "Hi-Res",
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
                serde_json::to_string(&v).unwrap_or_default().chars().take(300).collect::<String>()
            ))
        }
    };

    let mut last_err = String::from("无可用链接");
    for (q, h, _) in ladder.iter() {
        if h.is_empty() {
            continue;
        }
        match fetch(q, h) {
            Ok(ok) => return Ok(ok),
            Err(e) => {
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
    if !token.is_empty() && !userid.is_empty() {
        if let Ok((url, ext, label)) = song_url_v5(
            token, userid, album_audio_id, album_id, hash, hq_hash, sq_hash, super_hash, quality,
        ) {
            return Ok((url, ext, label.to_string()));
        }
        // 登录通道失败（风控/会话失效/权益不足）：继续走匿名通道兜底，
        // 免费曲目仍可 128k 播放；VIP 曲目会在此得到明确错误
    }
    let (url, ext) = song_url_free(hash, vip)?;
    Ok((url, ext, "标准".into()))
}

// ---------- 歌词（KRC 逐字 → 增强 LRC；回落 LRC） ----------

/// 解密 KRC：base64 → 去掉 4 字节文件头 → 16 字节循环 XOR → zlib 解压
fn decrypt_krc(content_b64: &str) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content_b64.trim())
        .map_err(|e| format!("KRC base64 解码失败: {e}"))?;
    if bytes.len() <= 4 {
        return Err("KRC 内容为空".into());
    }
    const KEY: [u8; 16] = [
        64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105,
    ];
    let body = &bytes[4..];
    let xored: Vec<u8> = body
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ KEY[i % KEY.len()])
        .collect();
    let mut out = String::new();
    ZlibDecoder::new(xored.as_slice())
        .read_to_string(&mut out)
        .map_err(|e| format!("KRC 解压失败: {e}"))?;
    Ok(out)
}

/// 歌词：krcs 检索 → 优先下载 KRC 逐字（解密转增强 LRC），失败回落
/// LRC（content 为 base64）。无歌词返回 Ok(None)
pub fn lyric(hash: &str) -> Result<Option<String>, String> {
    let search_url = format!(
        "http://krcs.kugou.com/search?ver=1&man=yes&client=mobi&keyword=&duration=&hash={}",
        form_encode(hash)
    );
    let text = get_text(&search_url, "http://krcs.kugou.com/", UA)?;
    let v = parse_json(&text, "酷狗歌词检索")?;
    let cand = v
        .pointer("/candidates/0")
        .cloned()
        .ok_or_else(|| "no candidates".to_string());
    let (id, accesskey) = match cand {
        Ok(c) => (
            c.get("id")
                .and_then(|x| {
                    x.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| x.as_i64().map(|n| n.to_string()))
                })
                .unwrap_or_default(),
            c.get("accesskey")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        ),
        Err(_) => return Ok(None),
    };
    if id.is_empty() {
        return Ok(None);
    }

    // KRC 逐字：解密/解析失败静默回落 LRC
    let krc_url = format!(
        "http://krcs.kugou.com/download?ver=1&client=mobi&fmt=krc&charset=utf8&id={id}&accesskey={accesskey}"
    );
    if let Ok(text) = get_text(&krc_url, "http://krcs.kugou.com/", UA) {
        if let Ok(v) = parse_json(&text, "酷狗 KRC 下载") {
            if let Some(content) = v.get("content").and_then(|c| c.as_str()).filter(|s| !s.is_empty()) {
                if let Ok(krc) = decrypt_krc(content) {
                    let trimmed = krc.trim_start_matches('\u{feff}').trim().to_string();
                    if let Some(enhanced) = crate::lyrics::krc_to_enhanced_lrc(&trimmed) {
                        return Ok(Some(enhanced));
                    }
                }
            }
        }
    }

    // LRC 行级
    let dl_url = format!(
        "http://krcs.kugou.com/download?ver=1&client=pc&fmt=lrc&charset=utf8&id={id}&accesskey={accesskey}"
    );
    let text = get_text(&dl_url, "http://krcs.kugou.com/", UA)?;
    let v = parse_json(&text, "酷狗歌词下载")?;
    let content = v
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    if content.is_empty() {
        return Ok(None);
    }
    // content 为 base64（UTF-8 文本，可能带 BOM）；解不开时退回原文
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(content.as_bytes())
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or(content);
    let trimmed = decoded.trim_start_matches('\u{feff}').trim().to_string();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

// ---------- 排行榜（匿名） ----------

/// 官方榜单列表（TOP500 / 飙升榜 / 热歌榜等，老明文接口免签名）
pub fn toplists() -> Result<Vec<KgToplist>, String> {
    let v = get_json(
        "http://mobilecdn.kugou.com/api/v3/rank/list?format=json",
        "http://mobilecdn.kugou.com/",
        UA,
        "酷狗榜单列表",
    )?;
    let mut out = Vec::new();
    if let Some(list) = v.pointer("/data/info").and_then(|x| x.as_array()) {
        for t in list {
            let id = t.get("rankid").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = t
                .get("rankname")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id == 0 || name.is_empty() {
                continue;
            }
            let pic = cover_sized(
                t.get("imgurl")
                    .or_else(|| t.get("img_9"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
                240,
            );
            out.push(KgToplist { id, name, pic });
        }
    }
    Ok(out)
}

/// 榜单曲目（网关 kmr 接口，匿名可调）。内部翻页拉全：榜单实际 100~500 首
/// （TOP500 等），单页 30 只能给个零头；每页 90，封顶 20 页防异常死循环
pub fn toplist_tracks(rankid: i64, page: i64) -> Result<Vec<KgSong>, String> {
    let pagesize = 90i64;
    let body = serde_json::json!({
        "show_portrait_mv": 1,
        "show_type_total": 1,
        "filter_original_remarks": 1,
        "area_code": 1,
        "pagesize": pagesize,
        "rank_cid": 0,
        "type": 1,
        "page": page.max(1),
        "rank_id": rankid,
    });
    let v = gateway_ex(
        "/openapi/kmr/v2/rank/audio",
        "",
        &[],
        Some(&body),
        &[("kg-tid", "369")],
    )
    .map_err(|e| format!("获取榜单歌曲失败: {e}"))?;
    let list = v
        .pointer("/data/songlist")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut songs: Vec<KgSong> = list.iter().filter_map(song_from_kmr).collect();
    enrich_vip(&mut songs);
    Ok(songs)
}

/// kmr 榜单条目 → KgSong（audio_info 里带各档 hash）
fn song_from_kmr(t: &serde_json::Value) -> Option<KgSong> {
    let name = t
        .get("songname")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() {
        return None;
    }
    let hash = t
        .pointer("/audio_info/hash_128")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    if hash.is_empty() {
        return None;
    }
    let info = |k: &str| -> String {
        t.pointer(&format!("/audio_info/{k}"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase()
    };
    let album_id = t.get("album_id").and_then(|v| v.as_u64()).unwrap_or(0);
    let cover = cover_sized(
        t.pointer("/album_info/sizable_cover")
            .or_else(|| t.pointer("/trans_param/union_cover"))
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        240,
    );
    Some(KgSong {
        id: hash,
        name,
        singer: t
            .get("author_name")
            .or_else(|| t.pointer("/authors/0/author_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        album: t
            .pointer("/album_info/album_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        // duration_128 单位是毫秒（实测 403000 → 6:43），勿再换算；
        // 搜索接口的 duration 才是秒（见 search_fallback 的 *1000）
        duration_ms: t
            .pointer("/audio_info/duration_128")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            .max(0) as u64,
        cover,
        vip: false,
        album_audio_id: t.get("album_audio_id").and_then(|v| v.as_u64()).unwrap_or(0),
        album_id,
        hq_hash: info("hash_320"),
        sq_hash: info("hash_flac"),
        super_hash: info("hash_super"),
    })
}

// ---------- 歌单广场 / 歌单详情（匿名） ----------

/// 歌单推荐页（specialrec 服务）：返回 (分类下歌单, 是否还有更多)
fn special_recommend(category_id: i64, page: i64) -> Result<Vec<KgPublicPlaylist>, String> {
    let clienttime = now_secs().to_string();
    // key = MD5(appid + 盐 + clientver + clienttime)
    let key = md5_hex(&format!("{APPID}{SALT_ANDROID}{CLIENTVER}{clienttime}"));
    let body = serde_json::json!({
        "appid": APPID.parse::<i64>().unwrap_or(1005),
        "mid": mid_of(),
        "clientver": CLIENTVER.parse::<i64>().unwrap_or(20489),
        "platform": "android",
        "clienttime": clienttime,
        "userid": 0,
        "module_id": 1,
        "page": page.max(1),
        "pagesize": 30,
        "key": key,
        "special_recommend": {
            "withtag": 1,
            "withsong": 1,
            "sort": 1,
            "ugc": 1,
            "is_selected": 0,
            "withrecommend": 1,
            "area_code": 1,
            "categoryid": category_id,
        },
        "req_multi": 1,
        "retrun_min": 5,
        "return_special_falg": 1,
    });
    let v = gateway("/v2/special_recommend", "specialrec.service.kugou.com", &[], Some(&body))
        .map_err(|e| format!("获取歌单推荐失败: {e}"))?;
    let mut out = Vec::new();
    if let Some(list) = v.pointer("/data/special_list").and_then(|x| x.as_array()) {
        for p in list {
            let id = p
                .get("global_collection_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = p
                .get("specialname")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() || name.is_empty() {
                continue;
            }
            out.push(KgPublicPlaylist {
                id,
                name,
                cover: cover_sized(
                    p.get("imgurl")
                        .or_else(|| p.get("flexible_cover"))
                        .or_else(|| p.get("pic"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                    480,
                ),
                play_count: p.get("play_count").and_then(|v| v.as_i64()).unwrap_or(0),
                creator: p
                    .get("nickname")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                songs: vec![],
                // 服务端声称的曲目总数（诊断翻页完整性/详情页展示用）
                total_count: p.get("count").and_then(|v| v.as_i64()).unwrap_or(0),
            });
        }
    }
    Ok(out)
}

/// 歌单详情（global_collection_id 拉全曲目，每页 90 条翻页）
pub fn playlist_tracks(gcid: &str) -> Result<KgPublicPlaylist, String> {
    let mut name = String::new();
    let mut cover = String::new();
    let mut creator = String::new();
    let mut play_count = 0i64;
    let mut songs = Vec::new();
    let pagesize = 90i64;
    for page in 1..=20i64 {
        let extra = vec![
            ("area_code".into(), "1".into()),
            ("begin_idx".into(), ((page - 1) * pagesize).to_string()),
            ("plat".into(), "1".into()),
            ("type".into(), "1".into()),
            ("mode".into(), "1".into()),
            ("personal_switch".into(), "1".into()),
            (
                "extend_fields".into(),
                "abtags,hot_cmt,popularization".into(),
            ),
            ("pagesize".into(), pagesize.to_string()),
            ("global_collection_id".into(), gcid.to_string()),
        ];
        let v = gateway("/pubsongs/v2/get_other_list_file_nofilt", "", &extra, None)
            .map_err(|e| format!("获取歌单详情失败: {e}"))?;
        if page == 1 {
            if let Some(info) = v.pointer("/data/list_info") {
                name = info
                    .get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                cover = cover_sized(info.get("pic").and_then(|x| x.as_str()).unwrap_or(""), 480);
                creator = info
                    .get("list_create_username")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            play_count = 0; // 该接口不返回播放量，置 0（UI 不展示）
        }
        let list = v
            .pointer("/data/songs")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        let got = list.len() as i64;
        for t in &list {
            if let Some(s) = song_from_playlist(t) {
                songs.push(s);
            }
        }
        if got < pagesize {
            break;
        }
    }
    if name.is_empty() && songs.is_empty() {
        return Err("歌单不存在或为空".into());
    }
    enrich_vip(&mut songs);
    Ok(KgPublicPlaylist {
        id: gcid.to_string(),
        name,
        cover,
        play_count,
        creator,
        songs,
        total_count: 0,
    })
}

/// 批量权益查询（media.store privilege_lite，匿名可调）：返回按入参顺序的
/// (hash→pay_type, album_id, album_audio_id) 映射。pay_type==3 即 VIP/付费。
fn privilege_map(
    items: &[(String, u64)],
) -> Result<std::collections::HashMap<String, (bool, u64, u64)>, String> {
    let mut out = std::collections::HashMap::new();
    for batch in items.chunks(30) {
        let body = serde_json::json!({
            "appid": APPID.parse::<i64>().unwrap_or(1005),
            "area_code": 1,
            "behavior": "play",
            "clientver": CLIENTVER.parse::<i64>().unwrap_or(20489),
            "need_hash_offset": 1,
            "relate": 1,
            "support_verify": 1,
            "resource": batch
                .iter()
                .map(|(h, aid)| serde_json::json!({
                    "type": "audio", "page_id": 0,
                    "hash": h.to_uppercase(),
                    "album_id": aid,
                }))
                .collect::<Vec<_>>(),
            "qualities": ["128", "320", "flac", "high", "super"],
        });
        let v = gateway("/v2/get_res_privilege/lite", "media.store.kugou.com", &[], Some(&body))
            .map_err(|e| format!("权益查询失败: {e}"))?;
        // 响应结构：{data: [ {...,hash, pay_type, album_audio_id}, ... ]}
        let list = v
            .pointer("/data")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        for it in list {
            let hash = it
                .get("hash")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_lowercase();
            if hash.is_empty() {
                continue;
            }
            let pay_type = it.get("pay_type").and_then(|x| x.as_i64()).unwrap_or(0);
            let album_id = it
                .get("album_id")
                .and_then(|x| {
                    x.as_str()
                        .map(|s| s.parse::<u64>().unwrap_or(0))
                        .or_else(|| x.as_u64())
                })
                .unwrap_or(0);
            let album_audio_id = it.get("album_audio_id").and_then(|x| x.as_u64()).unwrap_or(0);
            out.insert(hash, (pay_type == 3, album_id, album_audio_id));
        }
    }
    Ok(out)
}

/// 用权益数据回填歌单曲目的 VIP 标志与专辑 ID（歌单通道不带付费信息）
fn enrich_vip(songs: &mut [KgSong]) {
    let items: Vec<(String, u64)> = songs
        .iter()
        .map(|s| (s.id.to_lowercase(), s.album_id))
        .collect();
    let result = privilege_map(&items);
    if let Err(e) = &result {
        eprintln!("[kugou] enrich_vip 失败: {e}");
    }
    if let Ok(map) = result {
        for s in songs.iter_mut() {
            if let Some((vip, album_id, album_audio_id)) = map.get(&s.id.to_lowercase()) {
                s.vip = *vip;
                if s.album_id == 0 {
                    s.album_id = *album_id;
                }
                if s.album_audio_id == 0 {
                    s.album_audio_id = *album_audio_id;
                }
            }
        }
    }
}

/// 歌单条目 → KgSong
fn song_from_playlist(t: &serde_json::Value) -> Option<KgSong> {
    let hash = t.get("hash").and_then(|v| v.as_str()).unwrap_or("");
    let name = t
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if hash.is_empty() || name.is_empty() {
        return None;
    }
    let singer = t
        .pointer("/singerinfo")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    a.get("author_name")
                        .or_else(|| a.get("name"))
                        .and_then(|n| n.as_str())
                })
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .unwrap_or_default();
    // 歌单通道的 name 常是 "歌手 - 歌名" 全格式，剥掉歌手前缀避免 UI 重复
    let name = if !singer.is_empty() && name.starts_with(&format!("{singer} - ")) {
        name[singer.len() + 3..].trim().to_string()
    } else {
        name
    };
    Some(KgSong {
        id: hash.to_string(),
        name,
        singer,
        album: t
            .pointer("/albuminfo/name")
            .or_else(|| t.pointer("/albuminfo/album_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        duration_ms: t.get("timelen").and_then(|v| v.as_i64()).unwrap_or(0).max(0) as u64,
        cover: cover_sized(t.get("cover").and_then(|v| v.as_str()).unwrap_or(""), 240),
        // 歌单通道的 privilege 对全部歌曲恒为 10，无法判定 VIP，置 false
        //（播放失败时错误信息已按权益明确分类）
        vip: false,
        album_audio_id: t.get("audio_id").and_then(|v| v.as_u64()).unwrap_or(0),
        album_id: t.get("album_id").and_then(|v| v.as_u64()).unwrap_or(0),
        hq_hash: String::new(),
        sq_hash: String::new(),
        super_hash: String::new(),
    })
}

/// 随机歌单（随便听听）：推荐分类下随机抽一个歌单并拉取曲目，
/// 空歌单/拉取失败自动换一个重试
pub fn random_playlist() -> Result<KgPublicPlaylist, String> {
    use rand::seq::SliceRandom;
    let specials = special_recommend(0, 1)?;
    if specials.is_empty() {
        return Err("歌单广场暂无数据".into());
    }
    let mut last_err = String::from("未能抽中可用歌单");
    for _ in 0..4 {
        let pick = specials.choose(&mut rand::thread_rng()).unwrap();
        match playlist_tracks(&pick.id) {
            Ok(mut pl) => {
                if pl.name.is_empty() {
                    pl.name = pick.name.clone();
                }
                if pl.cover.is_empty() {
                    pl.cover = pick.cover.clone();
                }
                if pl.creator.is_empty() {
                    pl.creator = pick.creator.clone();
                }
                if !pl.songs.is_empty() {
                    return Ok(pl);
                }
                last_err = "抽中的歌单暂无曲目".into();
            }
            Err(e) => last_err = e,
        }
    }
    Err(format!("随机歌单获取失败：{last_err}"))
}

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
    let status_code = v.pointer("/data/status").and_then(|s| s.as_i64()).unwrap_or(1);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_vectors() {
        // 与参考实现（MakcRe/KuGouMusicApi helper.js）交叉验证的向量
        let pairs = vec![
            ("appid".to_string(), "1005".to_string()),
            ("clienttime".to_string(), "1790000000".to_string()),
            ("clientver".to_string(), "20489".to_string()),
            ("dfid".to_string(), "abc123".to_string()),
            ("mid".to_string(), "12345".to_string()),
            ("uuid".to_string(), "-".to_string()),
        ];
        assert_eq!(
            sign_android(&pairs, ""),
            "58cfefe77313a1706ffadca4f3b4ff06"
        );
        let pairs2 = vec![
            ("appid".to_string(), "1005".to_string()),
            ("clientver".to_string(), "20489".to_string()),
        ];
        assert_eq!(
            sign_android(&pairs2, "{\"a\":1}"),
            "345153e763985ede8f4eb4369b6f9b0c"
        );
        let pairs3 = vec![
            ("appid".to_string(), "1001".to_string()),
            ("plat".to_string(), "4".to_string()),
            ("srcappid".to_string(), "2919".to_string()),
            ("clienttime".to_string(), "1790000000".to_string()),
        ];
        assert_eq!(sign_web(&pairs3), "62607c708107bad91f0b9249d0a9b87d");
    }

    #[test]
    fn mid_from_guid() {
        // guid=550e8400-e29b-41d4-a716-446655440000 → md5 e8456cee... → 十进制
        let guid_md5 = md5_hex("550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(guid_md5, "e8456cee7fd2348cfa239342b10a23bd");
        let mid = BigUint::parse_bytes(guid_md5.as_bytes(), 16)
            .map(|n| n.to_string())
            .unwrap();
        assert_eq!(mid, "308741372901437977228425563242293240765");
    }

    #[test]
    fn url_ext_cases() {
        assert_eq!(url_ext("https://a.b/x/file.mp3?k=v"), "mp3");
        assert_eq!(url_ext("https://a.b/x/file"), "mp3");
        assert_eq!(url_ext("https://a.b/x/file.flac"), "flac");
    }

    #[test]
    fn test_search_and_lyric() {
        let songs = search("晴天", 1).unwrap();
        assert!(!songs.is_empty());
        assert!(!songs[0].id.is_empty());
        // 付费曲目报“需要 VIP/付费”，免费曲目拿到直链
        let free = songs.iter().find(|s| !s.vip).expect("有免费曲目");
        let (url, _ext, _label) =
            song_url(&free.id, free.album_audio_id, free.album_id, &free.hq_hash, &free.sq_hash, &free.super_hash, false, "", "", "high").unwrap();
        assert!(url.starts_with("http"));
        let lyric = lyric(&free.id).unwrap();
        let _ = lyric; // 有无歌词均可（有词时应包含时间标签）
    }

    #[test]
    #[ignore] // 需要网络
    fn test_krc_word_lyrics() {
        // 热门歌曲应有 KRC 逐字歌词：歌词接口返回增强 LRC，解析出词级时间轴
        let songs = search("晴天", 1).unwrap();
        let free = songs.iter().find(|s| !s.vip).expect("有免费曲目");
        let lrc = lyric(&free.id).unwrap().expect("有歌词");
        let p = crate::lyrics::parse(&lrc);
        assert!(p.synced);
        assert!(
            p.lines.iter().any(|l| l.words.is_some()),
            "应为 KRC 逐字歌词（增强 LRC 带词级时间轴）"
        );
        // 词时间必须是绝对毫秒（词首 ≥ 行首）：线上 KRC 词时间是相对行首的
        // 偏移，转换器漏加行首的话词首全落在 0 附近，前端逐词染色失效
        for l in p.lines.iter() {
            let Some(words) = l.words.as_ref() else { continue };
            let Some(first) = words.iter().find(|w| !w.text.is_empty()) else { continue };
            assert!(
                first.start_ms + 100 >= l.time_ms.unwrap_or(0),
                "词时间应为绝对毫秒：行首 {:?}，词首 {}（{}）",
                l.time_ms,
                first.start_ms,
                l.text
            );
        }
    }


    #[test]
    #[ignore] // 需要网络 + 本机应用数据库
    fn test_full_app_path_with_real_account() {
        // 直接复现应用路径：读本机库里的登录凭证 → 注册/注入 dfid →
        // 搜索 → 挑 VIP 曲目 → 按当前音质取链接
        let db_path = std::env::var("APPDATA").unwrap()
            + r"\com.rustmusic.app\library.db";
        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("打开应用数据库失败");
        let get = |k: &str| -> String {
            conn.query_row("SELECT value FROM settings WHERE key = ?1", [k], |r| {
                r.get::<_, String>(0)
            })
            .unwrap_or_default()
        };
        let token = get("kg_token");
        let userid = get("kg_userid");
        let dfid = get("kg_dfid");
        println!("userid={userid} token_len={} dfid={dfid}", token.len());
        assert!(!token.is_empty(), "本机应有酷狗登录凭证（先扫码登录）");
        set_account(&token, &userid);
        let (stored_mid, stored_dfid) = {
            let m = conn
                .query_row("SELECT value FROM settings WHERE key = 'kg_mid'", [], |r| {
                    r.get::<_, String>(0)
                })
                .unwrap_or_default();
            (m, dfid)
        };
        if stored_mid.is_empty() || stored_dfid.is_empty() {
            set_device("", "");
            let d = register_dev().expect("register_dev failed");
            println!("new mid={} dfid={d}", current_mid());
        } else {
            set_device(&stored_mid, &stored_dfid);
            println!("injected mid={stored_mid} dfid={stored_dfid}");
        }

        let songs = search("七里香", 1).expect("search failed");
        let vip_list: Vec<_> = songs.iter().filter(|s| s.vip).collect();
        println!("共 {} 首，VIP {} 首", songs.len(), vip_list.len());
        assert!(!vip_list.is_empty(), "搜索结果应含 VIP 曲目");

        let v = &vip_list[0];
        println!("试播 VIP: {} - {} (album_audio_id={})", v.singer, v.name, v.album_audio_id);
        let (url, ext, label) = song_url(
            &v.id, v.album_audio_id, v.album_id, &v.hq_hash, &v.sq_hash,
            &v.super_hash, v.vip, &token, &userid, "lossless",
        )
        .expect("VIP 曲目取链接失败");
        println!("✓ {label} .{ext} {}...", &url[..url.len().min(70)]);
    }

    #[test]
    #[ignore] // 需要网络
    fn test_lyric_debug() {
        // 七里香（VIP 正版）的歌词：应返回 KRC 转换的增强 LRC
        let r = lyric("9218d81686c6ac681bc7ca621958ad6d");
        match r {
            Ok(Some(text)) => println!("lyric ok, head: {}", text.chars().take(200).collect::<String>()),
            Ok(None) => println!("lyric: None（无歌词）"),
            Err(e) => println!("lyric error: {e}"),
        }
    }

    #[test]
    #[ignore] // 需要网络
    fn test_playlist_vip_badges() {
        // 用户 Jay Chou 歌单：周杰伦正版歌应标 VIP（pay_type==3 回填）
        let pl = playlist_tracks("collection_3_560871299_3_0").expect("playlist failed");
        let vip_count = pl.songs.iter().filter(|s| s.vip).count();
        println!(
            "歌单 {} 首，VIP {} 首，第一首 vip={} album_audio_id={}",
            pl.songs.len(),
            vip_count,
            pl.songs.first().map(|s| s.vip).unwrap_or(false),
            pl.songs.first().map(|s| s.album_audio_id).unwrap_or(0)
        );
        assert!(vip_count > 0, "歌单应包含 VIP 曲目");
    }

    #[test]
    #[ignore] // 需要网络
    fn test_toplist_vip_badges() {
        // 榜单曲目也要有 VIP 标签（回归：toplist_tracks 曾漏掉权益回填）
        let songs = toplist_tracks(8888, 1).expect("toplist failed");
        let vip = songs.iter().filter(|s| s.vip).count();
        println!("TOP500 前 30 首：VIP {} 首", vip);
        assert!(vip > 0, "榜单应包含 VIP 曲目");
    }

    #[test]
    #[ignore] // 需要网络
    fn test_register_dev() {
        let dfid = register_dev().expect("register_dev failed");
        assert_eq!(dfid.len(), 24, "dfid 应为 24 位");
        // 二次注册：服务端对同一 mid 返回空 data，应复用缓存 dfid
        let dfid2 = register_dev().expect("re-register failed");
        assert_eq!(dfid2, dfid);
    }

    #[test]
    #[ignore] // 需要网络
    fn test_search_shows_vip() {
        // 匿名搜索也必须能看到 VIP 曲目（回归：songsearch_v2 主通道会
        // 把 pay_type==3 的歌整体过滤掉）
        let songs = search("七里香", 1).unwrap();
        assert!(
            songs.iter().any(|s| s.vip),
            "搜索结果应包含 VIP 曲目"
        );
    }

    #[test]
    #[ignore] // 需要网络，依赖网关可用性
    fn test_toplists_and_random() {
        let tops = toplists().expect("toplists failed");
        assert!(!tops.is_empty());
        let songs = toplist_tracks(tops[0].id, 1).expect("toplist tracks failed");
        assert!(!songs.is_empty());
        let pl = random_playlist().expect("random playlist failed");
        assert!(!pl.songs.is_empty());
    }
}

/// 音质审计：按标题反查各档 hash（存档行只有 128 hash 时的补齐路径）
#[test]
#[ignore] // 需要网络
fn test_quality_hashes_by_search_audit() {
    // 验证 app 真实链路：搜索结果经 enrich_from_songsearch 补齐各档 hash
    let songs = search("突然好想你", 1).expect("search failed");
    let s = songs.first().expect("no songs");
    println!(
        "✓ 搜索链路 hash 补齐: hq={} sq={} super={}",
        s.hq_hash.len(),
        s.sq_hash.len(),
        s.super_hash.len()
    );
    // 反查（存档行补齐用）：可能因同名多版本匹配失败，仅观察
    match quality_hashes_by_search(&s.id, &s.name) {
        Some((h, sq, sup)) => {
            println!("✓ 反查成功: hq={} sq={} super={}", h.len(), sq.len(), sup.len())
        }
        None => println!("反查未命中（同名多版本，属已知局限）"),
    }
}

/// 下载质量审计：完整复现 download_online 对酷狗的逻辑（搜索 → hash 补齐 →
/// v5 按无损取链 → 探测实际文件大小），需要网络 + 本机酷狗登录
#[test]
#[ignore]
fn test_kugou_download_quality_audit() {
    let db_path = std::env::var("APPDATA").unwrap() + r"\com.rustmusic.app\library.db";
    let conn = rusqlite::Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("打开应用数据库失败");
    let get = |k: &str| -> String {
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [k], |r| {
            r.get::<_, String>(0)
        })
        .unwrap_or_default()
    };
    let token = get("kg_token");
    let userid = get("kg_userid");
    println!("token_len={} userid={}", token.len(), userid);

    let songs = search("晴天 周杰伦", 1).expect("search failed");
    let s = songs
        .iter()
        .find(|s| s.name.contains("晴天") && s.singer.contains("周杰伦"))
        .expect("未找到目标歌曲");
    println!(
        "搜索到: {} - {} | 128hash={} hq={} sq={} super={}",
        s.singer,
        s.name,
        &s.id[..8],
        s.hq_hash.len(),
        s.sq_hash.len(),
        s.super_hash.len()
    );
    // 下载路径：存档行无各档 hash，按标题反查补齐
    let (hq, sq, sup) = enrich_hashes(&s.id, &s.name, &s.singer, &s.hq_hash, &s.sq_hash, &s.super_hash);
    println!("补齐后: hq={} sq={} super={}", hq.len(), sq.len(), sup.len());
    // 带真实 album_audio_id（修复后的下载路径）
    let (url, ext, label) = song_url(
        &s.id, s.album_audio_id, s.album_id, &hq, &sq, &sup, s.vip, &token, &userid, "lossless",
    )
    .expect("song_url failed");
    let agent = ureq::AgentBuilder::new().build();
    let size = agent
        .head(&url)
        .call()
        .ok()
        .and_then(|r| r.header("content-length").and_then(|v| v.parse::<u64>().ok()))
        .unwrap_or(0);
    println!("✓ 带 album_audio_id={}: {label} .{ext} 大小={}", s.album_audio_id, bytes_mb(size));
    // 存档行场景（最近播放/收藏下载）：只有 128 hash，按"标题+歌手"反查补齐
    let (hq2, sq2, sup2) = enrich_hashes(&s.id, &s.name, &s.singer, "", "", "");
    println!(
        "存档行反查（{} {}）: hq={} sq={} super={}",
        s.name,
        s.singer,
        hq2.len(),
        sq2.len(),
        sup2.len()
    );
    match song_url(&s.id, 0, 0, &hq2, &sq2, &sup2, s.vip, &token, &userid, "lossless") {
        Ok((url0, ext0, label0)) => {
            let size0 = agent
                .head(&url0)
                .call()
                .ok()
                .and_then(|r| r.header("content-length").and_then(|v| v.parse::<u64>().ok()))
                .unwrap_or(0);
            println!("✓ 存档行反查后取链: {label0} .{ext0} 大小={}", bytes_mb(size0));
        }
        Err(e) => println!("✗ 存档行反查后取链仍失败（{e}）"),
    }
}

#[cfg(test)] // 仅测试用（生产构建否则报 never used）
fn bytes_mb(n: u64) -> String {
    format!("{:.2} MB", n as f64 / 1048576.0)
}

/// 榜单审计：翻页拉全后与单页 30 对比（需要网络）
#[test]
#[ignore]
fn test_toplist_tracks_count_audit() {
    let tops = toplists().expect("榜单列表失败");
    println!("榜单数: {}", tops.len());
    for t in tops.iter().take(4) {
        match toplist_tracks(t.id, 1) {
            Ok(songs) => println!("✓ 榜单「{}」→ {} 首", t.name, songs.len()),
            Err(e) => println!("✗ 榜单「{}」失败: {e}", t.name),
        }
    }
}

/// 歌单详情审计：拉一个真实推荐歌单，检查实际返回曲目数（需要网络）
#[test]
#[ignore]
fn test_playlist_tracks_count_audit() {
    let specials = special_recommend(0, 1).expect("推荐页失败");
    println!("推荐歌单数: {}", specials.len());
    // 逐页打印：每页 got 与累计去重数（服务端若忽略 begin_idx 会重复返回首页）
    let pagesize = 90i64;
    for pick in specials.iter().take(4) {
        let mut seen = std::collections::HashSet::new();
        let mut total = 0usize;
        let mut pages = String::new();
        for page in 1..=20i64 {
            let extra = vec![
                ("area_code".into(), "1".into()),
                ("begin_idx".into(), ((page - 1) * pagesize).to_string()),
                ("plat".into(), "1".into()),
                ("type".into(), "1".into()),
                ("mode".into(), "1".into()),
                ("personal_switch".into(), "1".into()),
                ("extend_fields".into(), "abtags,hot_cmt,popularization".into()),
                ("pagesize".into(), pagesize.to_string()),
                ("global_collection_id".into(), pick.id.clone()),
            ];
            let v = gateway("/pubsongs/v2/get_other_list_file_nofilt", "", &extra, None)
                .expect("拉取失败");
            let list = v
                .pointer("/data/songs")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default();
            let got = list.len();
            for t in &list {
                if let Some(h) = t.get("hash").and_then(|x| x.as_str()) {
                    if seen.insert(h.to_string()) {
                        total += 1;
                    }
                }
            }
            pages.push_str(&format!("p{page}={got}/{} ", total));
            if got < pagesize as usize {
                break;
            }
        }
        println!("「{}」: {}", pick.name, pages);
    }
}
