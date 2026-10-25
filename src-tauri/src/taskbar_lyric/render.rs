//! 任务栏歌词：DirectWrite/Direct2D 渲染与托盘锚定定位。

use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Brush, ID2D1DCRenderTarget, ID2D1Factory, ID2D1SolidColorBrush,
    D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_FEATURE_LEVEL_DEFAULT,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_TEXT_ANTIALIAS_MODE_DEFAULT,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteFactory2, IDWriteRenderingParams,
    IDWriteTextFormat, IDWriteTextLayout, DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_GRID_FIT_MODE_DEFAULT,
    DWRITE_PIXEL_GEOMETRY_FLAT, DWRITE_RENDERING_MODE_NATURAL, DWRITE_TEXT_METRICS,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, ScreenToClient,
    SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ,
};
use windows::core::w;
use windows_numerics::Vector2;
use super::zone::secondary_zone_left;
use super::*;

/// DWrite/D2D 渲染上下文（渲染线程私有；COM 指针不跨线程使用）
struct RenderCtx {
    dwrite: IDWriteFactory,
    d2d: ID2D1Factory,
    params: IDWriteRenderingParams,
    rt: Option<ID2D1DCRenderTarget>,
    format: Option<(IDWriteTextFormat, i32)>,
    brush: Option<ID2D1SolidColorBrush>,
    brush_color: u32,
}

// COM 指针仅在渲染线程触碰
unsafe impl Send for RenderCtx {}

static CTX: Mutex<Option<RenderCtx>> = Mutex::new(None);

/// 初始化渲染上下文（mod 的 run 阶段调用）
pub(super) fn init() {
    unsafe {
        match init_ctx() {
            Ok(ctx) => *CTX.lock() = Some(ctx),
            Err(e) => eprintln!("[taskbar-lyric] 渲染上下文初始化失败: {e}"),
        }
    }
}
/// DWrite/D2D 工厂与默认渲染参数（伽马 2.2 + 对比度增强：
/// 与 ClearType 的视觉粗细对齐，避免 GDI 灰度路径的"发虚"观感）
unsafe fn init_ctx() -> windows::core::Result<RenderCtx> {
    let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
    let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
    let f2: IDWriteFactory2 = dwrite.cast()?;
    let params = f2
        .CreateCustomRenderingParams(
            2.2,
            1.0,
            1.0,
            0.0,
            DWRITE_PIXEL_GEOMETRY_FLAT,
            DWRITE_RENDERING_MODE_NATURAL,
            DWRITE_GRID_FIT_MODE_DEFAULT,
        )?
        .cast::<IDWriteRenderingParams>()?;
    Ok(RenderCtx {
        dwrite,
        d2d,
        params,
        rt: None,
        format: None,
        brush: None,
        brush_color: 0,
    })
}


