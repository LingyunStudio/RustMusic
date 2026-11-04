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
      className="fixed inset-0 z-[80] flex items-center justify-center bg-black/60 anim-fade"
      onMouseDown={onClose}
    >
      <div
        className="glass-strong rounded-2xl anim-np overflow-hidden relative border border-[var(--sk-edge)]"
        style={{
          width,
          boxShadow: "var(--cover-shadow-lg)",
        }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {/* 拟物四角沉头螺丝 */}
        <div className="skeuo-screw absolute top-3 left-3" />
        <div className="skeuo-screw absolute top-3 right-12" />

        {title && (
          <div className="flex items-center justify-between pl-8 pr-4 pt-3.5 pb-3 border-b border-[var(--sk-divider)] shadow-[0_1px_0_var(--sk-divider-hi)]">
            <span className="font-bold text-[14.5px] text-[var(--ink)] tracking-wide">{title}</span>
            <button className="btn-ghost !w-7 !h-7" onClick={onClose}>
              <X size={14} />
            </button>
          </div>
        )}
        <div className="p-5">{children}</div>
      </div>
    </div>,
    document.body
  );
}
