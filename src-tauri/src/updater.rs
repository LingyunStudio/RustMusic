//! GitHub Release 自动更新。
//!
//! 流程：检查最新 release → 前端弹窗展示 markdown 更新说明 → 用户确认后
//! 下载安装包到临时目录（带进度事件）→ 退出应用，由分离的 PowerShell 助手
//! 静默运行 Inno Setup 安装包（显式传 `/DIR=<当前安装目录>`，安装位置保持
//! 不变），安装进程结束后自动重启应用并清理临时文件。

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// GitHub 仓库（owner/name），自动更新从这里拉取最新 Release
const GITHUB_REPO: &str = "LingyunStudio/RustMusic";
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
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
    pub published_at: String,
}

/// "v0.1.2" / "0.1.2-beta.1" → [0, 1, 2]，无法解析的段按 0 处理
fn version_tuple(v: &str) -> Vec<u64> {
    let v = v.trim().trim_start_matches(|c| c == 'v' || c == 'V');
    let v = v.split(['-', '+']).next().unwrap_or(v);
    v.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect()
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
fn pick_setup_asset(assets: &[serde_json::Value]) -> Option<(String, String, u64)> {
    let mut fallback = None;
    for a in assets {
        let (Some(name), Some(url)) = (a["name"].as_str(), a["browser_download_url"].as_str()) else {
            continue;
        };
        let lower = name.to_lowercase();
        if !lower.ends_with(".exe") {
            continue;
        }
        let item = (name.to_string(), url.to_string(), a["size"].as_u64().unwrap_or(0));
        if lower.contains("setup") {
            return Some(item);
        }
        fallback.get_or_insert(item);
    }
    fallback
}

/// 查询 GitHub 最新 release；有更新返回安装包信息，已是最新返回 None
pub fn fetch_latest(current_version: &str) -> Result<Option<UpdateInfo>, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let resp = ureq::get(&url)
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
    let (asset_name, asset_url, asset_size) = pick_setup_asset(assets).ok_or_else(|| {
        format!(
            "新版本 {tag} 未提供 Windows 安装包，请到 GitHub 发布页手动下载"
        )
    })?;

    Ok(Some(UpdateInfo {
        version: tag.trim().trim_start_matches(|c| c == 'v' || c == 'V').to_string(),
        notes: release["body"].as_str().unwrap_or_default().to_string(),
        asset_name,
        asset_url,
        asset_size,
        published_at: release["published_at"].as_str().unwrap_or_default().to_string(),
    }))
}

/// 下载安装包到临时目录，期间通过 `update://progress` 事件上报进度。
/// 返回下载文件的完整路径。文件大小与 release 附件声明的 size 校验一致。
pub fn download(app: &AppHandle, url: &str, name: &str, expected_size: u64) -> Result<PathBuf, String> {
    DOWNLOAD_CANCEL.store(false, Ordering::Relaxed);
    // 附件名固定为 RustMusic_*-setup.exe，仅允许常规文件名字符，避免拼进脚本出问题
    let safe_name: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '(' | ')'))
        .collect();
    if safe_name.is_empty() {
        return Err("安装包文件名无效".into());
    }
    let path = std::env::temp_dir().join(&safe_name);

    let resp = ureq::get(url)
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
    let mut file = std::fs::File::create(&path).map_err(|e| format!("无法写入临时目录：{e}"))?;

    let mut buf = [0u8; 64 * 1024];
    let mut received: u64 = 0;
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
    Ok(path)
}

fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// 生成 PowerShell 助手脚本内容：等应用退出 → 静默安装（/DIR 固定当前目录）→
/// 等安装进程结束（兼容 UAC 提权中转）→ 重启应用 → 清理安装包与脚本自身
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
pub fn install_and_restart(app: &AppHandle, setup: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("获取程序路径失败：{e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "获取安装目录失败".to_string())?;
    if !setup.is_file() {
        return Err("安装包不存在，请重新下载".into());
    }

    let script = build_update_script(
        &setup.to_string_lossy(),
        &dir.to_string_lossy(),
        &exe.to_string_lossy(),
    );

    let script_path = std::env::temp_dir().join(format!("rustmusic-update-{}.ps1", std::process::id()));
    std::fs::write(&script_path, format!("\u{feff}{script}"))
        .map_err(|e| format!("写入更新脚本失败：{e}"))?;

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    if let Err(e) = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
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

    /// 走真实 GitHub API 的集成验证：需要网络，手动 `cargo test -- --ignored` 运行
    #[test]
    #[ignore]
    fn fetch_latest_against_github() {
        // 当前仓库最新即 v0.1.1：以当前版本查询应返回 None（无更新）
        assert!(fetch_latest("0.1.1").unwrap().is_none());
        // 以旧版本查询应返回安装包信息
        let info = fetch_latest("0.0.9").unwrap().expect("应有更新");
        assert!(is_newer(&info.version, "0.0.9"));
        assert!(info.asset_url.contains("github.com/LingyunStudio/RustMusic/releases/download"));
        assert!(info.asset_name.to_lowercase().contains("setup"));
        assert!(info.asset_size > 0);
    }

    #[test]
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
