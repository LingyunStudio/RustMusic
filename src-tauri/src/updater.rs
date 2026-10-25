//! GitHub Release 自动更新。
//!
//! 流程：检查最新 release（按当前平台挑选附件：Windows 找 `*setup*.exe` 安装包，
//! Linux 找 AppImage）→ 前端弹窗展示 markdown 更新说明 → 用户确认后下载安装包
//! （带进度事件，校验大小与 SHA-256）→ 安装并自动重启：
//! - Windows：退出后由分离的 PowerShell 助手静默运行安装包（显式传
//!   `/DIR=<当前安装目录>`，安装位置不变），装完自动重启并清理临时文件；
//! - Linux（AppImage 运行）：下载文件落在当前 .AppImage 同目录，chmod +x 后
//!   原子 rename 覆盖运行中的 AppImage（rename 只替换目录项，旧进程仍持有旧
//!   inode），由分离的 sh 助手等旧进程退出后启动新版本；
//! - Linux（deb 安装）或 release 无本平台附件：应用内安装不可用，
//!   `can_install=false`，前端引导前往 GitHub 发布页手动下载。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// GitHub 仓库（owner/name），自动更新默认从这里拉取最新 Release。
/// 可用环境变量 RUSTMUSIC_UPDATE_REPO（"owner/name"）覆盖，用于测试/内部渠道发布
const GITHUB_REPO: &str = "LingyunStudio/RustMusic";

fn update_repo() -> String {
    match std::env::var("RUSTMUSIC_UPDATE_REPO") {
        Ok(repo) if repo.contains('/') && !repo.trim().is_empty() => repo.trim().to_string(),
        _ => GITHUB_REPO.to_string(),
    }
}
const UA: &str = "RustMusic-Updater";
/// 下载整体超时兜底：安装包一般 10~20 MB，弱网也足够；卡死连接最终会在此报错
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(20 * 60);

static DOWNLOAD_CANCEL: AtomicBool = AtomicBool::new(false);

pub fn cancel_download() {
    DOWNLOAD_CANCEL.store(true, Ordering::Relaxed);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// 最新版本号（去掉 tag 前缀 v，如 "0.1.2"）
    pub version: String,
    /// release notes（markdown 原文）
    pub notes: String,
    /// 本平台安装包附件；release 没有本平台附件时为 None
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_size: u64,
    /// 附件哈希（GitHub 返回 "sha256:<hex>"；缺省 = 无法校验，仅剩大小检查）
    pub asset_digest: Option<String>,
    /// 签名附件（<安装包>.sig，ed25519）；缺失 = 更新被拒绝（见 download），
    /// 仅当发布流水线签名缺失时才会发生
    pub asset_sig_url: Option<String>,
    pub published_at: String,
    /// 当前安装方式能否应用内自动更新（false → 前端引导前往发布页）
    pub can_install: bool,
}

