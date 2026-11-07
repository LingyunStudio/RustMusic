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
      onPointerCancel={() => {
        if (!dragging) return;
        setDragging(false);
        onCancel?.();
      }}
      onLostPointerCapture={() => {
        if (!dragging) return;
        setDragging(false);
        onCancel?.();
      }}
    >
      {/* 拟物凹槽滑轨 */}
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

      {/* 拟物车削金属推子旋钮 */}
      {showThumb !== "never" && (
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
          {/* 同心微点凹槽指示 */}
          <span className="w-1.5 h-1.5 rounded-full bg-[var(--accent)] shadow-[0_0_3px_var(--accent)]" />
        </div>
      )}
    </div>
  );
}
