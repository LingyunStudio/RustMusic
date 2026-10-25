//! 非 Windows 占位：任务栏歌词是 Windows 专属功能。
//! 保持与 taskbar_lyric.rs 相同的公开接口（含命令签名引用的类型），
//! 调用点无需 cfg 分支。命令注册表全平台共用，实际命令在非 Windows
//! 平台返回 Err。

/// 与 taskbar_lyric.rs 同构的显示器信息（命令返回类型签名需要）
#[derive(Debug, serde::Serialize)]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    pub primary: bool,
    pub has_taskbar: bool,
}

#[allow(dead_code)]
pub fn spawn() {}

#[allow(dead_code)]
pub fn list_displays() -> Vec<DisplayInfo> {
    Vec::new()
}