/// "v0.1.2" / "0.1.2-beta.1" → [0, 1, 2]，无法解析的段按 0 处理
fn version_tuple(v: &str) -> Vec<u64> {
    let v = v.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let v = v.split(['-', '+']).next().unwrap_or(v);
    v.split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

fn is_newer(latest: &str, current: &str) -> bool {
    let (a, b) = (version_tuple(latest), version_tuple(current));
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// 从 release 附件里挑 Windows 安装包：优先 *setup*.exe（Inno 安装包），其次任意 .exe
#[cfg(windows)]
fn pick_setup_asset(assets: &[serde_json::Value]) -> Option<(String, String, u64, Option<String>)> {
    let mut fallback = None;
    for a in assets {
        let (Some(name), Some(url)) = (a["name"].as_str(), a["browser_download_url"].as_str())
        else {
            continue;
        };
        let lower = name.to_lowercase();
        if !lower.ends_with(".exe") {
            continue;
        }
        let item = (
            name.to_string(),
            url.to_string(),
            a["size"].as_u64().unwrap_or(0),
            a["digest"].as_str().map(|s| s.to_string()),
        );
        if lower.contains("setup") {
            return Some(item);
        }
        fallback.get_or_insert(item);
    }
    fallback
}

/// 从 release 附件里挑 Linux AppImage：优先匹配当前架构
///（deb 命名用 amd64/arm64，Rust 的 ARCH 是 x86_64/aarch64，两种都认）
#[cfg(target_os = "linux")]
fn pick_appimage_asset(
    assets: &[serde_json::Value],
) -> Option<(String, String, u64, Option<String>)> {
    let aliases: &[&str] = match std::env::consts::ARCH {
        "x86_64" => &["x86_64", "amd64"],
        "aarch64" => &["aarch64", "arm64"],
        other => &[other],
    };
    let mut fallback = None;
    for a in assets {
        let (Some(name), Some(url)) = (a["name"].as_str(), a["browser_download_url"].as_str())
        else {
            continue;
        };
        let lower = name.to_lowercase();
        if !lower.ends_with(".appimage") {
            continue;
        }
        let item = (
            name.to_string(),
            url.to_string(),
            a["size"].as_u64().unwrap_or(0),
            a["digest"].as_str().map(|s| s.to_string()),
        );
        if aliases.iter().any(|s| lower.contains(s)) {
            return Some(item);
        }
        fallback.get_or_insert(item);
    }
    fallback
}

/// 按当前平台挑选可安装附件；None = release 没有本平台的安装包
#[cfg(windows)]
fn pick_platform_asset(
    assets: &[serde_json::Value],
) -> Option<(String, String, u64, Option<String>)> {
    pick_setup_asset(assets)
}

#[cfg(target_os = "linux")]
fn pick_platform_asset(
    assets: &[serde_json::Value],
) -> Option<(String, String, u64, Option<String>)> {
    pick_appimage_asset(assets)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn pick_platform_asset(_assets: &[serde_json::Value]) -> Option<(String, String, u64, Option<String>)> {
    None
}

/// 当前进程是否以 AppImage 方式运行（AppImage 运行时会注入 APPIMAGE 环境变量，
/// 指向 .AppImage 文件自身；deb/apt 等系统包安装时没有该变量）
#[cfg(target_os = "linux")]
fn running_appimage() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// 当前安装方式是否支持应用内自动更新
#[cfg(windows)]
fn can_install_in_app() -> bool {
    true
}

#[cfg(target_os = "linux")]
fn can_install_in_app() -> bool {
    running_appimage().is_some()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn can_install_in_app() -> bool {
    false
}

/// GitHub 访问走系统代理（环境变量）：无代理网络下直连 GitHub 会被重置/极慢，
/// 而浏览器等遵循系统代理的应用速度正常。ureq 不读系统/环境代理，需显式传入；
/// 未设置代理或代理无效时回退直连。
fn github_agent() -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new();
    if let Some(p) = crate::qq::system_proxy() {
        builder = builder.proxy(p);
    }
    builder.build()
}

/// 查询 GitHub 最新 release；有更新返回安装包信息，已是最新返回 None
pub fn fetch_latest(current_version: &str) -> Result<Option<UpdateInfo>, String> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", update_repo());
    let resp = github_agent()
        .get(&url)
        .set("User-Agent", UA)
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(12))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => "仓库还没有发布版本".to_string(),
            ureq::Error::Status(code, _) => format!("GitHub 返回状态 {code}"),
            other => format!("无法连接 GitHub：{other}"),
        })?;
    let release: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("解析发布信息失败：{e}"))?;

    let tag = release["tag_name"].as_str().unwrap_or_default();
    if tag.is_empty() {
        return Err("仓库还没有发布版本".into());
    }
    if !is_newer(tag, current_version) {
        return Ok(None);
    }
    let assets = release["assets"]
        .as_array()
        .ok_or_else(|| "发布信息缺少附件".to_string())?;
    let asset = pick_platform_asset(&assets);
    // 没有本平台附件也照常提示新版本，前端会引导去发布页手动下载
    let can_install = asset.is_some() && can_install_in_app();
    let (asset_name, asset_url, asset_size, asset_digest) = match asset {
        Some((name, url, size, digest)) => (Some(name), Some(url), size, digest),
        None => (None, None, 0, None),
    };
    // 签名附件：<安装包名>.sig（发布流水线自动生成）
    let asset_sig_url = asset_name.as_ref().and_then(|n| {
        let sig_name = format!("{n}.sig");
        assets.iter().find_map(|a| {
            (a["name"].as_str() == Some(sig_name.as_str()))
                .then(|| a["browser_download_url"].as_str().map(|s| s.to_string()))
                .flatten()
        })
    });

    Ok(Some(UpdateInfo {
        version: tag
            .trim()
            .trim_start_matches(|c| c == 'v' || c == 'V')
            .to_string(),
        notes: release["body"].as_str().unwrap_or_default().to_string(),
        asset_name,
        asset_url,
        asset_size,
        asset_digest,
        asset_sig_url,
        published_at: release["published_at"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        can_install,
    }))
}

