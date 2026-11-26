//! 设置：播放音质 / 关闭行为 / 自动更新 / 缓存管理

use serde_json::json;
use tauri::State;

use crate::db;
use crate::models::SettingsPayload;
use crate::AppState;

use super::engine_clone;
#[tauri::command]
pub async fn set_play_quality(
    state: State<'_, AppState>,
    kind: String,
    quality: String,
) -> Result<(), String> {
    let key = match kind.as_str() {

        "qq" => {
            // QQ 音乐官方 App 完整档位：
            // standard(128k) / high(HQ 320k) / lossless(SQ 无损) / hires(Hi-Res 24bit)
            // atmos_2(臻品音质 2.0 SVIP) / atmos(臻品全景声 SVIP) / master(臻品母带 24bit/192kHz SVIP)
            if !matches!(
                quality.as_str(),
                "standard" | "high" | "lossless" | "hires" | "atmos_2" | "atmos" | "master"
            ) {
                return Err(crate::i18n::pick("无效的音质选项", "Invalid audio quality option").into());
            }
            "quality_qq".to_string()
        }
        "netease" => {
            // 网易云官方 App 完整档位（weapi level）：
            // standard(128k) / higher(192k) / high(极高 320k) / lossless(无损) / hires(Hi-Res)
            // jyeffect(高清环绕声 VIP) / sky(沉浸环绕声 SVIP) / jymaster(超清母带 SVIP)
            if !matches!(
                quality.as_str(),
                "standard"
                    | "higher"
                    | "high"
                    | "lossless"
                    | "hires"
                    | "jyeffect"
                    | "sky"
                    | "jymaster"
            ) {
                return Err(crate::i18n::pick("无效的音质选项", "Invalid audio quality option").into());
            }
            "quality_netease".to_string()
        }
        "kugou" => {
            // 酷狗音乐官方 App 完整档位：
            // standard(128k) / high(HQ 320k) / lossless(SQ 无损) / hires(Hi-Res)
            // viper_clear(蝰蛇超清 SVIP) / viper_atmos(蝰蛇全景声 SVIP) / viper_tape(蝰蛇母带 SVIP)
            if !matches!(
                quality.as_str(),
                "standard"
                    | "high"
                    | "lossless"
                    | "hires"
                    | "viper_clear"
                    | "viper_atmos"
                    | "viper_tape"
                    | "master"
            ) {
                return Err(crate::i18n::pick("无效的音质选项", "Invalid audio quality option").into());
            }
            "quality_kugou".to_string()
        }
        _ => return Err(crate::i18n::pick("无效的音源", "Invalid audio source").into()),
    };
    let conn = state.db.lock();
    db::set_setting(&conn, &key, &quality);
    Ok(())
}

/// 关闭主窗口行为：tray = 最小化到托盘（默认）；exit = 直接退出应用
#[tauri::command]
pub async fn set_close_action(state: State<'_, AppState>, action: String) -> Result<(), String> {
    if !matches!(action.as_str(), "tray" | "exit") {
        return Err(crate::i18n::pick("无效的关闭行为", "Invalid close window action").into());
    }
    let conn = state.db.lock();
    db::set_setting(&conn, "close_action", &action);
    Ok(())
}
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    let conn = state.db.lock();
    let volume: f32 = db::get_setting(&conn, "volume")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.8);
    let speed: f32 = db::get_setting(&conn, "speed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    let eq_gains: Vec<f32> = db::get_setting(&conn, "eq_gains")
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| vec![0.0; 10]);
    let eq_enabled = db::get_setting(&conn, "eq_enabled")
        .map(|s| s == "true")
        .unwrap_or(false);
    let quality = db::get_setting(&conn, "quality").unwrap_or_else(|| "high".to_string());
    // 三源分别的音质（issue 跟进：各源权益/档位不同，分开设置）；
    // 未单独设置过时回落到旧的单一 quality，老用户设置无缝迁移
    let quality_netease = super::source_quality(&conn, "netease");
    let quality_qq = super::source_quality(&conn, "qq");
    let quality_kugou = super::source_quality(&conn, "kugou");
    let cache_limit: u64 = db::get_setting(&conn, "cache_limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(2 * 1024 * 1024 * 1024);
    let close_action = db::get_setting(&conn, "close_action").unwrap_or_else(|| "tray".to_string());
    let auto_update = db::get_setting(&conn, "auto_update")
        .map(|s| s != "false")
        .unwrap_or(true);
    let wasapi_exclusive = db::get_setting(&conn, "wasapi_exclusive")
        .map(|s| s == "true")
        .unwrap_or(false);
    let language = db::get_setting(&conn, "language")
        .map(|s| crate::i18n::normalize_pref(&s).to_string())
        .unwrap_or_else(|| "auto".to_string());
    Ok(SettingsPayload {
        volume,
        speed,
        eq_gains,
        eq_enabled,
        quality,
        quality_netease,
        quality_qq,
        quality_kugou,
        cache_limit,
        close_action,
        auto_update,
        wasapi_exclusive,
        language,
    })
}