/// 渲染一行歌词并更新图层窗口位置/大小。
/// 主屏：锚定 TrayNotifyWnd（托盘区）左侧；副屏（无托盘区窗口）：
/// 新版 Win11 副屏右缘是镜像的系统时钟，改为紧跟应用图标区（WorkerW）右侧。
/// 竖排任务栏：停靠侧对侧、底部对齐
pub(super) unsafe fn render_line(hwnd: HWND, tray: HWND, text: &str, color: COLORREF) {
    let mut guard = CTX.lock();
    static WARNED: AtomicBool = AtomicBool::new(false);
    let Some(ctx) = guard.as_mut() else {
        if !WARNED.swap(true, Ordering::Relaxed) {
            eprintln!("[taskbar-lyric] 渲染上下文未就绪，歌词不可用");
        }
        return;
    };

    let mut tray_r = RECT::default();
    let _ = GetWindowRect(tray, &mut tray_r);
    let taskbar_w = tray_r.right - tray_r.left;
    let taskbar_h = tray_r.bottom - tray_r.top;
    if taskbar_w <= 0 || taskbar_h <= 0 {
        return;
    }
    let font_px = ((taskbar_h as f32 * 0.34).clamp(12.0, 20.0)).round() as i32;
    let Some(format) = ensure_format(ctx, font_px) else {
        return;
    };

    // 测量与截断（DirectWrite 排版，超宽二分 + 省略号）
    let wide: Vec<u16> = text.encode_utf16().collect();
    let max_w = ((taskbar_w * 35 / 100).max(80)).min(560) as f32;
    let max_h = taskbar_h as f32 * 2.0;
    let Some((layout, text_w, line_h)) = fit_text(ctx, &format, &wide, max_w, max_h) else {
        return;
    };
    let pad = 4i32;
    let draw_w = (text_w.ceil() as i32).max(1);
    let w = draw_w + pad * 2;
    let h = (line_h.ceil() as i32) + 2 * scale(taskbar_h, 2);
    if w <= pad * 2 || h <= 0 {
        return;
    }

    // 锚点（子窗口坐标 = 任务栏客户区坐标）
    let (x, y) = anchor_point(tray, &tray_r, taskbar_w, taskbar_h, w, h);

    let Some(buf) = FrameBuf::new(w, h) else {
        return;
    };
    let Some(rt) = ensure_rt(ctx, buf.dc, w, h) else {
        return;
    };
    let Some(brush) = ensure_brush(ctx, &rt, color) else {
        return;
    };
    rt.BeginDraw();
    // BeginDraw 后背景为透明黑（Clear 冗余但显式表达意图）
    let _ = rt.Clear(None);
    let origin = Vector2 {
        X: pad as f32,
        Y: (h as f32 - line_h) / 2.0,
    };
    let Ok(brush_base) = brush.cast::<ID2D1Brush>() else {
        return;
    };
    rt.DrawTextLayout(origin, &layout, &brush_base, D2D1_DRAW_TEXT_OPTIONS_NONE);
    if let Err(e) = rt.EndDraw(None, None) {
        eprintln!("[taskbar-lyric] D2D 绘制失败: {e}");
        return;
    }

    let pos = POINT { x, y };
    let size = SIZE { cx: w, cy: h };
    let src = POINT { x: 0, y: 0 };
    let screen_dc = GetDC(Some(tray));
    let blend = windows::Win32::Graphics::Gdi::BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let res = UpdateLayeredWindow(
        hwnd,
        Some(screen_dc),
        Some(&pos),
        Some(&size),
        Some(buf.dc),
        Some(&src),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );
    let _ = ReleaseDC(Some(tray), screen_dc);
    if let Err(e) = res {
        eprintln!("[taskbar-lyric] UpdateLayeredWindow 失败: {e}");
        return;
    }
    let _ = ShowWindow(hwnd, SHOW_WINDOW_CMD(SW_SHOWNOACTIVATE.0));
    // 保持位于任务栏其它子窗口（XAML 岛）之上；开销可忽略，每次渲染顺手断言
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOP),
        0,
        0,
        0,
        0,
        SET_WINDOW_POS_FLAGS(SWP_NOACTIVATE.0 | SWP_NOMOVE.0 | SWP_NOSIZE.0),
    );
}

/// 锚点计算：横排主屏 = 托盘区左缘 - 宽 - 间距；横排副屏 = 图标区右缘 + 间距；
/// 竖排 = 停靠侧对侧、底部对齐
unsafe fn anchor_point(
    tray: HWND,
    tray_r: &RECT,
    taskbar_w: i32,
    taskbar_h: i32,
    w: i32,
    h: i32,
) -> (i32, i32) {
    let gap = scale(taskbar_h, 8);
    let inset = scale(taskbar_h, 4);
    if taskbar_w < taskbar_h {
        // 竖排：停靠侧对侧、底部对齐
        let docked_left = tray_r.left <= 0;
        let x = if docked_left { taskbar_w - w - gap } else { gap };
        return (x.max(inset), taskbar_h - h - gap);
    }
    if let Some(tn) = find_tray_notify(tray) {
        // 主屏（有托盘区窗口）：托盘区左缘
        let mut tn_r = RECT::default();
        let _ = GetWindowRect(tn, &mut tn_r);
        let mut pt = POINT { x: tn_r.left, y: 0 };
        let _ = ScreenToClient(tray, &mut pt);
        return ((pt.x - w - gap).max(inset), (taskbar_h - h) / 2);
    }
    // 副屏：右缘是镜像的系统时钟（XAML 绘制，无锚定窗口；图标区宿主
    // WorkerW 的矩形不随 XAML 布局更新，不可作为锚点）。
    // 用 UIA 实测时钟元素左缘，歌词紧贴其左侧（仅留常规间距）；
    // 测量在后台线程进行（实测一次可达数秒，绝不能阻塞渲染线程），
    // 未落地前按固定预留回落（贴近实测结果，落位几乎无跳动）
    let zone_screen_x = secondary_zone_left(tray, tray_r)
        .unwrap_or_else(|| tray_r.right - scale(taskbar_h, 96));
    let mut pt = POINT { x: zone_screen_x, y: 0 };
    let _ = ScreenToClient(tray, &mut pt);
    let x = (pt.x - w - gap).min(taskbar_w - w - inset);
    (x.max(inset), (taskbar_h - h) / 2)
}

