import { useCallback, useRef, useState } from "react";

interface SliderProps {
  value: number;
  max: number;
  onChange?: (v: number) => void;
  onCommit?: (v: number) => void;
  /** 拖动被系统中断（pointercancel）时回调：调用方借此复位拖动期状态 */
  onCancel?: () => void;
  className?: string;
  /** 轨道高度 px */
  thick?: number;
  showThumb?: "always" | "hover" | "never";
  disabled?: boolean;
}

export default function Slider({
  value,
  max,
  onChange,
  onCommit,
  onCancel,
  className = "",
  thick = 4,
  showThumb = "hover",
  disabled = false,
}: SliderProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);
  const [preview, setPreview] = useState(0);

  const display = dragging ? preview : value;
  const pct = max > 0 ? Math.min(1, Math.max(0, display / max)) : 0;

  const posToValue = useCallback(
    (clientX: number) => {
      const el = ref.current;
      if (!el) return 0;
      const rect = el.getBoundingClientRect();
      const ratio = rect.width > 0 ? (clientX - rect.left) / rect.width : 0;
      return Math.min(1, Math.max(0, ratio)) * max;
    },
    [max]
  );

  return (
    <div
      ref={ref}
      className={`group relative flex items-center ${disabled ? "pointer-events-none opacity-50" : "cursor-pointer"} ${className}`}
      onPointerDown={(e) => {
        if (disabled || max <= 0 || e.button !== 0) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        setDragging(true);
        const v = posToValue(e.clientX);
        setPreview(v);
        onChange?.(v);
      }}
      onPointerMove={(e) => {
        if (!dragging) return;
        const v = posToValue(e.clientX);
        setPreview(v);
        onChange?.(v);
      }}
      onPointerUp={(e) => {
        if (!dragging) return;
        setDragging(false);
        const v = posToValue(e.clientX);
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
      {showThumb !== "never" && (
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
      )}
    </div>
  );
}