/// 设置界面语言（"auto" | "zh" | "en"），并即时同步托盘与桌面歌词窗口标题
#[tauri::command]
pub async fn set_language(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    lang: String,
    system_hint: Option<String>,
) -> Result<String, String> {
    use tauri::{Emitter, Manager};
    let normalized = crate::i18n::normalize_pref(&lang).to_string();
    {
        let conn = state.db.lock();
        db::set_setting(&conn, "language", &normalized);
    }
    if let Some(hint) = system_hint.as_deref() {
        crate::i18n::set_system_hint(Some(hint));
    }
    crate::i18n::set_preference(&normalized);
    crate::tray::refresh_tray_labels(&app);
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        let _ = w.set_title(crate::i18n::pick("桌面歌词", "Desktop Lyrics"));
    }
    let _ = app.emit(
        "app://language-changed",
        serde_json::json!({ "language": normalized }),
    );
    Ok(normalized)
}

/// 同步系统语言提示（macOS/Linux GUI 启动无 LANG 时由前端传入 navigator.language）
#[tauri::command]
pub async fn sync_system_lang(app: tauri::AppHandle, hint: String) -> Result<(), String> {
    use tauri::Manager;
    crate::i18n::set_system_hint(Some(&hint));
    if crate::i18n::get_preference() == "auto" {
        crate::tray::refresh_tray_labels(&app);
        if let Some(w) = app.get_webview_window("desktop-lyrics") {
            let _ = w.set_title(crate::i18n::pick("桌面歌词", "Desktop Lyrics"));
        }
    }
    Ok(())
}

/// 设置是否在启动时自动检查更新
#[tauri::command]
pub async fn set_auto_update(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let conn = state.db.lock();
    db::set_setting(&conn, "auto_update", if enabled { "true" } else { "false" });
    Ok(())
}
/// 解析当前生效的缓存根目录：用户设置了 `cache_dir` 且非空则用之，否则回落默认 `app_data`
pub(crate) fn resolve_cache_root(state: &AppState) -> std::path::PathBuf {
    let custom = {
        let conn = state.db.lock();
        db::get_setting(&conn, "cache_dir").filter(|s| !s.trim().is_empty())
    };
    match custom {
        Some(dir) => std::path::PathBuf::from(dir),
        None => state.app_data.clone(),
    }
}

/// 路径等值比较（Windows 不区分大小写且统一反斜杠；macOS/Linux 优先 canonicalize）
fn same_dir(a: &std::path::Path, b: &std::path::Path) -> bool {
    let ca = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let cb = std::fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    #[cfg(windows)]
    {
        ca.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .eq_ignore_ascii_case(cb.to_string_lossy().trim_end_matches(['\\', '/']))
    }
    #[cfg(not(windows))]
    {
        ca == cb
    }
}

/// 判断 child 是否位于 parent 内部（防止将缓存目录迁入旧缓存子目录造成递归嵌套）
fn is_subpath(child: &std::path::Path, parent: &std::path::Path) -> bool {
    let cc = std::fs::canonicalize(child).unwrap_or_else(|_| child.to_path_buf());
    let cp = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    #[cfg(windows)]
    {
        let sc = cc.to_string_lossy().to_lowercase();
        let sp = cp.to_string_lossy().to_lowercase();
        let sp_norm = sp.trim_end_matches(['\\', '/']);
        sc == sp_norm || sc.starts_with(&format!("{sp_norm}\\"))
    }
    #[cfg(not(windows))]
    {
        cc.starts_with(&cp)
    }
}

