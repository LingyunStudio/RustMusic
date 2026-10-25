//! WASAPI 独占输出的非 Windows 桩实现（仅编译期 API 兼容，见 wasapi_out.rs）。
//!
//! 独占模式是 Windows 专属能力；Linux/macOS 无对应机制。引擎/命令层通过
//! `crate::wasapi_out` 引用本模块的恒等 API：调用 `spawn_exclusive_session`
//! 一律失败，引擎按既有回退路径转共享模式播放，不影响可用性。
//! 正常情况下 Linux 前端不暴露独占开关、恢复设置时也强制关闭（main.rs），
//! 本桩只是兜底（例如数据库从 Windows 机器迁移过来带着旧设置）。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};

use rodio::Source;

/// 会话控制句柄：与 Windows 实现同构，字段仅作占位（不会有会话线程）
#[derive(Clone)]
#[allow(dead_code)] // 字段为 API 兼容保留：Linux 无会话线程，恒不被读写
pub struct ExclusiveCtl {
    pub active: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub stop: Arc<AtomicBool>,
    pub exited: Arc<AtomicBool>,
}

/// 等待会话线程退出（桩：无会话，恒视为已退出）
pub fn wait_session_exit(ctl: &ExclusiveCtl, _timeout_ms: u64) -> bool {
    ctl.stop.store(true, Ordering::Relaxed);
    true
}

/// 会话音频参数（与 Windows 实现同构）
#[allow(dead_code)] // 字段为 API 兼容保留：非 Windows 平台恒不开会话
pub struct ExclusiveParams {
    pub device_pref: Option<String>,
    pub channels: usize,
    pub src_rate: u32,
    pub volume_bits: Arc<AtomicU32>,
    pub speed_bits: Arc<AtomicU32>,
}

/// 启动独占播放会话（桩：非 Windows 平台一律失败，由引擎回退共享模式）
pub fn spawn_exclusive_session<S>(
    _src: S,
    _params: ExclusiveParams,
) -> Result<(ExclusiveCtl, mpsc::Receiver<Result<(), String>>), String>
where
    S: Source + Send + 'static,
{
    Err("WASAPI 独占模式仅支持 Windows".into())
}
