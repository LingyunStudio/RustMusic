//! 应用杂项：asset 放行 / 皮肤图压缩 / 应用信息 / 封面取色 / 桌面歌词窗口 / 打开链接

use serde_json::json;
use tauri::{AppHandle, Manager, State};

use crate::db;
use crate::AppState;
use std::io::Read;
/// 运行时放行 asset 协议路径：自定义皮肤图片可能在用户目录任意位置
#[tauri::command]
pub async fn asset_scope_allow(app: AppHandle, path: String) -> Result<(), String> {
    app.asset_protocol_scope()
        .allow_file(path)
        .map_err(|e| e.to_string())
}

/// 自定义皮肤图的展示用压缩副本：整图解码后按屏幕级尺寸（≤1920px）重存，
/// 避免原始分辨率（4K/8K 壁纸解码可达数十至上百 MB）常驻渲染进程图片缓存。
/// 动图（gif）与已 ≤1920px 的图原样返回；任何失败都回退原路径，不影响选图
#[tauri::command]
pub async fn prepare_skin_image(
    state: State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    let lower = path.to_lowercase();
    if lower.ends_with(".gif") {
        return Ok(path);
    }
    let src = std::path::Path::new(&path);
    let (mtime, size) = {
        let md = std::fs::metadata(src).map_err(|e| e.to_string())?;
        (
            md.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            md.len(),
        )
    };
    // 目标名绑定来源路径 + 内容版本：换图/改图后不会读到旧压缩副本
    let mut h = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    path.hash(&mut h);
    mtime.hash(&mut h);
    size.hash(&mut h);
    let stem = format!("{:016x}", h.finish());

    let dir = state.app_data.join("skins");
    let _ = std::fs::create_dir_all(&dir);

    // 已有副本直接复用（重复选同一张图不重复解码）
    let hit = dir.join(format!("{stem}.png"));
    if hit.exists() {
        return Ok(hit.to_string_lossy().into_owned());
    }
    let hit_jpg = dir.join(format!("{stem}.jpg"));
    if hit_jpg.exists() {
        return Ok(hit_jpg.to_string_lossy().into_owned());
    }

    // 先廉价读尺寸：≤1920 的图不做整图解码，直接原样返回
    let (w, h) = image::ImageReader::open(src)
        .ok()
        .and_then(|r| r.with_guessed_format().ok())
        .and_then(|r| r.into_dimensions().ok())
        .ok_or_else(|| "读取图片信息失败".to_string())?;
    if w <= 1920 && h <= 1920 {
        return Ok(path);
    }

    let img = image::open(src).map_err(|e| e.to_string())?;
    let small = img.thumbnail(1920, 1920);
    // 带透明通道的图存 PNG 保持透明；照片存 JPEG 体积小
    let dest = if small.color().has_alpha() {
        hit
    } else {
        hit_jpg
    };
    small
        .save_with_format(&dest, if dest.extension().and_then(|e| e.to_str()) == Some("png") {
            image::ImageFormat::Png
        } else {
            image::ImageFormat::Jpeg
        })
        .map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}
#[tauri::command]
pub async fn get_app_info(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    Ok(json!({
        "version": app.package_info().version.to_string(),
        "dataDir": state.app_data.to_string_lossy(),
    }))
}
// ---------- 封面取色（服务端，绕过 QQ 封面域无 CORS 的限制） ----------