/// 跨盘符/跨文件系统安全迁移单个文件：
/// 1. 优先 std::fs::rename（同盘符零拷贝原子移动）；
/// 2. 跨盘符回退 copy → 临时文件 → rename 原子落盘，再尽力删除源文件；
/// 3. Windows 下若源文件正被音频引擎占用无法删除，已安全拷贝到新目录，返回 `Ok(false)` 表示源文件留待后续清理。
fn move_file_safe(src: &std::path::Path, dst: &std::path::Path) -> Result<bool, String> {
    if dst.exists() {
        // 目标已有同名缓存文件：以目标为准，尽力移除旧文件
        let _ = std::fs::remove_file(src);
        return Ok(true);
    }
    if std::fs::rename(src, dst).is_ok() {
        return Ok(true);
    }
    let tmp = dst.with_extension("rm_migrating");
    std::fs::copy(src, &tmp).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&tmp, dst) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    // 尽量保留原有修改时间（LRU 清理依据）
    if let Ok(meta) = std::fs::metadata(src) {
        if let Ok(mtime) = meta.modified() {
            let _ = filetime::set_file_mtime(dst, filetime::FileTime::from_system_time(mtime));
        }
    }
    let removed = std::fs::remove_file(src).is_ok();
    Ok(removed)
}

/// 迁移缓存子目录（downloads / cover-cache）内的全部完整缓存文件，跳过正在写入的 .part 临时文件
fn migrate_cache_subdir(
    old_dir: &std::path::Path,
    new_dir: &std::path::Path,
) -> Result<(u32, u32), String> {
    std::fs::create_dir_all(new_dir).map_err(|e| {
        crate::i18n::pick(
            format!("无法创建目标缓存目录: {e}"),
            format!("Failed to create target cache directory: {e}"),
        )
    })?;
    if !old_dir.exists() || same_dir(old_dir, new_dir) {
        return Ok((0, 0));
    }
    let mut moved = 0u32;
    let mut skipped = 0u32;
    let rd = match std::fs::read_dir(old_dir) {
        Ok(r) => r,
        Err(_) => return Ok((0, 0)),
    };
    for entry in rd.flatten() {
        let src = entry.path();
        if !src.is_file() {
            continue;
        }
        let ext = src.extension().and_then(|x| x.to_str()).unwrap_or("");
        if ext == "part" || ext == "rm_migrating" {
            skipped += 1;
            continue;
        }
        let dst = new_dir.join(entry.file_name());
        match move_file_safe(&src, &dst) {
            Ok(true) => moved += 1,
            Ok(false) => {
                // 已安全复制到新目录，仅旧文件被系统占用未删掉
                moved += 1;
                skipped += 1;
            }
            Err(_) => skipped += 1,
        }
    }
    Ok((moved, skipped))
}

#[tauri::command]
pub async fn get_cache_dir(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let default_dir = state.app_data.clone();
    let custom = {
        let conn = state.db.lock();
        db::get_setting(&conn, "cache_dir").filter(|s| !s.trim().is_empty())
    };
    let is_custom = custom.is_some();
    let current = custom
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| default_dir.clone());
    Ok(json!({
        "dir": current.to_string_lossy(),
        "defaultDir": default_dir.to_string_lossy(),
        "isCustom": is_custom,
    }))
}

