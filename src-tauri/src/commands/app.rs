//! 应用杂项：asset 放行 / 皮肤图压缩 / 应用信息 / 封面取色 / 桌面歌词窗口 / 打开链接

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

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
    let app_data = state.app_data.clone();
    super::blocking(move || prepare_skin_image_task(app_data, path)).await
}

/// 同步任务体：整图解码 + 缩放重存是重 IO（4K/8K 图可达上百 MB）
fn prepare_skin_image_task(app_data: std::path::PathBuf, path: String) -> Result<String, String> {
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

    let dir = app_data.join("skins");
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
        .save_with_format(
            &dest,
            if dest.extension().and_then(|e| e.to_str()) == Some("png") {
                image::ImageFormat::Png
            } else {
                image::ImageFormat::Jpeg
            },
        )
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
        // 运行平台（"windows" / "linux" / "macos"）：前端据此决定
        // WASAPI 独占开关、自动更新安装等平台专属 UI 的显隐
        "platform": std::env::consts::OS,
    }))
}
// ---------- 封面取色（服务端，绕过 QQ 封面域无 CORS 的限制） ----------

/// 远程封面图经后端拉取：封面 <img> 不直连 CDN，改走本命令落盘后的
/// asset 协议地址。WebKit 的网络层遵循系统代理设置（GNOME 手动代理指向
/// 127.0.0.1:xxxx），代理进程未运行时所有远程封面加载全部失败（表现为
/// 整列表灰色占位）；后端 ureq 不读系统代理、始终直连，且带磁盘缓存
///（同一封面只下载一次，列表滚动零重复请求）
#[tauri::command]
pub async fn fetch_cover(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let app_data = state.app_data.clone();
    super::blocking(move || fetch_cover_task(app_data, url)).await
}

fn fetch_cover_task(app_data: std::path::PathBuf, url: String) -> Result<String, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("不支持的封面地址".into());
    }
    // 缓存名 = URL 哈希 + 扩展名（重复调用直接命中，进程生命周期内零下载）
    let mut h = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    url.hash(&mut h);
    let stem = format!("{:016x}", h.finish());
    let ext = if url.contains(".png") { "png" } else { "jpg" };

    let dir = app_data.join("cover-cache");
    let _ = std::fs::create_dir_all(&dir);
    let dest = dir.join(format!("{stem}.{ext}"));
    if dest.exists() && dest.metadata().map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(dest.to_string_lossy().into_owned());
    }

    let resp = ureq::get(&url)
        .set("User-Agent", "Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| format!("封面下载失败: {e}"))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .take(8 * 1024 * 1024)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    if buf.len() < 64 {
        return Err("封面内容为空".into());
    }
    std::fs::write(&dest, &buf).map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn extract_cover_palette(url: String) -> Result<Vec<String>, String> {
    super::blocking(move || extract_cover_palette_task(url)).await
}