pub(super) fn scale(taskbar_h: i32, v: i32) -> i32 {
    (v as f32 * taskbar_h as f32 / 48.0).round() as i32
}

/// 字体格式缓存：任务栏高度（近似 DPI）变化时重建
unsafe fn ensure_format(ctx: &mut RenderCtx, font_px: i32) -> Option<IDWriteTextFormat> {
    if let Some((f, px)) = &ctx.format {
        if *px == font_px {
            return Some(f.clone());
        }
    }
    let f = ctx
        .dwrite
        .CreateTextFormat(
            w!("Microsoft YaHei UI"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            font_px as f32,
            w!(""),
        )
        .ok()?;
    ctx.format = Some((f.clone(), font_px));
    Some(f)
}

/// DC 渲染目标缓存：每帧重新 BindDC（DIB 尺寸随文本变化）
unsafe fn ensure_rt(ctx: &mut RenderCtx, dc: HDC, w: i32, h: i32) -> Option<ID2D1DCRenderTarget> {
    if ctx.rt.is_none() {
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            dpiX: 96.0, // DIP == 物理像素
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        match ctx.d2d.CreateDCRenderTarget(&props) {
            Ok(rt) => {
                let _ = rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_DEFAULT);
                let _ = rt.SetTextRenderingParams(Some(&ctx.params));
                ctx.rt = Some(rt);
            }
            Err(e) => {
                eprintln!("[taskbar-lyric] 创建 D2D 渲染目标失败: {e}");
                return None;
            }
        }
    }
    let rt = ctx.rt.as_ref()?;
    let rect = RECT { left: 0, top: 0, right: w, bottom: h };
    if rt.BindDC(dc, &rect).is_err() {
        return None;
    }
    Some(rt.clone())
}

/// 画刷缓存：颜色变化时重建
unsafe fn ensure_brush(
    ctx: &mut RenderCtx,
    rt: &ID2D1DCRenderTarget,
    color: COLORREF,
) -> Option<ID2D1SolidColorBrush> {
    if ctx.brush.is_none() || ctx.brush_color != color.0 {
        let c = D2D1_COLOR_F {
            r: (color.0 & 0xFF) as f32 / 255.0,
            g: ((color.0 >> 8) & 0xFF) as f32 / 255.0,
            b: ((color.0 >> 16) & 0xFF) as f32 / 255.0,
            a: 1.0,
        };
        match rt.CreateSolidColorBrush(&c, None) {
            Ok(b) => {
                ctx.brush = Some(b.clone());
                ctx.brush_color = color.0;
                return Some(b);
            }
            Err(e) => {
                eprintln!("[taskbar-lyric] 创建画刷失败: {e}");
                return None;
            }
        }
    }
    ctx.brush.clone()
}

