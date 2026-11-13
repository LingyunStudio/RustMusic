//! 网络层公共辅助（issue#8-2）：传输级错误识别、用户可读文案与一次自动重试。
//!
//! 背景：系统代理 / 网络切换后，取链整链报错（原 issue 报告为 "API请求错误"）。
//! ureq 的传输级错误（DNS / 连接 / TLS 失败）多为瞬态——等待片刻重试一次
//! 即可自愈；连接池类 agent（QQ）在传输错误时由调用方重建后重试。

/// 是否传输级（网络层）错误——非 HTTP 状态码错误，重试有意义
pub fn is_transport_err(e: &ureq::Error) -> bool {
    matches!(e, ureq::Error::Transport(_))
}

/// 传输错误的用户可读文案（替代裸 ureq 英文文本）
pub fn transport_text() -> &'static str {
    crate::i18n::pick(
        "网络连接失败（请检查网络或代理设置后重试）",
        "Network connection failed (check network or proxy settings and retry)",
    )
}

/// 带一次传输级重试：`f` 每次调用须新建请求（不复用可疑连接）。
/// 首次传输错误 → 等 600ms 重试一次；仍失败返回友好文案。
pub fn with_retry<T>(mut f: impl FnMut() -> Result<T, ureq::Error>) -> Result<T, String> {
    match f() {
        Ok(v) => Ok(v),
        Err(e) if is_transport_err(&e) => {
            std::thread::sleep(std::time::Duration::from_millis(600));
            f().map_err(|e2| {
                if is_transport_err(&e2) {
                    transport_text().to_string()
                } else {
                    e2.to_string()
                }
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

/// 带一次传输级重试（保留原始 ureq::Error——HTTP 状态码错误携带的
/// 响应体需要被调用方读取时使用，如酷狗网关把错误 body 交给上层解析）
pub fn with_retry_raw<T>(mut f: impl FnMut() -> Result<T, ureq::Error>) -> Result<T, ureq::Error> {
    match f() {
        Ok(v) => Ok(v),
        Err(e) if is_transport_err(&e) => {
            std::thread::sleep(std::time::Duration::from_millis(600));
            f()
        }
        Err(e) => Err(e),
    }
}