/// 修改缓存目录并自动迁移旧缓存内容（downloads + cover-cache）；
/// `dir` 为 None 或空串时恢复默认应用数据目录。
#[tauri::command]
pub async fn set_cache_dir(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    dir: Option<String>,
) -> Result<serde_json::Value, String> {
    use tauri::Manager;

    let default_root = state.app_data.clone();
    let old_root = resolve_cache_root(&state);
    let trimmed = dir.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let (new_root, is_custom) = match trimmed {
        Some(ref s) => {
            let p = std::path::PathBuf::from(s);
            if same_dir(&p, &default_root) {
                (default_root.clone(), false)
            } else {
                (p, true)
            }
        }
        None => (default_root.clone(), false),
    };

    if same_dir(&old_root, &new_root) {
        return Ok(json!({
            "dir": new_root.to_string_lossy(),
            "defaultDir": default_root.to_string_lossy(),
            "isCustom": is_custom,
            "moved": 0,
            "skipped": 0,
        }));
    }

    // 禁止迁移进旧的 downloads / cover-cache 内部（防递归嵌套）
    let old_dl = old_root.join("downloads");
    let old_cv = old_root.join("cover-cache");
    if is_subpath(&new_root, &old_dl) || is_subpath(&new_root, &old_cv) {
        return Err(crate::i18n::pick(
            "不能将缓存目录设置到当前缓存子目录内部",
            "Cannot set cache folder inside an existing cache subdirectory",
        )
        .into());
    }

    let new_root_task = new_root.clone();
    let (moved, skipped) = super::blocking(move || -> Result<(u32, u32), String> {
        // 1. 创建目标目录并探测写权限（Windows/macOS/Linux 统一防只读/系统保护目录）
        std::fs::create_dir_all(&new_root_task).map_err(|e| {
            crate::i18n::pick(
                format!("无法创建目标目录: {e}"),
                format!("Cannot create target directory: {e}"),
            )
        })?;
        let probe = new_root_task.join(".rustmusic_write_probe");
        std::fs::write(&probe, b"ok").map_err(|e| {
            crate::i18n::pick(
                format!("目标目录不可写，请检查权限: {e}"),
                format!("Target directory is not writable: {e}"),
            )
        })?;
        let _ = std::fs::remove_file(&probe);

        // 2. 安全迁移音频缓存（downloads）与封面缓存（cover-cache）
        let (m1, s1) = migrate_cache_subdir(&old_dl, &new_root_task.join("downloads"))?;
        let (m2, s2) = migrate_cache_subdir(&old_cv, &new_root_task.join("cover-cache"))?;

        // 3. 若旧的自定义子目录已空则顺手清理空文件夹（保留默认 app_data 根目录结构）
        let _ = std::fs::remove_dir(&old_dl);
        let _ = std::fs::remove_dir(&old_cv);

        Ok((m1 + m2, s1 + s2))
    })
    .await?;

    // 4. 放行新封面缓存目录的 Tauri asset:// 访问权限，确保前端封面立即正常渲染
    let _ = app
        .asset_protocol_scope()
        .allow_directory(&new_root.join("cover-cache"), true);

    // 5. 热切换引擎下载缓存目录 + 持久化设置
    engine_clone(&state).set_downloads_dir(new_root.join("downloads"));
    {
        let conn = state.db.lock();
        let val = if is_custom {
            new_root.to_string_lossy().into_owned()
        } else {
            String::new()
        };
        db::set_setting(&conn, "cache_dir", &val);
    }

    Ok(json!({
        "dir": new_root.to_string_lossy(),
        "defaultDir": default_root.to_string_lossy(),
        "isCustom": is_custom,
        "moved": moved,
        "skipped": skipped,
    }))
}

#[tauri::command]
pub async fn clear_cache(state: State<'_, AppState>) -> Result<u32, String> {
    let mut n = engine_clone(&state).clear_cache();
    // 同步清理远程封面磁盘缓存（cover-cache）
    let cover_dir = resolve_cache_root(&state).join("cover-cache");
    if let Ok(rd) = std::fs::read_dir(&cover_dir) {
        for e in rd.flatten() {
            if e.path().is_file() && std::fs::remove_file(e.path()).is_ok() {
                n += 1;
            }
        }
    }
    Ok(n)
}

/// 当前缓存占用（bytes）与文件数（含音频缓存与 cover-cache 远程封面缓存）
#[tauri::command]
pub async fn cache_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let (mut bytes, mut files) = engine_clone(&state).cache_usage();
    let cover_dir = resolve_cache_root(&state).join("cover-cache");
    if let Ok(rd) = std::fs::read_dir(&cover_dir) {
        for e in rd.flatten() {
            if let Ok(meta) = e.metadata() {
                if meta.is_file() {
                    bytes += meta.len();
                    files += 1;
                }
            }
        }
    }
    Ok(json!({ "bytes": bytes, "files": files }))
}

/// 设置缓存上限（bytes，0 = 不限制）；超限立即 LRU 清理
#[tauri::command]
pub async fn set_cache_limit(state: State<'_, AppState>, bytes: u64) -> Result<(), String> {
    {
        let conn = state.db.lock();
        db::set_setting(&conn, "cache_limit", &bytes.to_string());
    }
    engine_clone(&state).set_cache_limit(bytes);
    Ok(())
}
