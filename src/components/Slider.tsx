import { useRef, useState } from "react";
import { useStore } from "../store";

export interface SliderProps {
  value: number; // 0..1 或当前数值
  min?: number;
  max?: number;
  onChange?: (v: number) => void;
  onCommit?: (v: number) => void;
  /** 拖动中被系统取消时（Alt-Tab / 触摸被中断 / lostpointercapture），通知上层复位拖动态 */
  onCancel?: () => void;
  className?: string;
  thick?: number;
  showThumb?: "always" | "hover" | "never";
}

export default function Slider({
  value,
  min = 0,
  max = 1,
  onChange,
  onCommit,
  onCancel,
  className = "",
  thick = 3,
  showThumb = "hover",
}: SliderProps) {
  const [dragging, setDragging] = useState(false);
  const trackRef = useRef<HTMLDivElement>(null);
  const uiTheme = useStore((s) => s.uiTheme);
  const isLiquid = uiTheme === "liquid";
  const isSkeuo = uiTheme === "skeuo";

  const span = max - min || 1;
  const pct = Math.max(0, Math.min(1, (value - min) / span));

  const calc = (e: React.PointerEvent) => {
    const el = trackRef.current;
    if (!el) return min;
    const r = el.getBoundingClientRect();
    const ratio = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
    return min + ratio * span;
  };

  return (
    <div
      ref={trackRef}
      className={`relative py-2 cursor-pointer select-none group touch-none ${className}`}
      onPointerDown={(e) => {
        // 只响应鼠标左键（button === 0），中键/右键点击不触发拖动
        if (e.button !== 0) return;
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
        setDragging(true);
        const v = calc(e);
        onChange?.(v);
      }}
      onPointerMove={(e) => {
        if (!dragging) return;
        onChange?.(calc(e));
      }}
      onPointerUp={(e) => {
        if (!dragging) return;
        setDragging(false);
        try {
          (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
        } catch {}
        const v = calc(e);
        onChange?.(v);
        onCommit?.(v);
      }}
      // 拖动中 Alt-Tab / 系统手势 / 触摸取消触发 pointercancel（不会来
      // pointerup）：不复位的话 dragging 永久卡 true——之后鼠标不按键划过
      // 也会持续 onChange，scrubbing 卡死导致进度条冻结。中断即放弃预览值，
      // 回落到受控 value（不 commit 中途的拖动）
      onPointerCancel={() => {
        if (!dragging) return;
        setDragging(false);
        onCancel?.();
      }}
      // 兜底：某些取消路径只发 lostpointercapture 不发 pointercancel
      onLostPointerCapture={() => {
        if (!dragging) return;
        setDragging(false);
        onCancel?.();
      }}
    >
      {/* 轨道 */}
      {isSkeuo ? (
        <div
          className="relative w-full rounded-full overflow-hidden transition-all"
          style={{
            height: thick + (dragging ? 2 : 0),
            background: "var(--sk-groove-bg)",
            border: "1px solid var(--sk-groove-border)",
            boxShadow: "var(--sk-groove-shadow)",
          }}
        >
          <div
            className="absolute left-0 top-0 h-full rounded-full transition-[width] duration-75"
            style={{
              width: `${pct * 100}%`,
              background: "linear-gradient(180deg, var(--accent-strong) 0%, var(--accent) 100%)",
              boxShadow: "0 0 8px var(--accent-soft), inset 0 1px 0 rgba(255,255,255,0.4)",
            }}
          />
        </div>
      ) : isLiquid ? (
        <div
          className="relative w-full rounded-full bg-[var(--shade-strong)] border border-[var(--line)] shadow-[inset_0_1px_2px_rgba(0,0,0,0.3)] overflow-hidden transition-all"
          style={{ height: thick + (dragging ? 2 : 0) }}
        >
          <div
            className="absolute left-0 top-0 h-full rounded-full transition-[width] duration-75"
            style={{
              width: `${pct * 100}%`,
              background: "linear-gradient(90deg, var(--accent) 0%, var(--accent-strong) 100%)",
              boxShadow: "0 0 8px var(--accent-soft)",
            }}
          />
        </div>
      ) : (
        <div
          className="relative w-full rounded-full bg-[var(--shade-strong)] overflow-hidden transition-all"
          style={{ height: thick + (dragging ? 2 : 0) }}
        >
          <div
            className="absolute left-0 top-0 h-full rounded-full"
            style={{
              width: `${pct * 100}%`,
              background: "linear-gradient(90deg, #e8823f, var(--accent-strong))",
            }}
          />
        </div>
      )}

      {/* 滑块 Thumb */}
      {showThumb !== "never" &&
        (isSkeuo ? (
          <div
            className={`absolute top-1/2 -translate-y-1/2 -translate-x-1/2 flex items-center justify-center rounded-full transition-all duration-100 ${
              showThumb === "hover" ? "opacity-0 group-hover:opacity-100" : ""
            } ${dragging ? "!opacity-100 scale-110" : ""}`}
            style={{
              left: `${pct * 100}%`,
              width: 15,
              height: 15,
              background: "var(--sk-knob-metal)",
              border: "1px solid var(--sk-edge)",
              boxShadow: "var(--sk-knob-shadow)",
            }}
          >
            <span className="w-1.5 h-1.5 rounded-full bg-[var(--accent)] shadow-[0_0_3px_var(--accent)]" />
          </div>
        ) : isLiquid ? (
          <div
            className="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 rounded-full border border-white/90 shadow-[0_2px_8px_rgba(0,0,0,0.4),0_0_12px_var(--accent)] transition-all pointer-events-none group-hover:opacity-100"
            style={{
              left: `${pct * 100}%`,
              width: dragging ? 15 : 12,
              height: dragging ? 15 : 12,
              background: "radial-gradient(circle at 35% 35%, #ffffff 0%, #f1f5f9 65%, #cbd5e1 100%)",
              opacity: dragging ? 1 : showThumb === "always" ? 1 : 0,
              transform: `translate(-50%, -50%) scale(${dragging ? 1.15 : 1})`,
            }}
          />
        ) : (
          <div
            className="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 rounded-full bg-[var(--accent-strong)] shadow-[0_0_8px_rgba(0,0,0,0.5)] transition-all"
            style={{
              left: `${pct * 100}%`,
              width: dragging ? 13 : 11,
              height: dragging ? 13 : 11,
              opacity: dragging ? 1 : showThumb === "always" ? 1 : 0,
            }}
          />
        ))}
    </div>
  );
}