/// 下载目标目录：AppImage 运行时必须与当前 .AppImage 同目录——原地替换依赖
/// 同一文件系统内的原子 rename，跨文件系统（如 /tmp）会 EXDEV 失败；其余用系统临时目录
fn download_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    if let Some(dir) = running_appimage().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
        return dir;
    }
    std::env::temp_dir()
}

/// 下载安装包到下载目录，期间通过 `update://progress` 事件上报进度。
/// 更新包 ed25519 验签公钥（hex）。私钥由发布方离线保管
///（installer/signing.key，不入库），发布流水线对安装包签名并随附件上传
///（<安装包名>.sig）。公钥内置使更新校验不依赖 GitHub 的可信度：
/// 发布账号被盗也只能推送会被验签拒绝的文件
const UPDATE_PUBKEY: &str = "cd75f7f605c81473c5096f4da54436782bb9531f678984637ab7d99c79fd0033";

/// 校验下载文件的 ed25519 签名（sig 为 64 字节签名 hex）
fn verify_signature(file: &std::path::Path, sig_hex: &str, pubkey_hex: &str) -> Result<(), String> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let mut pk = [0u8; 32];
    hex::decode_to_slice(pubkey_hex, &mut pk).map_err(|_| "内置公钥格式错误".to_string())?;
    let vk = VerifyingKey::from_bytes(&pk).map_err(|_| "内置公钥无效".to_string())?;
    let sig_bytes = hex::decode(sig_hex.trim()).map_err(|_| "签名格式错误".to_string())?;
    let sig = Signature::from_slice(&sig_bytes).map_err(|_| "签名格式错误".to_string())?;
    let data = std::fs::read(file).map_err(|e| format!("读取安装包失败: {e}"))?;
    vk.verify(&data, &sig)
        .map_err(|_| "更新包签名校验失败，已删除下载文件".to_string())
}

