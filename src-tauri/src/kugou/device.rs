//! 设备标识与注册（mid/dfid）及 AES/RSA 加密辅助。

use super::*;

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
pub(super) fn generate_mid() -> String {
    let mut rng = rand::thread_rng();
    // 必须是真 hex 字符：`b'0' + 10..=15` 会得到 :;<=>? 而非 a-f
    const HEX: &[u8] = b"0123456789abcdef";
    let uuid = (0..32)
        .map(|i| {
            if [8, 12, 16, 20].contains(&i) {
                '-'
            } else {
                HEX[rng.gen_range(0..16)] as char
            }
        })
        .collect::<String>();
    let guid = md5_hex(&uuid);
    BigUint::parse_bytes(guid.as_bytes(), 16)
        .map(|n| n.to_string())
        .unwrap_or(guid)
}

/// 当前 mid：优先用注入值；未注入时生成一次并缓存（与 dfid_of 同生命周期）
pub(super) fn mid_of() -> String {
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
    REGISTERED_DFID.lock().unwrap().clone().unwrap_or_default()
}

/// 当前 dfid；未注册时立即走 register_dev 注册
pub(super) fn dfid_of() -> String {
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
pub(super) fn re_register() -> Option<String> {
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
    // 同 generate_mid：必须生成真 hex 字符（a-f），不能是 :;<=>?
    const HEX: &[u8] = b"0123456789abcdef";
    let guid: String = (0..32)
        .map(|i| {
            if [8, 12, 16, 20].contains(&i) {
                '-'
            } else {
                HEX[rng.gen_range(0..16)] as char
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
fn aes_128_cbc_decrypt(
    cipher_bytes: &[u8],
    key: &str,
    iv: &str,
) -> Result<serde_json::Value, String> {
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

pub(super) fn rand_dfid() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| char::from(b'a' + rng.gen_range(0..26)))
        .collect()
}
