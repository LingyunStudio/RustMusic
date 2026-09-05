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
      className="fixed inset-0 z-[80] flex items-center justify-center bg-black/55 anim-fade"
      onMouseDown={onClose}
    >
      <div
        className="glass-strong rounded-2xl shadow-2xl anim-np overflow-hidden"
        style={{ width }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {title && (
          <div className="flex items-center justify-between px-5 pt-4 pb-3">
            <span className="font-semibold text-[15px]">{title}</span>
            <button className="btn-ghost w-7 h-7" onClick={onClose}>
              <X size={15} />
            </button>
          </div>
        )}
        <div className="px-5 pb-5">{children}</div>
      </div>
    </div>,
    document.body
  );
}
