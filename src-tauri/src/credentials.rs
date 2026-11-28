//! 登录凭据存取：平台 token 统一走系统凭据管理器（keyring），不再明文落库。
//! SQLite 只保留非敏感字段（昵称、uid 等）。
//!
//! 旧版本把 token 明文写在 settings 表；启动时 migrate_legacy 迁入
//! keyring 并清除库内残留（keyring 写入失败时保留明文待下次重试，
//! 绝不在落 keyring 前丢数据）。
//!
//! 引擎/命令层的读取统一走 get；测试代码（#[ignore] 网络用例）仍可直连
//! 本机数据库或 keyring 自行取值。

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::RwLock;

use crate::db;

/// 进程内凭据缓存：消除每次业务命令都触发系统凭据管理器同步 IPC
/// （Windows CredRead / Linux D-Bus Secret Service / macOS Keychain），
/// 写入或清除时立即同步更新缓存
static CRED_CACHE: OnceLock<RwLock<HashMap<String, Option<String>>>> = OnceLock::new();

fn cache() -> &'static RwLock<HashMap<String, Option<String>>> {
    CRED_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// keyring 服务名（Entry::new(SERVICE, user)，user 复用 settings 表历史键名）
const SERVICE: &str = "RustMusic";

/// 走 keyring 的 token 键
const KEYS: [&str; 6] = [
    "qq_musickey",
    "qq_session",
    "kg_token",
    "netease_music_u",
    "bili_sessdata",
    "bili_buvid3",
];

fn entry(key: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, key).map_err(|e| {
        crate::i18n::pick(
            format!("凭据管理器不可用: {e}"),
            format!("Credential manager unavailable: {e}"),
        )
    })
}

/// 读取平台 token；无凭据返回 None（含空串视为无）
pub fn get(key: &str) -> Option<String> {
    if let Some(hit) = cache().read().get(key) {
        return hit.clone();
    }
    let val = entry(key)
        .ok()
        .and_then(|e| e.get_password().ok())
        .filter(|s| !s.is_empty());
    cache().write().insert(key.to_string(), val.clone());
    val
}

/// 写入平台 token（空串 = 清除）
pub fn set(key: &str, value: &str) -> Result<(), String> {
    let e = entry(key)?;
    if value.is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {
                cache().write().insert(key.to_string(), None);
                Ok(())
            }
            Err(err) => Err(crate::i18n::pick(
                format!("清除凭据失败: {err}"),
                format!("Failed to clear credentials: {err}"),
            )),
        }
    } else {
        e.set_password(value).map_err(|err| {
            crate::i18n::pick(
                format!("保存凭据失败: {err}"),
                format!("Failed to save credentials: {err}"),
            )
        })?;
        cache()
            .write()
            .insert(key.to_string(), Some(value.to_string()));
        Ok(())
    }
}

/// 清除平台 token（幂等）
pub fn clear(key: &str) {
    let _ = set(key, "");
}

/// 启动迁移：settings 表里的存量明文 token → keyring，成功后清除库内残留
pub fn migrate_legacy(conn: &rusqlite::Connection) {
    for key in KEYS {
        let legacy = db::get_setting(conn, key).unwrap_or_default();
        if legacy.is_empty() {
            continue;
        }
        if get(key).is_some() {
            // 已迁移过（库内残留双写）：直接清库
            let _ = db::set_setting(conn, key, "");
            continue;
        }
        if set(key, &legacy).is_ok() {
            let _ = db::set_setting(conn, key, "");
            eprintln!("[credentials] {key} 已迁移至系统凭据管理器");
        } else {
            eprintln!("[credentials] {key} 迁移失败（凭据管理器不可用），保留明文待下次重试");
        }
    }
    // 兼容迁移：旧版本曾把微信续期凭证 wxrefresh_token 明文存于 settings 表，
    // 迁入 keyring 的 qq_session JSON 后清理库内明文残留
    let legacy_wx_rt = db::get_setting(conn, "qq_wx_refresh_token").unwrap_or_default();
    if !legacy_wx_rt.is_empty() {
        if get("qq_session").is_none() {
            let musicid = db::get_setting(conn, "qq_musicid").unwrap_or_default();
            let musickey = get("qq_musickey").unwrap_or_default();
            let euin = db::get_setting(conn, "qq_encrypt_uin").unwrap_or_default();
            let nick = db::get_setting(conn, "qq_nickname").unwrap_or_default();
            let seeded = serde_json::json!({
                "musicid": musicid,
                "musickey": musickey,
                "loginType": 1,
                "refreshToken": legacy_wx_rt,
                "encryptUin": euin,
                "nickname": nick,
            });
            if set("qq_session", &seeded.to_string()).is_ok() {
                let _ = db::set_setting(conn, "qq_wx_refresh_token", "");
                eprintln!("[credentials] qq_wx_refresh_token 已迁入系统凭据管理器 qq_session");
            }
        } else {
            let _ = db::set_setting(conn, "qq_wx_refresh_token", "");
        }
    }
}