#[tauri::command]
pub async fn extract_cover_palette(url: String) -> Result<Vec<String>, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Ok(vec![]);
    }
    // 网易云 CDN 支持 ?param=WxH 出缩略图：库存的原图（1500px PNG）可达
    // 3~4MB，超过读取上限被截断后解码必然失败 → 取色静默失效。
    // 主动要一张 350px 小图，既小又完整。
    let url = if url.contains("music.126.net") && !url.contains("param=") {
        format!("{url}?param=350y350")
    } else {
        url
    };
    let bytes = match ureq::get(&url)
        .set("User-Agent", "Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(10))
        .call()
    {
        Ok(resp) => {
            let mut buf = Vec::new();
            // 截断的图片（PNG 的 IEND 在尾部）解码必败，上限须远大于真实封面
            resp.into_reader()
                .take(12 * 1024 * 1024)
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            buf
        }
        Err(_) => return Ok(vec![]),
    };
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let rgb = img.thumbnail(36, 36).to_rgb8();
    // 色相分桶（与前端算法一致），输出 hsl
    let mut buckets: std::collections::BTreeMap<i64, (f64, f64, f64, f64)> =
        std::collections::BTreeMap::new();
    for p in rgb.pixels() {
        let (r, g, b) = (
            p[0] as f64 / 255.0,
            p[1] as f64 / 255.0,
            p[2] as f64 / 255.0,
        );
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let lum = r * 0.3 + g * 0.6 + b * 0.1;
        if lum < 0.06 || lum > 0.97 {
            continue;
        }
        let sat = if max == 0.0 { 0.0 } else { (max - min) / max };
        let d = max - min;
        let mut hue = 0.0;
        if d != 0.0 {
            if max == r {
                hue = ((g - b) / d) % 6.0;
            } else if max == g {
                hue = (b - r) / d + 2.0;
            } else {
                hue = (r - g) / d + 4.0;
            }
            hue *= 60.0;
            if hue < 0.0 {
                hue += 360.0;
            }
        }
        let bucket = (hue / 45.0).floor() as i64;
        let w = 0.4 + sat;
        let e = buckets.entry(bucket).or_insert((0.0, 0.0, 0.0, 0.0));
        e.0 += r * w;
        e.1 += g * w;
        e.2 += b * w;
        e.3 += w;
    }
    let mut colors: Vec<(f64, f64, f64, f64)> = buckets.into_values().collect();
    colors.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    for (r, g, b, _) in colors.iter().take(4) {
        let (r, g, b) = (*r, *g, *b);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        let d = max - min;
        let mut h = 0.0;
        let mut s = 0.0;
        if d != 0.0 {
            s = d / (1.0 - (2.0 * l - 1.0).abs());
            if max == r {
                h = ((g - b) / d) % 6.0;
            } else if max == g {
                h = (b - r) / d + 2.0;
            } else {
                h = (r - g) / d + 4.0;
            }
            h *= 60.0;
            if h < 0.0 {
                h += 360.0;
            }
        }
        let s2 = (s.max(0.55) * 1.1).min(1.0);
        let l_out = (l.max(0.66)).min(0.85);
        out.push(format!(
            "hsl({}, {:.0}%, {:.0}%)",
            h.round() as i64,
            s2 * 100.0,
            l_out * 100.0
        ));
    }
    Ok(out)
}
// ---------- 桌面歌词窗口 ----------

