import { useEffect, useState } from "react";

/**
 * 基于精确 CSS 像素尺寸生成 Apple Liquid Glass 凸透镜折射置换贴图：
 * - 1:1 映射实际 DOM 尺寸（filterUnits="userSpaceOnUse"）
 * - 外圈 bevelPx 倒角区域编码凸透镜法线偏移 (R=ΔX, G=ΔY)，滚动穿过边缘时产生真实弯曲折射
 * - 中央平坦区保持中性灰 (128, 128)，保证文字与控件锐利不扭曲
 */
function buildLensDisplacementDataUrl(
  width: number,
  height: number,
  radius: number,
  bevelPx: number
): string {
  const w = Math.max(32, Math.round(width));
  const h = Math.max(24, Math.round(height));
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) return "";

  const img = ctx.createImageData(w, h);
  const data = img.data;

  const halfW = w * 0.5;
  const halfH = h * 0.5;
  const r = Math.min(radius, halfW - 1, halfH - 1);
  const bx = halfW - r;
  const by = halfH - r;
  const bevel = Math.min(bevelPx, halfH - 2, halfW - 2);

  for (let y = 0; y < h; y++) {
    const py = y + 0.5 - halfH;
    const ay = Math.abs(py);
    const qy = ay - by;

    for (let x = 0; x < w; x++) {
      const px = x + 0.5 - halfW;
      const ax = Math.abs(px);
      const qx = ax - bx;

      let sdf: number;
      let nx = 0;
      let ny = 0;

      if (qx > 0 && qy > 0) {
        const len = Math.hypot(qx, qy);
        sdf = len - r;
        if (len > 1e-4) {
          nx = (qx / len) * Math.sign(px);
          ny = (qy / len) * Math.sign(py);
        }
      } else {
        const m = Math.max(qx, qy);
        sdf = m - r;
        if (qx >= qy) {
          nx = Math.sign(px);
          ny = 0;
        } else {
          nx = 0;
          ny = Math.sign(py);
        }
      }

      const depth = -sdf;
      let dx = 0;
      let dy = 0;

      if (depth >= 0 && depth < bevel) {
        const t = depth / bevel;
        const profile = Math.pow(1 - t, 1.6);
        dx = -nx * profile;
        dy = -ny * profile;
      }

      const idx = (y * w + x) * 4;
      data[idx] = Math.max(0, Math.min(255, Math.round(128 + dx * 127)));
      data[idx + 1] = Math.max(0, Math.min(255, Math.round(128 + dy * 127)));
      data[idx + 2] = 128;
      data[idx + 3] = 255;
    }
  }

  ctx.putImageData(img, 0, 0);
  return canvas.toDataURL("image/png");
}

interface DockLensState {
  width: number;
  height: number;
  mapUrl: string;
}

/**
 * 液态玻璃主题专用轻量级单通道 SVG 凸透镜折射滤镜（仅在 uiTheme === "liquid" 时挂载）。
 * - 单 pass feImage + feDisplacementMap（避免多通道 feColorMatrix/feBlend 触发 CPU 软件光栅化）
 * - 无全局 MutationObserver，仅在 .liquid-dock 尺寸变化时重建置换贴图
 */
export default function LiquidGlassFilters() {
  const [dockLens, setDockLens] = useState<DockLensState | null>(null);

  useEffect(() => {
    const isChromium =
      typeof navigator !== "undefined" &&
      /\b(Chrome|Edg|Chromium)\b/.test(navigator.userAgent);

    if (!isChromium) {
      delete document.documentElement.dataset.lgRefract;
      return;
    }

    let raf = 0;
    let lastW = 0;
    let lastH = 0;
    let ro: ResizeObserver | null = null;

    const rebuild = (w: number, h: number) => {
      const rw = Math.round(w);
      const rh = Math.round(h);
      if (rw < 40 || rh < 24) return;
      if (Math.abs(rw - lastW) <= 2 && Math.abs(rh - lastH) <= 2) return;
      lastW = rw;
      lastH = rh;
      const mapUrl = buildLensDisplacementDataUrl(rw, rh, 26, 20);
      if (mapUrl) {
        setDockLens({ width: rw, height: rh, mapUrl });
        document.documentElement.dataset.lgRefract = "on";
      }
    };

    const bind = () => {
      const el = document.querySelector(".liquid-dock");
      if (!el) return;
      ro?.disconnect();
      ro = new ResizeObserver((entries) => {
        const rect = entries[0]?.contentRect;
        if (!rect) return;
        cancelAnimationFrame(raf);
        raf = requestAnimationFrame(() => rebuild(rect.width, rect.height));
      });
      ro.observe(el);
      const rect = el.getBoundingClientRect();
      rebuild(rect.width, rect.height);
    };

    // 等首帧布局完成后绑定一次，窗口缩放时由 ResizeObserver 自动更新
    raf = requestAnimationFrame(bind);

    return () => {
      cancelAnimationFrame(raf);
      ro?.disconnect();
      delete document.documentElement.dataset.lgRefract;
    };
  }, []);

  if (!dockLens) return null;

  const { width, height, mapUrl } = dockLens;

  return (
    <svg
      aria-hidden="true"
      width="0"
      height="0"
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        width: 0,
        height: 0,
        pointerEvents: "none",
        overflow: "hidden",
      }}
    >
      <defs>
        <filter
          id="lg-refract-dock"
          filterUnits="userSpaceOnUse"
          primitiveUnits="userSpaceOnUse"
          x="0"
          y="0"
          width={width}
          height={height}
          colorInterpolationFilters="sRGB"
        >
          <feImage
            href={mapUrl}
            x="0"
            y="0"
            width={width}
            height={height}
            preserveAspectRatio="none"
            result="lensMap"
          />
          <feDisplacementMap
            in="SourceGraphic"
            in2="lensMap"
            scale="32"
            xChannelSelector="R"
            yChannelSelector="G"
          />
        </filter>
      </defs>
    </svg>
  );
}