/// 返回下载文件的完整路径。校验链：大小一致 → SHA-256（GitHub 附件
/// digest，形如 "sha256:<hex>"，防传输损坏）→ ed25519 签名（公钥内置，
/// 防发布源被冒充）——全部通过才允许进入静默安装，任一不符即删除下载文件
pub fn download(
    app: &AppHandle,
    url: &str,
    name: &str,
    expected_size: u64,
    expected_digest: Option<&str>,
    sig_url: Option<&str>,
) -> Result<PathBuf, String> {
    use sha2::Digest as _;
    DOWNLOAD_CANCEL.store(false, Ordering::Relaxed);
    // 附件名固定为 RustMusic_*-setup.exe，仅允许常规文件名字符，避免拼进脚本出问题
    let safe_name: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '(' | ')'))
        .collect();
    if safe_name.is_empty() {
        return Err("安装包文件名无效".into());
    }
    // 临时文件名带进程号：固定可预测的路径可能被其它用户预创建/替换
    //（SHA-256 校验是第二道防线，不能当第一道）
    let path = download_dir().join(format!("{}-{safe_name}", std::process::id()));

    let resp = github_agent()
        .get(url)
        .set("User-Agent", UA)
        .timeout(DOWNLOAD_TIMEOUT)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("下载失败（HTTP {code}）"),
            other => format!("下载失败：{other}"),
        })?;
    let total = resp
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(expected_size);

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(&path).map_err(|e| format!("无法写入下载目录：{e}"))?;

    let mut buf = [0u8; 64 * 1024];
    let mut received: u64 = 0;
    let mut hasher = sha2::Sha256::new();
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    loop {
        if DOWNLOAD_CANCEL.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err("已取消下载".into());
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                hasher.update(&buf[..n]);
                file.write_all(&buf[..n])
                    .map_err(|e| format!("写入安装包失败：{e}"))?;
                received += n as u64;
                if last_emit.elapsed() >= Duration::from_millis(120) {
                    last_emit = Instant::now();
                    let _ = app.emit(
                        "update://progress",
                        serde_json::json!({ "received": received, "total": total }),
                    );
                }
            }
            Err(e) => {
                drop(file);
                let _ = std::fs::remove_file(&path);
                return Err(format!("下载中断：{e}"));
            }
        }
    }
    file.flush().map_err(|e| format!("写入安装包失败：{e}"))?;
    drop(file);

    if total > 0 && received != total {
        let _ = std::fs::remove_file(&path);
        return Err("下载不完整，请重试".into());
    }
    if let Some(d) = expected_digest {
        let expected = d.trim().to_lowercase();
        let expected = expected.strip_prefix("sha256:").unwrap_or(&expected);
        let actual = hex::encode(hasher.finalize());
        if !expected.is_empty() && actual != expected {
            let _ = std::fs::remove_file(&path);
            return Err("安装包校验失败（SHA-256 不符），已删除下载文件".into());
        }
    }
    // 签名校验：正常发布必有 <安装包>.sig 附件（build.ps1 自动签名）；
    // 缺失视为发布异常，拒绝安装（fail-closed）
    match sig_url {
        Some(u) => {
            let sig = github_agent()
                .get(u)
                .set("User-Agent", UA)
                .timeout(Duration::from_secs(20))
                .call()
                .map_err(|e| format!("下载更新包签名失败：{e}"))?
                .into_string()
                .map_err(|e| format!("读取更新包签名失败：{e}"))?;
            if let Err(e) = verify_signature(&path, &sig, UPDATE_PUBKEY) {
                let _ = std::fs::remove_file(&path);
                return Err(e);
            }
        }
        None => {
            let _ = std::fs::remove_file(&path);
            return Err("更新包未签名，已取消安装；请前往发布页手动下载".into());
        }
    }
    Ok(path)
}

#[cfg(windows)]
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// 生成 PowerShell 助手脚本内容：等应用退出 → 静默安装（/DIR 固定当前目录）→
/// 等安装进程结束（兼容 UAC 提权中转）→ 重启应用 → 清理安装包与脚本自身
#[cfg(windows)]
fn build_update_script(setup: &str, dir: &str, exe: &str) -> String {
    let setup_q = ps_quote(setup);
    let dir_q = ps_quote(dir);
    let exe_q = ps_quote(exe);
    let stem = Path::new(setup)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("rustmusic-setup");
    let stem_q = ps_quote(stem);

    format!(
        "$ErrorActionPreference = 'SilentlyContinue'\n\
         Start-Sleep -Seconds 2\n\
         $setup = {setup_q}\n\
         $dir = {dir_q}\n\
         $exe = {exe_q}\n\
         Start-Process -FilePath $setup -ArgumentList @('/VERYSILENT','/NORESTART','/SUPPRESSMSGBOXES',('/DIR=\"' + $dir + '\"'))\n\
         while (Get-Process -Name {stem_q} -ErrorAction SilentlyContinue) {{ Start-Sleep -Milliseconds 500 }}\n\
         Start-Sleep -Milliseconds 500\n\
         Start-Process -FilePath $exe\n\
         Start-Sleep -Seconds 2\n\
         Remove-Item -LiteralPath $setup -Force\n\
         Remove-Item -LiteralPath $PSCommandPath -Force\n",
    )
}