/// 打开桌面歌词窗口（透明、无边框、置顶、跳过任务栏）。
/// 已存在则仅显示与聚焦。窗口加载 desktop-lyrics.html（dev 下走 Vite 端口）。
#[tauri::command]
pub async fn desktop_lyrics_open(app: AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        let _ = w.show();
        return Ok(());
    }
    let url = {
        // dev：Vite 服务 desktop-lyrics.html；release：dist 内多页产物
        let dev = cfg!(debug_assertions);
        let base = if dev {
            "http://localhost:1420/desktop-lyrics.html".to_string()
        } else {
            // tauri build 时 frontendDist 已包含 desktop-lyrics.html
            "desktop-lyrics.html".to_string()
        };
        base
    };
    let (w, h) = (800.0f64, 110.0f64);
    // 位置记忆：上次关闭时保存的 geometry（逻辑像素），无记录则默认主屏
    // 水平居中、垂直 82% 处（不挡任务栏）
    let default_pos = || {
        app.primary_monitor()
            .ok()
            .flatten()
            .map(|m| {
                let s = m.size();
                let sc = m.scale_factor();
                let (sw, sh) = (s.width as f64 / sc, s.height as f64 / sc);
                ((sw - w) / 2.0, sh * 0.82 - h / 2.0)
            })
            .unwrap_or((120.0, 640.0))
    };
    // "x,y,w,h" 四元组（逻辑像素）。
    // 注意：几何里存的是“歌词区高度”；实际窗口还要加上顶部 40px 的
    // 控制条预留区（前端常驻，不 hover 时全透明）——打开加、关闭减，
    // 保证歌词区始终是用户设定的高度，控制条不挤占歌词空间
    const CTRL_STRIP: f64 = 40.0;
    let saved = {
        let st = app.state::<AppState>();
        let conn = st.db.lock();
        db::get_setting(&conn, "dlyrics_geom")
    };
    let (x, y, ww, hh) = saved
        .and_then(|s| {
            let p: Vec<f64> = s.split(',').filter_map(|v| v.parse().ok()).collect();
            (p.len() == 4).then_some((p[0], p[1], p[2], p[3]))
        })
        .map(|(x, y, ww, hh)| (x, y, ww.max(320.0), hh.max(70.0)))
        .unwrap_or_else(|| {
            let (x, y) = default_pos();
            (x, y, w, h)
        });
    let builder = WebviewWindowBuilder::new(&app, "desktop-lyrics", WebviewUrl::App(url.into()))
        .title("桌面歌词")
        .inner_size(ww, hh + CTRL_STRIP)
        .position(x, (y - CTRL_STRIP).max(0.0))
        .decorations(false)
        .transparent(true)
        // WebView2 透明：alpha=0 的背景色是 Windows 下真正穿透的关键
        .background_color(tauri::utils::config::Color(0, 0, 0, 0))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .shadow(false);
    builder
        .build()
        .map_err(|e| format!("创建桌面歌词窗口失败: {e}"))?;
    Ok(())
}

/// 关闭桌面歌词窗口（无窗口时静默成功）；关闭前把位置尺寸存进设置表
#[tauri::command]
pub async fn desktop_lyrics_close(app: AppHandle) -> Result<(), String> {
    // 与 desktop_lyrics_open 对应：窗口高度含 40px 控制条预留区，
    // 保存几何时减掉，保证下次打开歌词区高度不变
    const CTRL_STRIP: f64 = 40.0;
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        // 几何持久化（逻辑像素四元组）
        let scale = w.scale_factor().unwrap_or(1.0);
        if let (Ok(pos), Ok(size)) = (w.outer_position(), w.inner_size()) {
            let geom = format!(
                "{},{},{},{}",
                (pos.x as f64 / scale),
                (pos.y as f64 / scale) + CTRL_STRIP,
                size.width as f64 / scale,
                ((size.height as f64 / scale) - CTRL_STRIP).max(70.0)
            );
            let st = app.state::<AppState>();
            let conn = st.db.lock();
            let _ = db::set_setting(&conn, "dlyrics_geom", &geom);
        }
        let _ = w.close();
    }
    Ok(())
}

/// 解锁桌面歌词（锁定 = set_ignore_cursor_events，穿透后窗口收不到点击，
/// 由主窗口的全局快捷键/设置开关调此命令恢复交互）
#[tauri::command]
pub async fn desktop_lyrics_unlock(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        let _ = w.set_ignore_cursor_events(false);
    }
    Ok(())
}

/// 桌面歌词窗口是否还开着。主窗口挂起期间歌词窗口可能被直接关闭
///（关闭事件丢失），恢复后查询本命令校准“词”按钮状态
#[tauri::command]
pub async fn desktop_lyrics_is_open(app: AppHandle) -> Result<bool, String> {
    Ok(app.get_webview_window("desktop-lyrics").is_some())
}
/// 用系统默认浏览器打开链接（release notes 内的跳转用）
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("不支持的链接".into());
    }
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("打开链接失败：{e}"))?;
    Ok(())
}