/// 同步任务体：封面拉取 + 图片解码取色都是阻塞 IO
fn extract_cover_palette_task(url: String) -> Result<Vec<String>, String> {
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
    // 网络抖动会让取色整体失效（播放页丢失自适应配色、歌词回退纯色），
    // 失败重试一次；仍失败返回空数组走前端兜底
    let mut buf: Vec<u8> = Vec::new();
    for _ in 0..2 {
        buf.clear();
        let ok = match ureq::get(&url)
            .set("User-Agent", "Mozilla/5.0")
            .timeout(std::time::Duration::from_secs(12))
            .call()
        {
            Ok(resp) => {
                // 截断的图片（PNG 的 IEND 在尾部）解码必败，上限须远大于真实封面
                resp.into_reader()
                    .take(12 * 1024 * 1024)
                    .read_to_end(&mut buf)
                    .is_ok()
            }
            Err(_) => false,
        };
        if ok && !buf.is_empty() {
            break;
        }
    }
    if buf.is_empty() {
        return Ok(vec![]);
    }
    let img = image::load_from_memory(&buf).map_err(|e| e.to_string())?;
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

/// 几何持久化的防抖信号通道（拖动/缩放期间 Moved/Resized 事件连发，
/// 逐事件直接写库等于每像素一次 SQLite 写）
static DLYRICS_GEOM_TX: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>> =
    std::sync::Mutex::new(None);

/// 立即把桌面歌词窗口的位置尺寸存进设置表（逻辑像素 "x,y,w,h" 四元组）。
/// 几何里存的是"歌词区高度"：实际窗口顶部还有 40px 控制条预留区（前端
/// 常驻，不 hover 时全透明）——保存时减、打开时加，保证歌词区尺寸稳定
pub(crate) fn persist_dlyrics_geom(app: &AppHandle) {
    const CTRL_STRIP: f64 = 40.0;
    let Some(w) = app.get_webview_window("desktop-lyrics") else {
        return;
    };
    let scale = w.scale_factor().unwrap_or(1.0);
    if let (Ok(pos), Ok(size)) = (w.outer_position(), w.inner_size()) {
        let geom = format!(
            "{},{},{},{}",
            pos.x as f64 / scale,
            pos.y as f64 / scale + CTRL_STRIP,
            size.width as f64 / scale,
            ((size.height as f64 / scale) - CTRL_STRIP).max(70.0)
        );
        let st = app.state::<AppState>();
        let conn = st.db.lock();
        let _ = db::set_setting(&conn, "dlyrics_geom", &geom);
    }
}

/// 防抖保存几何：窗口静止 400ms 后落一次盘。托盘退出（app.exit）不走
/// desktop_lyrics_close，拖动/缩放中即时落盘才能保证位置记忆不丢
pub(crate) fn schedule_persist_dlyrics_geom(app: &AppHandle) {
    let mut guard = DLYRICS_GEOM_TX.lock().unwrap();
    if guard.is_none() {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        *guard = Some(tx);
        let app = app.clone();
        std::thread::spawn(move || {
            while rx.recv().is_ok() {
                std::thread::sleep(std::time::Duration::from_millis(400));
                // 静止窗口期内又动过：吞掉旧信号，按最后一次的位置保存
                while rx.try_recv().is_ok() {}
                persist_dlyrics_geom(&app);
            }
        });
    }
    let _ = guard.as_ref().unwrap().send(());
}

/// 打开桌面歌词窗口（透明、无边框、置顶、跳过任务栏）。
/// 已存在则恢复显示并解除锁定残留。窗口加载 desktop-lyrics.html（dev 下走 Vite 端口）。
#[tauri::command]
pub async fn desktop_lyrics_open(app: AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        // 僵尸自愈：底层窗口可能已被 WM 销毁而对象仍挂在注册表（XWayland
        // 快速显隐下发生过）——show 会无声失败，歌词从此打不开。探测窗口
        // 基本属性失败即判死：强销毁清出注册表，走下方全新创建
        if w.outer_position().is_ok() {
            let _ = w.show();
            // re-show 后 WM 可能丢弃 always-on-top：按存的偏好重新断言
            //（真机/虚拟机 mutter 都出现过置顶丢失；读失败则保持创建默认）
            let want_top = {
                let st = app.state::<AppState>();
                let conn = st.db.lock();
                db::get_setting(&conn, "dlyrics_always_top")
            };
            let _ = w.set_always_on_top(want_top.as_deref() != Some("0"));
            // 重开必须解除锁定残留：锁定 = 鼠标穿透（set_ignore_cursor_events），
            // 只 show 不解锁的话得到的是一个永远点不着、也没有控制条的窗口
            //（歌词窗口自身的 locked 状态只认 dlyrics://lock 事件）
            let _ = w.set_ignore_cursor_events(false);
            let _ = app.emit("dlyrics://lock", serde_json::json!({ "locked": false }));
            app.state::<AppState>()
                .dlyrics_locked
                .store(false, std::sync::atomic::Ordering::SeqCst);
            // show 异步生效：稍等再刷托盘文字，否则 is_visible 读到旧值
            let app2 = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                crate::tray::update_dlyrics_tray(&app2);
            });
            return Ok(());
        }
        eprintln!("[dlyrics] 检测到僵尸窗口，销毁后重建");
        let _ = w.destroy();
        // 注销是异步的：稍等片刻避免立刻重建撞同名
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let url = {
        // dev：Vite 服务 desktop-lyrics.html；release：dist 内多页产物。
        // 虚拟机（main.rs 已禁用 WebKit 合成）时 WebView 能输出 alpha，
        // ?transparent=1 让前端切到"纯悬浮文字"形态（无实色黑底）
        let dev = cfg!(debug_assertions);
        let vm_transparent = cfg!(target_os = "linux")
            && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_some();
        let suffix = if vm_transparent { "?transparent=1" } else { "" };
        if dev {
            format!("http://localhost:1420/desktop-lyrics.html{suffix}")
        } else {
            // tauri build 时 frontendDist 已包含 desktop-lyrics.html
            format!("desktop-lyrics.html{suffix}")
        }
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
    let mut builder =
        WebviewWindowBuilder::new(&app, "desktop-lyrics", WebviewUrl::App(url.into()))
            .title("桌面歌词")
            .inner_size(ww, hh + CTRL_STRIP)
            .position(x, (y - CTRL_STRIP).max(0.0))
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(true)
            // 拖拽热区铺满全窗（data-tauri-drag-region）：可最大化窗口双击热区
            // 会被系统当成"双击标题栏"最大化，歌词条铺满全屏显然不是预期
            .maximizable(false)
            // 无最小尺寸可把窗口拖缩到 0：控制条与歌词全被压没
            .min_inner_size(360.0, 150.0)
            // 打开歌词不该从主窗口抢走键盘焦点（空格暂停等快捷键会失灵）
            .focused(false)
            .shadow(false);
    // Linux 真机：不透明窗口 + 前端实色底栏（透明在部分驱动栈上渲染为
    // 全黑/灰块）。虚拟机例外：合成已被禁用（Cairo 路径输出 alpha 正确），
    // 透明窗口实现真正的"悬浮文字"形态
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_some() {
        builder = builder
            .transparent(true)
            .background_color(tauri::utils::config::Color(0, 0, 0, 0));
    }
    // Windows：透明窗口 + WebView2 alpha=0 背景（纯悬浮文字形态）
    #[cfg(windows)]
    {
        builder = builder
            .transparent(true)
            // WebView2 透明：alpha=0 的背景色是 Windows 下真正穿透的关键
            .background_color(tauri::utils::config::Color(0, 0, 0, 0))
            // 与主窗一致的低内存目标（Chromium 更积极回收工作集）；
            // 覆盖式参数需带上 wry 默认禁用的 Edge 附加功能
            .additional_browser_args(
                "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --memory-usage-target-level=low",
            );
    }
    builder
        .build()
        .map_err(|e| format!("创建桌面歌词窗口失败: {e}"))?;
    app.state::<AppState>()
        .dlyrics_locked
        .store(false, std::sync::atomic::Ordering::SeqCst);
    crate::tray::update_dlyrics_tray(&app);
    Ok(())
}

