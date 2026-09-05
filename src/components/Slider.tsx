import { useCallback, useRef, useState } from "react";

interface SliderProps {
  value: number;
  max: number;
  onChange?: (v: number) => void;
  onCommit?: (v: number) => void;
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
        if (disabled || max <= 0) return;
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
    >
      <div
        className="relative w-full rounded-full bg-white/[0.12] overflow-hidden transition-all"
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
      {showThumb !== "never" && (
        <div
          className="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 rounded-full bg-[var(--accent-strong)] shadow-[0_0_8px_rgba(0,0,0,0.5)] transition-all"
          style={{
            left: `${pct * 100}%`,
            width: dragging ? 13 : 11,
            height: dragging ? 13 : 11,
            opacity: dragging ? 1 : showThumb === "always" ? 1 : 0,
          }}
        />
      )}
    </div>
  );
}