/// 排版一帧文本：自然宽度内直接用；超宽二分最长前缀 + "…"。
/// 返回（待绘制布局, 实际宽度 DIP, 行高 DIP）
unsafe fn fit_text(
    ctx: &RenderCtx,
    format: &IDWriteTextFormat,
    text: &[u16],
    max_w: f32,
    max_h: f32,
) -> Option<(IDWriteTextLayout, f32, f32)> {
    let layout = ctx.dwrite.CreateTextLayout(text, format, max_w, max_h).ok()?;
    let mut m = DWRITE_TEXT_METRICS::default();
    layout.GetMetrics(&mut m).ok()?;
    if m.width <= max_w {
        return Some((layout, m.width, m.height));
    }
    let ell: Vec<u16> = "…".encode_utf16().collect();
    let mut lo = 0usize;
    let mut hi = text.len();
    let mut best: Option<(IDWriteTextLayout, f32, f32)> = None;
    while lo <= hi {
        let mid = (lo + hi) / 2;
        let mut cand = text[..mid].to_vec();
        cand.extend_from_slice(&ell);
        if let Ok(l) = ctx.dwrite.CreateTextLayout(&cand, format, max_w, max_h) {
            let mut cm = DWRITE_TEXT_METRICS::default();
            if l.GetMetrics(&mut cm).is_ok() && cm.width <= max_w {
                best = Some((l, cm.width, cm.height));
                lo = mid + 1;
                continue;
            }
        }
        if mid == 0 {
            break;
        }
        hi = mid - 1;
    }
    best.or(Some((layout, max_w, m.height)))
}

/// 解析 "#rgb" / "#rrggbb" / "#rrggbbaa"；空/非法 → 默认色（随任务栏深浅）
pub(super) fn resolve_color(color: Option<&str>) -> COLORREF {
    let Some(c) = color.map(str::trim).filter(|c| !c.is_empty()) else {
        return default_color();
    };
    let hex = c.strip_prefix('#').unwrap_or(c);
    let ok = !hex.is_empty() && hex.len() <= 8 && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    if !ok {
        return default_color();
    }
    let v = u32::from_str_radix(hex, 16).unwrap_or(0);
    COLORREF(match hex.len() {
        3 => {
            let (r, g, b) = ((v >> 8) & 0xF, (v >> 4) & 0xF, v & 0xF);
            (r * 17) | ((g * 17) << 8) | ((b * 17) << 16)
        }
        6 => ((v & 0xFF) << 16) | (v & 0xFF00) | ((v >> 16) & 0xFF),
        // 带透明度的 #rrggbbaa：忽略 alpha（任务栏文字应不透明），取前 6 位
        _ => {
            let v = v >> 8;
            ((v & 0xFF) << 16) | (v & 0xFF00) | ((v >> 16) & 0xFF)
        }
    })
}

/// 默认文字色：读系统主题（任务栏跟随系统深浅色），浅色 → 深字，深色 → 白字
fn default_color() -> COLORREF {
    let mut light: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let light_theme = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut light as *mut u32 as *mut _),
            Some(&mut size),
        )
        .is_ok()
    };
    COLORREF(if light_theme && light != 0 { 0x000000 } else { 0xFFFFFF })
}

/// 渲染缓冲：内存 DC + 32bpp top-down DIB，drop 时释放全部 GDI 资源。
/// 像素由 DirectWrite/D2D 以预乘 ARGB 写入，与 ULW 的合成语义一致
struct FrameBuf {
    dc: HDC,
    bmp: HBITMAP,
    old: HGDIOBJ,
}

impl FrameBuf {
    unsafe fn new(w: i32, h: i32) -> Option<Self> {
        let dc = CreateCompatibleDC(None);
        if dc.is_invalid() {
            return None;
        }
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = w;
        bi.bmiHeader.biHeight = -h; // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB.0;
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bmp = match CreateDIBSection(Some(dc), &bi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[taskbar-lyric] CreateDIBSection 失败: {e}");
                let _ = DeleteDC(dc);
                return None;
            }
        };
        let old = SelectObject(dc, HGDIOBJ(bmp.0));
        Some(Self { dc, bmp, old })
    }
}

impl Drop for FrameBuf {
    fn drop(&mut self) {
        unsafe {
            let _ = SelectObject(self.dc, self.old);
            let _ = DeleteObject(HGDIOBJ(self.bmp.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

// ---------- 显示器列表（供设置界面选择） ----------