/// 关闭桌面歌词窗口 = 隐藏（不销毁）。销毁/创建循环有竞态：GTK 销毁是异步
/// 的，快速开关时会命中"注册表里还有半死窗口"的窗口期——show 死窗口无声
/// 失效（歌词再也打不开）、再对它 close 直接段错误崩掉整个进程。
/// 隐藏/显示语义下窗口全程存活，无任何竞态；真正销毁只随应用退出发生
#[tauri::command]
pub async fn desktop_lyrics_close(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        persist_dlyrics_geom(&app);
        let _ = w.hide();
    }
    app.state::<AppState>()
        .dlyrics_locked
        .store(false, std::sync::atomic::Ordering::SeqCst);
    crate::tray::update_dlyrics_tray(&app);
    Ok(())
}

/// 解锁桌面歌词（锁定 = set_ignore_cursor_events，穿透后窗口收不到点击，
/// 由主窗口的全局快捷键/设置开关调此命令恢复交互）
#[tauri::command]
pub async fn desktop_lyrics_unlock(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("desktop-lyrics") {
        let _ = w.set_ignore_cursor_events(false);
    }
    app.state::<AppState>()
        .dlyrics_locked
        .store(false, std::sync::atomic::Ordering::SeqCst);
    crate::tray::update_dlyrics_tray(&app);
    Ok(())
}

/// 记录桌面歌词置顶偏好（前端 toggle 后调用；后端 re-show 时据此断言）
#[tauri::command]
pub async fn desktop_lyrics_set_always_top(
    state: State<'_, AppState>,
    on: bool,
) -> Result<(), String> {
    let conn = state.db.lock();
    let _ = db::set_setting(&conn, "dlyrics_always_top", if on { "1" } else { "0" });
    Ok(())
}

/// 桌面歌词窗口是否处于"开启"（显示中）。窗口本体常驻（关闭=隐藏），
/// 主窗口挂起期间校准"词"按钮状态以可见性为准
#[tauri::command]
pub async fn desktop_lyrics_is_open(app: AppHandle) -> Result<bool, String> {
    Ok(app
        .get_webview_window("desktop-lyrics")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false))
}

// ---------- 菜单栏歌词（Linux / GNOME 顶栏） ----------
//
// 应用把当前歌词行写入会话运行时目录的 JSON 文件，GNOME Shell 扩展
//（rustmusic-lyrics@lingyunstudio，随应用内置）轮询显示。与悬浮歌词
// 窗口相互独立，可以单独开启。

/// 菜单栏数据文件：$XDG_RUNTIME_DIR（tmpfs，登录周期自动清理）优先
#[cfg(target_os = "linux")]
fn menubar_file() -> Result<std::path::PathBuf, String> {
    if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") {
        return Ok(std::path::PathBuf::from(rt).join("rustmusic-menubar.json"));
    }
    let home = std::env::var_os("HOME").ok_or("无法定位运行时目录")?;
    Ok(std::path::PathBuf::from(home).join(".cache/rustmusic-menubar.json"))
}

