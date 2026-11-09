import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { useStore } from "../store";

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  width?: number;
  children: ReactNode;
}

export default function Modal({ open, onClose, title, width = 400, children }: ModalProps) {
  const uiTheme = useStore((s) => s.uiTheme);
  const isLiquid = uiTheme === "liquid";
  const isSkeuo = uiTheme === "skeuo";

  if (!open) return null;

  const backdropClass = isLiquid
    ? "fixed inset-0 z-[80] flex items-center justify-center bg-black/50 backdrop-blur-md anim-fade"
    : isSkeuo
      ? "fixed inset-0 z-[80] flex items-center justify-center bg-black/60 anim-fade"
      : "fixed inset-0 z-[80] flex items-center justify-center bg-black/45 anim-fade";

  const cardClass = isLiquid
    ? "liquid-card shadow-2xl anim-np overflow-hidden"
    : isSkeuo
      ? "glass-strong rounded-2xl anim-np overflow-hidden relative border border-[var(--sk-edge)]"
      : "glass-strong rounded-2xl shadow-2xl anim-np overflow-hidden";

  const cardStyle = isSkeuo
    ? { width, boxShadow: "var(--cover-shadow-lg)" }
    : { width };

  const headerClass = isLiquid
    ? "flex items-center justify-between px-6 pt-5 pb-3 border-b border-[var(--line)]"
    : isSkeuo
      ? "flex items-center justify-between pl-8 pr-4 pt-3.5 pb-3 border-b border-[var(--sk-divider)] shadow-[0_1px_0_var(--sk-divider-hi)]"
      : "flex items-center justify-between px-5 pt-4 pb-3";

  const titleClass = isLiquid
    ? "font-bold text-[15px] text-[var(--ink)] tracking-wide"
    : isSkeuo
      ? "font-bold text-[14.5px] text-[var(--ink)] tracking-wide"
      : "font-semibold text-[15px]";

  const closeBtnClass = isLiquid
    ? "btn-ghost w-8 h-8 rounded-full"
    : isSkeuo
      ? "btn-ghost !w-7 !h-7"
      : "btn-ghost w-7 h-7";

  const contentClass = isLiquid
    ? "px-6 py-5"
    : isSkeuo
      ? "p-5"
      : "px-5 pb-5";

  return createPortal(
    <div className={backdropClass} onMouseDown={onClose}>
      <div
        className={cardClass}
        style={cardStyle}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {isSkeuo && (
          <>
            <div className="skeuo-screw absolute top-3 left-3" />
            <div className="skeuo-screw absolute top-3 right-12" />
          </>
        )}
        {title && (
          <div className={headerClass}>
            <span className={titleClass}>{title}</span>
            <button className={closeBtnClass} onClick={onClose}>
              <X size={isSkeuo ? 14 : 15} />
            </button>
          </div>
        )}
        <div className={contentClass}>{children}</div>
      </div>
    </div>,
    document.body
  );
}
