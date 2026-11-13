//! 应用国际化辅助模块（支持自动检测系统语言与手动切换 zh / en）

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

const PREF_AUTO: u8 = 0;
const PREF_ZH: u8 = 1;
const PREF_EN: u8 = 2;

const HINT_NONE: u8 = 0;
const HINT_ZH: u8 = 1;
const HINT_EN: u8 = 2;

static LANGUAGE_PREF: AtomicU8 = AtomicU8::new(PREF_AUTO);
static SYSTEM_LANG_HINT: AtomicU8 = AtomicU8::new(HINT_NONE);

/// 规范化语言偏好配置值
pub fn normalize_pref(pref: &str) -> &'static str {
    match pref.trim().to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh_cn" | "zh-hans" => "zh",
        "en" | "en-us" | "en_us" | "en-gb" => "en",
        _ => "auto",
    }
}

/// 从数据库读取语言设置并初始化
pub fn init_from_db(conn: &rusqlite::Connection) {
    let pref = crate::db::get_setting(conn, "language")
        .map(|s| normalize_pref(&s).to_string())
        .unwrap_or_else(|| "auto".to_string());
    set_preference(&pref);
}

/// 设置当前语言偏好（"auto" | "zh" | "en"）
pub fn set_preference(pref: &str) {
    let val = match normalize_pref(pref) {
        "zh" => PREF_ZH,
        "en" => PREF_EN,
        _ => PREF_AUTO,
    };
    LANGUAGE_PREF.store(val, Ordering::SeqCst);
}

/// 获取当前语言偏好字符串
pub fn get_preference() -> &'static str {
    match LANGUAGE_PREF.load(Ordering::SeqCst) {
        PREF_ZH => "zh",
        PREF_EN => "en",
        _ => "auto",
    }
}

/// 记录来自前端 WebView 的系统语言提示（解决 macOS GUI 启动缺少 LANG 环境变量的问题）
pub fn set_system_hint(lang: Option<&str>) {
    let val = match lang.and_then(parse_locale_tag) {
        Some(Lang::Zh) => HINT_ZH,
        Some(Lang::En) => HINT_EN,
        None => HINT_NONE,
    };
    SYSTEM_LANG_HINT.store(val, Ordering::SeqCst);
}

/// 解析区域标识字符串（例如 "zh_CN.UTF-8"、"zh-Hans-CN"、"en-US"）
pub fn parse_locale_tag(raw: &str) -> Option<Lang> {
    let primary = raw
        .split(['.', '@'])
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if primary.is_empty() || primary == "c" || primary == "posix" {
        return None;
    }
    let first = primary.split(['-', '_']).next().unwrap_or("");
    if first == "zh" {
        Some(Lang::Zh)
    } else if !first.is_empty() {
        Some(Lang::En)
    } else {
        None
    }
}

#[cfg(windows)]
fn detect_native_system_lang() -> Option<Lang> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // Windows LANGID 低 10 位为主语言 ID，0x04 为 LANG_CHINESE
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    let primary_lang = lang_id & 0x03FF;
    if primary_lang == 0x04 {
        Some(Lang::Zh)
    } else {
        Some(Lang::En)
    }
}

#[cfg(not(windows))]
fn detect_native_system_lang() -> Option<Lang> {
    for key in ["LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"] {
        if let Ok(val) = std::env::var(key) {
            if let Some(lang) = parse_locale_tag(&val) {
                return Some(lang);
            }
        }
    }
    None
}

/// 检测系统语言（优先原生系统 API / 环境变量，其次前端提示，默认回退 English）
pub fn detect_system_lang() -> Lang {
    match SYSTEM_LANG_HINT.load(Ordering::SeqCst) {
        HINT_ZH => return Lang::Zh,
        HINT_EN => return Lang::En,
        _ => {}
    }
    detect_native_system_lang().unwrap_or(Lang::En)
}

/// 系统检测到的语言代码（"zh" | "en"）
pub fn system_lang_code() -> &'static str {
    match detect_system_lang() {
        Lang::Zh => "zh",
        Lang::En => "en",
    }
}

/// 当前生效语言
pub fn current_lang() -> Lang {
    match LANGUAGE_PREF.load(Ordering::SeqCst) {
        PREF_ZH => Lang::Zh,
        PREF_EN => Lang::En,
        _ => detect_system_lang(),
    }
}

/// 当前是否处于英文模式
#[inline]
pub fn is_en() -> bool {
    current_lang() == Lang::En
}

/// 根据当前语言二选一
#[inline]
pub fn pick<T>(zh: T, en: T) -> T {
    if is_en() {
        en
    } else {
        zh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_pref_handles_variants() {
        assert_eq!(normalize_pref("zh"), "zh");
        assert_eq!(normalize_pref("ZH-CN"), "zh");
        assert_eq!(normalize_pref("en-US"), "en");
        assert_eq!(normalize_pref("auto"), "auto");
        assert_eq!(normalize_pref("fr"), "auto");
    }

    #[test]
    fn parse_locale_tag_detects_chinese_and_english() {
        assert_eq!(parse_locale_tag("zh_CN.UTF-8"), Some(Lang::Zh));
        assert_eq!(parse_locale_tag("zh-Hans-CN"), Some(Lang::Zh));
        assert_eq!(parse_locale_tag("zh-TW"), Some(Lang::Zh));
        assert_eq!(parse_locale_tag("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(parse_locale_tag("ja-JP"), Some(Lang::En));
        assert_eq!(parse_locale_tag("C"), None);
        assert_eq!(parse_locale_tag("POSIX"), None);
        assert_eq!(parse_locale_tag(""), None);
    }

    #[test]
    fn manual_preference_overrides_detection() {
        let prev = get_preference();
        set_preference("en");
        assert_eq!(current_lang(), Lang::En);
        assert!(is_en());
        assert_eq!(pick("中文", "English"), "English");

        set_preference("zh");
        assert_eq!(current_lang(), Lang::Zh);
        assert!(!is_en());
        assert_eq!(pick("中文", "English"), "中文");

        set_preference(prev);
    }
}
