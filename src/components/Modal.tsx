import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: string;
  width?: number;
  children: ReactNode;
}

export default function Modal({ open, onClose, title, width = 400, children }: ModalProps) {
  if (!open) return null;
  return createPortal(
    <div
      className="fixed inset-0 z-[80] flex items-center justify-center bg-black/50 backdrop-blur-md anim-fade"
      onMouseDown={onClose}
    >
      <div
        className="liquid-card glass-strong rounded-2xl shadow-2xl anim-np overflow-hidden relative border border-[var(--sk-panel-edge)]"
        style={{ width, boxShadow: "var(--cover-shadow-lg)" }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* 拟物四角沉头螺丝（仅拟物风格有样式，其余风格不可见） */}
        <div className="skeuo-screw absolute top-3 left-3" />
        <div className="skeuo-screw absolute top-3 right-12" />

        {title && (
          <div className="flex items-center justify-between px-6 pt-5 pb-3 border-b border-[var(--line)]">
            <span className="font-bold text-[15px] text-[var(--ink)] tracking-wide">{title}</span>
            <button className="btn-ghost w-8 h-8 rounded-full" onClick={onClose}>
              <X size={15} />
            </button>
          </div>
        )}
        <div className="px-6 py-5">{children}</div>
      </div>
    </div>,
    document.body
  );
}