/// 安装并重启：
/// 1. 写出 PowerShell 助手脚本（UTF-8 BOM，中文路径可安全表示）并分离启动；
/// 2. 助手静默运行安装包，`/DIR` 显式指定为当前安装目录 → 位置不变；
/// 3. 安装进程结束后重启应用，最后清理临时文件。用户取消 UAC 时也会重启旧版本。
#[cfg(windows)]
pub fn install_and_restart(app: &AppHandle, setup: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败：{e}"))?;
    let dir = exe.parent().ok_or_else(|| "获取安装目录失败".to_string())?;
    if !setup.is_file() {
        return Err("安装包不存在，请重新下载".into());
    }

    let script = build_update_script(
        &setup.to_string_lossy(),
        &dir.to_string_lossy(),
        &exe.to_string_lossy(),
    );

    let script_path =
        std::env::temp_dir().join(format!("rustmusic-update-{}.ps1", std::process::id()));
    std::fs::write(&script_path, format!("\u{feff}{script}"))
        .map_err(|e| format!("写入更新脚本失败：{e}"))?;

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if let Err(e) = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-File",
        ])
        .arg(&script_path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
    {
        let _ = std::fs::remove_file(&script_path);
        return Err(format!("启动更新程序失败：{e}"));
    }

    // 助手已分离接管后续流程，稍后退出本进程（给前端留出展示状态的时间）
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        handle.exit(0);
    });
    Ok(())
}

/// Linux（AppImage 运行）：原地替换更新。下载文件与当前 .AppImage 同目录，
/// chmod +x 后原子 rename 覆盖运行中的 AppImage（rename 只替换目录项，旧进程
/// 仍持有旧 inode，运行不受影响），再由分离的 sh 助手等本进程退出后启动新版本。
/// 启动新版本前清掉 AppImage 运行时注入的环境变量（APPDIR/LD_LIBRARY_PATH 等
/// 指向旧挂载点，会随旧进程退出而失效）。
/// deb 等系统包安装方式需要 root 权限装包，应用内自动更新不可用。
#[cfg(target_os = "linux")]
pub fn install_and_restart(app: &AppHandle, setup: &Path) -> Result<(), String> {
    use std::os::unix::{fs::PermissionsExt, process::CommandExt};

    let Some(appimage) = running_appimage() else {
        return Err("当前安装方式（deb）不支持应用内自动更新，请到 GitHub 发布页下载新版本".into());
    };
    if !setup.is_file() {
        return Err("安装包不存在，请重新下载".into());
    }
    std::fs::set_permissions(setup, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("设置安装包执行权限失败：{e}"))?;
    std::fs::rename(setup, &appimage)
        .map_err(|e| format!("替换 AppImage 失败（所在目录可能不可写）：{e}"))?;

    // 分离助手：独立进程组（不随终端 SIGHUP 退出），等旧进程结束后 exec 新版本，
    // 避免新旧实例短暂并行争抢数据库/音频设备
    let helper = format!(
        "while kill -0 {pid} 2>/dev/null; do sleep 0.2; done; exec \"$1\"",
        pid = std::process::id()
    );
    let dir = appimage.parent().map(|p| p.to_path_buf());
    let mut cmd = std::process::Command::new("sh");
    cmd.arg("-c")
        .arg(&helper)
        .arg(&appimage) // $0
        .arg(&appimage) // $1，exec 的目标
        .process_group(0)
        .stdin(std::process::Stdio::null())
        .env_remove("APPDIR")
        .env_remove("OWD")
        .env_remove("LD_LIBRARY_PATH");
    if let Some(d) = &dir {
        cmd.current_dir(d);
    }
    if let Err(e) = cmd.spawn() {
        // 磁盘上已是新版本，只是没能自动拉起：提示手动重启即可
        return Err(format!("更新已写入，自动重启失败，请手动重新启动应用：{e}"));
    }

    // 助手已分离接管重启，稍后退出本进程（给前端留出展示状态的时间）
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600));
        handle.exit(0);
    });
    Ok(())
}

