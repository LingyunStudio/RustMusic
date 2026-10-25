//! 登录凭据存取：平台 token 统一走系统凭据管理器（keyring），不再明文落库。
//! SQLite 只保留非敏感字段（昵称、uid 等）。
//!
//! 旧版本把 token 明文写在 settings 表；启动时 migrate_legacy 迁入
//! keyring 并清除库内残留（keyring 写入失败时保留明文待下次重试，
//! 绝不在落 keyring 前丢数据）。
//!
//! 引擎/命令层的读取统一走 get；测试代码（#[ignore] 网络用例）仍可直连
//! 本机数据库或 keyring 自行取值。

use crate::db;

/// keyring 服务名（Entry::new(SERVICE, user)，user 复用 settings 表历史键名）
const SERVICE: &str = "RustMusic";

/// 走 keyring 的 token 键
const KEYS: [&str; 5] = [
    "qq_musickey",
    "kg_token",
    "netease_music_u",
    "bili_sessdata",
    "bili_buvid3",
];

fn entry(key: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, key).map_err(|e| format!("凭据管理器不可用: {e}"))
}

/// 读取平台 token；无凭据返回 None（含空串视为无）
pub fn get(key: &str) -> Option<String> {
    let v = entry(key).ok()?.get_password().ok()?;
    Some(v).filter(|s| !s.is_empty())
}

/// 写入平台 token（空串 = 清除）
pub fn set(key: &str, value: &str) -> Result<(), String> {
    let e = entry(key)?;
    if value.is_empty() {
        match e.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(format!("清除凭据失败: {err}")),
        }
    } else {
        e.set_password(value).map_err(|err| format!("保存凭据失败: {err}"))
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
}
