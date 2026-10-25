//! 轻量下拉选择（替代原生 <select>）。
//! 原生 select 在 WebKitGTK 下闭态画 GTK 主题底色、开态是无样式白框，
//! 与应用玻璃风不统一；这里用 Portal 弹出玻璃风列表，两端渲染一致。
//! 交互：点击选择、点外部/Esc 关闭、打开时选中项滚动到可视区。
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown } from "lucide-react";

export interface SelectOption {
  value: string;
  label: string;
}

export function Select({
  value,
  options,
  onChange,
  disabled,
  className = "",
}: {
  value: string;
  options: SelectOption[];
  onChange: (v: string) => void;
  disabled?: boolean;
  className?: string;
}) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{
    left: number;
    top: number;
    width: number;
  } | null>(null);
  const btnRef = useRef<HTMLButtonElement>(null);
  const popRef = useRef<HTMLDivElement>(null);

  const selected = options.find((o) => o.value === value);

  const toggle = () => {
    if (disabled) return;
    if (!open && btnRef.current) {
      // 依触发器定位：默认向下展开，下方空间不足则向上
      const r = btnRef.current.getBoundingClientRect();
      const estH = Math.min(options.length * 34 + 12, 260);
      const up = window.innerHeight - r.bottom < estH + 8 && r.top > estH + 8;
      setPos({
        left: r.left,
        top: up ? r.top - estH - 6 : r.bottom + 6,
        width: r.width,
      });
    }
    setOpen(!open);
  };

  // 点外部 / Esc 关闭
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (popRef.current?.contains(t) || btnRef.current?.contains(t)) return;
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  // 打开后把选中项滚到可视区；窗口尺寸变化时收起（重开重新定位）
  useEffect(() => {
    if (!open) return;
    popRef.current
      ?.querySelector('[data-selected="true"]')
      ?.scrollIntoView({ block: "nearest" });
    const close = () => setOpen(false);
    window.addEventListener("resize", close);
    return () => window.removeEventListener("resize", close);
  }, [open, pos]);

  return (
    <>
      <button
        ref={btnRef}
        type="button"
        disabled={disabled}
        title={selected?.label}
        onClick={toggle}
        className={`h-9 rounded-lg bg-[var(--glass-bg-strong)] border border-[var(--line)] pl-2.5 pr-3 text-[12.5px] text-[var(--ink)] outline-none flex items-center justify-between gap-2 text-left transition-colors hover:border-[var(--ink-3)] disabled:opacity-60 ${className}`}
      >
        <span className="truncate">{selected?.label ?? ""}</span>
        <ChevronDown
          size={13}
          className={`shrink-0 text-[var(--ink-2)] transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>
      {open &&
        pos &&
        createPortal(
          <div
            ref={popRef}
            className="fixed z-[75] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu overflow-y-auto"
            style={{ left: pos.left, top: pos.top, width: pos.width, maxHeight: 260 }}
            onMouseDown={(e) => e.stopPropagation()}
          >
            {options.map((o) => {
              const isSel = o.value === value;
              return (
                <button
                  key={o.value}
                  type="button"
                  data-selected={isSel}
                  className={`w-full h-8 px-2.5 rounded-lg flex items-center gap-2 text-[12.5px] text-left ${
                    isSel
                      ? "text-[var(--accent-strong)] bg-[var(--accent-weak)] font-medium"
                      : "text-[var(--ink)] hover:bg-[var(--shade-strong)]"
                  }`}
                  onClick={() => {
                    onChange(o.value);
                    setOpen(false);
                  }}
                >
                  <span className="truncate flex-1">{o.label}</span>
                  {isSel && <Check size={13} className="shrink-0" />}
                </button>
              );
            })}
          </div>,
          document.body
        )}
    </>
  );
}