/// 其它平台（如 macOS）：应用内自动安装不可用，前端引导前往发布页
#[cfg(not(any(windows, target_os = "linux")))]
pub fn install_and_restart(_app: &AppHandle, _setup: &Path) -> Result<(), String> {
    Err("当前平台暂不支持应用内自动安装，请到 GitHub 发布页下载新版本".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("0.1.2", "0.1.1"));
        assert!(is_newer("v0.2.0", "0.1.1"));
        assert!(is_newer("0.2", "0.1.9"));
        assert!(is_newer("0.1.2.0", "0.1.1"));
        assert!(!is_newer("0.1.1", "0.1.1"));
        assert!(!is_newer("v0.1.1", "0.1.1"));
        assert!(!is_newer("0.1.0", "0.1.1"));
        assert!(!is_newer("0.1.1-beta.1", "0.1.1"));
    }

    #[test]
    fn version_tuple_parses() {
        assert_eq!(version_tuple("v0.1.2"), vec![0, 1, 2]);
        assert_eq!(version_tuple("0.1.2-beta.1"), vec![0, 1, 2]);
        assert_eq!(version_tuple(" 1.2 "), vec![1, 2]);
    }

    /// 走真实 GitHub API 的集成验证：需要网络，手动 `cargo test -- --ignored` 运行。
    /// 设置 RUSTMUSIC_UPDATE_REPO 后也会对测试仓库生效（发布测试版时验证整条链路用）
    #[test]
    fn signature_roundtrip_and_tamper() {
        use ed25519_dalek::{Signer, SigningKey};
        let dir = std::env::temp_dir().join(format!("rustmusic-sigtest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("asset.bin");
        let payload = b"rustmusic update payload";
        std::fs::write(&file, payload).unwrap();
        let sk = SigningKey::from_bytes(&[0x42u8; 32]);
        let sig = hex::encode(sk.sign(payload).to_bytes());
        let pk = hex::encode(sk.verifying_key().to_bytes());
        assert!(verify_signature(&file, &sig, &pk).is_ok());
        // 内容篡改后必须失败
        std::fs::write(&file, b"tampered payload").unwrap();
        assert!(verify_signature(&file, &sig, &pk).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore]
    fn fetch_latest_against_github() {
        let repo = update_repo();
        // 以很旧的版本查询：必然有更新，附件按平台挑选
        let info = fetch_latest("0.0.1").unwrap().expect("应有更新");
        assert!(is_newer(&info.version, "0.0.1"));
        if cfg!(windows) {
            assert!(info.can_install);
            assert!(info
                .asset_name
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .contains("setup"));
            assert!(info.asset_size > 0);
            assert!(info
                .asset_url
                .as_deref()
                .unwrap_or_default()
                .contains(&format!("github.com/{repo}/releases/download")));
        } else {
            // 非 Windows：能否应用内更新取决于安装方式（AppImage 可以），
            // 有 AppImage 附件时应命中，没有时只提示、引导去发布页
            let is_appimage = std::env::var_os("APPIMAGE").is_some();
            assert_eq!(info.can_install, is_appimage && info.asset_url.is_some());
            if let Some(name) = &info.asset_name {
                assert!(name.to_lowercase().ends_with(".appimage"));
                assert!(info.asset_size > 0);
                assert!(info
                    .asset_url
                    .as_deref()
                    .unwrap_or_default()
                    .contains(&format!("github.com/{repo}/releases/download")));
            }
        }

        // 以当前版本查询：除非仓库又发了更新，否则应为 None；若返回了则其版本必须更新
        let current = env!("CARGO_PKG_VERSION");
        if let Some(info) = fetch_latest(current).unwrap() {
            assert!(is_newer(&info.version, current));
        }
    }

    /// 仓库覆盖：非法值（无 owner/name 结构）回退默认仓库
    #[test]
    fn update_repo_override() {
        std::env::remove_var("RUSTMUSIC_UPDATE_REPO");
        assert_eq!(update_repo(), GITHUB_REPO);
        std::env::set_var("RUSTMUSIC_UPDATE_REPO", " someone/Fork-Music ");
        assert_eq!(update_repo(), "someone/Fork-Music");
        std::env::set_var("RUSTMUSIC_UPDATE_REPO", "invalid-no-slash");
        assert_eq!(update_repo(), GITHUB_REPO);
        std::env::remove_var("RUSTMUSIC_UPDATE_REPO");
    }

    /// AppImage 附件挑选：跳过非 AppImage，优先命中当前架构（amd64/x86_64 别名互认）
    #[test]
    #[cfg(target_os = "linux")]
    fn appimage_pick_prefers_current_arch() {
        use serde_json::json;
        let assets = vec![
            json!({"name": "RustMusic_0.1.2_arm64.AppImage", "size": 1,
                   "browser_download_url": "https://example.com/arm64.AppImage"}),
            json!({"name": "RustMusic_0.1.2_amd64.AppImage", "size": 2,
                   "browser_download_url": "https://example.com/amd64.AppImage"}),
            json!({"name": "RustMusic_0.1.2_amd64.deb", "size": 3,
                   "browser_download_url": "https://example.com/amd64.deb"}),
        ];
        let (name, url, size, _) = pick_appimage_asset(&assets).expect("应选中 AppImage");
        assert!(name.ends_with(".AppImage"));
        assert_eq!(size, 2);
        assert!(url.ends_with(".AppImage"));

        // 当前架构的附件缺席时回退到任意 AppImage
        let only_arm = vec![assets[0].clone()];
        let (name, _, _, _) = pick_appimage_asset(&only_arm).expect("应回退到 arm64 附件");
        assert!(name.contains("arm64"));

        assert!(pick_appimage_asset(&[assets[2].clone()]).is_none());
    }

    /// Windows 附件挑选：优先 *setup*.exe，忽略 .msi 等其它格式
    #[test]
    #[cfg(windows)]
    fn setup_pick_prefers_setup_exe() {
        use serde_json::json;
        let assets = vec![
            json!({"name": "RustMusic_0.1.2_x64_en-US.msi", "size": 3,
                   "browser_download_url": "https://example.com/a.msi"}),
            json!({"name": "RustMusic_0.1.2_x64-setup.exe", "size": 2,
                   "browser_download_url": "https://example.com/setup.exe"}),
        ];
        let (name, _, size, _) = pick_setup_asset(&assets).expect("应选中 setup.exe");
        assert!(name.ends_with("-setup.exe"));
        assert_eq!(size, 2);
    }

    #[test]
    #[cfg(windows)]
    fn script_quotes_paths_with_spaces_and_chinese() {
        let s = build_update_script(
            r"C:\Users\张三\AppData\Local\Temp\RustMusic_0.1.2.0_x64-setup.exe",
            r"C:\Program Files\RustMusic",
            r"C:\Program Files\RustMusic\rustmusic.exe",
        );
        // 落盘供 PowerShell 解析器做语法校验（UTF-8 BOM，与真实写入一致）
        let dump = std::env::temp_dir().join("rustmusic-update-script-test.ps1");
        std::fs::write(&dump, format!("\u{feff}{s}")).unwrap();
        assert!(s.contains(r"'C:\Users\张三\AppData\Local\Temp\RustMusic_0.1.2.0_x64-setup.exe'"));
        assert!(s.contains(r#"('/DIR="' + $dir + '"')"#));
        assert!(s.contains("'C:\\Program Files\\RustMusic'"));
        assert!(s.contains("Get-Process -Name 'RustMusic_0.1.2.0_x64-setup'"));
        // 单引号转义：路径里的 ' 必须 doubled，避免破坏 PS 字符串
        let q = build_update_script("C:\\it's\\setup.exe", "C:\\app", "C:\\app\\rustmusic.exe");
        assert!(q.contains("'C:\\it''s\\setup.exe'"));
    }
}
