import { ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useStore } from "../store";

export interface Option<T extends string | number> {
  value: T;
  label: string;
}

export function Select<T extends string | number>({
  value,
  options,
  onChange,
  className = "",
  disabled = false,
}: {
  value: T;
  options: Option<T>[];
  onChange: (v: T) => void;
  className?: string;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState({ left: 0, top: 0, width: 0 });
  const btnRef = useRef<HTMLButtonElement>(null);
  const popRef = useRef<HTMLDivElement>(null);
  const uiTheme = useStore((s) => s.uiTheme);
  const isLiquid = uiTheme === "liquid";

  const selected = options.find((o) => o.value === value) ?? options[0];

  // 计算浮层位置（右缘防出屏、底边贴按钮下方）
  const updatePos = () => {
    const el = btnRef.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const w = Math.max(r.width, 160);
    const left = Math.min(Math.max(8, r.left), window.innerWidth - w - 8);
    // 若贴底空间不足则向上弹
    const estH = Math.min(options.length * 32 + 12, 260);
    const placeAbove = r.bottom + 4 + estH > window.innerHeight && r.top - 4 - estH > 0;
    const top = placeAbove ? r.top - 4 - estH : r.bottom + 4;
    setPos({ left, top, width: w });
  };

  const toggle = () => {
    if (disabled) return;
    if (!open) updatePos();
    setOpen((v) => !v);
  };

  // 点击外部收起
  useEffect(() => {
    if (!open) return;
    const onDocDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (btnRef.current?.contains(target) || popRef.current?.contains(target)) return;
      setOpen(false);
    };
    const onScroll = () => updatePos();
    const onResize = () => updatePos();
    window.addEventListener("mousedown", onDocDown);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("mousedown", onDocDown);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
    };
  }, [open, options.length]);

  return (
    <>
      <button
        ref={btnRef}
        type="button"
        disabled={disabled}
        title={selected?.label}
        onClick={toggle}
        className={
          isLiquid
            ? `h-9 rounded-xl liquid-input pl-3 pr-3 text-[12.5px] text-[var(--ink)] outline-none flex items-center justify-between gap-2 text-left transition-all hover:border-[var(--liquid-glass-border-specular)] disabled:opacity-60 cursor-pointer ${className}`
            : `h-9 rounded-lg bg-[var(--glass-bg-strong)] border border-[var(--line)] pl-2.5 pr-3 text-[12.5px] text-[var(--ink)] outline-none flex items-center justify-between gap-2 text-left transition-colors hover:border-[var(--ink-3)] disabled:opacity-60 ${className}`
        }
      >
        <span className="truncate">{selected?.label ?? ""}</span>
        <ChevronDown
          size={14}
          className={`text-[var(--ink-3)] shrink-0 transition-transform ${open ? "rotate-180" : ""}`}
        />
      </button>

      {open &&
        createPortal(
          <div
            ref={popRef}
            className={`fixed z-[75] glass-strong p-1.5 shadow-2xl anim-menu overflow-y-auto ${
              isLiquid ? "!rounded-2xl" : "rounded-xl"
            }`}
            style={{ left: pos.left, top: pos.top, width: pos.width, maxHeight: 260 }}
            onMouseDown={(e) => e.stopPropagation()}
          >
            {options.map((o) => {
              const active = o.value === value;
              return (
                <button
                  key={String(o.value)}
                  type="button"
                  onClick={() => {
                    onChange(o.value);
                    setOpen(false);
                  }}
                  className={`w-full h-8 px-2.5 rounded-lg flex items-center text-[12.5px] text-left transition-colors truncate ${
                    active
                      ? "text-[var(--accent-strong)] font-semibold bg-[var(--accent-weak)]"
                      : "text-[var(--ink)] hover:bg-[var(--shade-strong)]"
                  }`}
                  title={o.label}
                >
                  <span className="truncate">{o.label}</span>
                </button>
              );
            })}
          </div>,
          document.body
        )}
    </>
  );
}