/// 原子写入当前歌词行（rename 替换，扩展侧读不到半截 JSON）。
/// line 为空 = 清除显示；ts 供扩展判断应用是否仍在运行（15s 无更新隐藏）；
/// color 为用户自选顶栏文字色（空 = 主题默认），随每次写入携带
#[cfg(target_os = "linux")]
fn menubar_write_line(line: &str, color: &str) -> Result<(), String> {
    let path = menubar_file()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建运行时目录失败: {e}"))?;
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let payload = serde_json::json!({ "line": line, "ts": ts, "color": color });
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, payload.to_string()).map_err(|e| format!("写入菜单栏歌词失败: {e}"))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("替换菜单栏歌词失败: {e}"))
}

/// 前端在歌词行变化 / 心跳时调用；非 Linux 直接忽略
#[tauri::command]
pub async fn dlyrics_menubar_line(line: String, color: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        super::blocking(move || menubar_write_line(&line, color.as_deref().unwrap_or("")))
            .await?;
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (line, color);
    }
    Ok(())
}

/// 释放内置的 GNOME Shell 扩展到用户扩展目录并写入启用列表。
/// Wayland 下新装的扩展要注销重登一次才加载；返回给前端的提示文案说明这一点
#[cfg(target_os = "linux")]
#[tauri::command]
pub async fn menubar_extension_install() -> Result<String, String> {
    super::blocking(|| {
        const UUID: &str = "rustmusic-lyrics@lingyunstudio";
        let home = std::env::var_os("HOME").ok_or("无法定位主目录")?;
        let dir = std::path::PathBuf::from(home)
            .join(".local/share/gnome-shell/extensions")
            .join(UUID);
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建扩展目录失败: {e}"))?;
        std::fs::write(
            dir.join("metadata.json"),
            include_str!("../../extensions/menubar-lyrics/metadata.json"),
        )
        .map_err(|e| format!("写入扩展元数据失败: {e}"))?;
        std::fs::write(
            dir.join("extension.js"),
            include_str!("../../extensions/menubar-lyrics/extension.js"),
        )
        .map_err(|e| format!("写入扩展脚本失败: {e}"))?;

        // 已启用则不再动 gsettings
        let get_out = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.shell", "enabled-extensions"])
            .output()
            .map_err(|e| format!("读取 GNOME 扩展配置失败（需要 gsettings）: {e}"))?;
        let cur = String::from_utf8_lossy(&get_out.stdout).trim().to_string();
        if cur.contains(UUID) {
            return Ok("扩展已安装并启用，顶栏未显示时请注销并重新登录一次".into());
        }
        // gsettings 数组语法：空列表 "@as []"，非空 "['a', 'b']"
        let new_val = if cur.is_empty() || cur == "@as []" {
            format!("@as ['{UUID}']")
        } else if let Some(stripped) = cur.strip_suffix(']') {
            format!("{stripped}, '{UUID}']")
        } else {
            return Err(format!("GNOME 扩展配置格式异常: {cur}"));
        };
        let set_out = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.shell", "enabled-extensions", &new_val])
            .output()
            .map_err(|e| format!("写入 GNOME 扩展配置失败: {e}"))?;
        if !set_out.status.success() {
            return Err(format!(
                "写入 GNOME 扩展配置失败: {}",
                String::from_utf8_lossy(&set_out.stderr).trim()
            ));
        }
        Ok("已安装并启用。注销并重新登录一次后，顶栏会出现歌词（GNOME 加载新扩展需要重启 Shell）".into())
    })
    .await
}

/// 非 Linux 无 GNOME 顶栏：占位实现（main.rs 的命令注册表是全平台共用的）；
/// 前端入口仅在 Linux 显示，正常不会调用到这里
#[cfg(not(target_os = "linux"))]
#[tauri::command]
pub async fn menubar_extension_install() -> Result<String, String> {
    Err("菜单栏歌词仅支持 Linux（GNOME 顶栏）".into())
}

/// 用系统默认浏览器打开链接（release notes 内的跳转用）。
/// Windows 走 explorer 而不是 `cmd /C start`：URL 未加引号时 cmd 会把 & | ^
/// 当命令分隔符执行（含查询串的链接直接命令注入），explorer 按单个参数透传；
/// Linux 走 xdg-open（浏览器打开是副作用，URL 本身不进 shell 求值）
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("不支持的链接".into());
    }
    #[cfg(windows)]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(not(windows))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(&url)
        .spawn()
        .map_err(|e| format!("打开链接失败：{e}"))?;
    Ok(())
}
