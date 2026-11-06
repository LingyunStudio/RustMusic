import { useEffect, useState, type ReactNode } from "react";
import Modal from "./Modal";

interface InputModalProps {
  open: boolean;
  title: string;
  placeholder?: string;
  confirmText?: string;
  /** 打开时的预填值（重命名用）；不传为空 */
  initialValue?: string;
  onClose: () => void;
  onConfirm: (value: string) => void;
}

export function InputModal({
  open,
  title,
  placeholder,
  confirmText = "确定",
  initialValue,
  onClose,
  onConfirm,
}: InputModalProps) {
  const [value, setValue] = useState("");
  useEffect(() => {
    if (open) setValue(initialValue ?? "");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const submit = () => {
    const v = value.trim();
    if (!v) return;
    onConfirm(v);
    onClose();
  };

  return (
    <Modal open={open} onClose={onClose} title={title} width={380}>
      <input
        type="text"
        autoFocus
        value={value}
        placeholder={placeholder}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && submit()}
        className="w-full h-10 rounded-lg bg-[var(--shade)] border border-[var(--line)] px-3.5 text-[13px] focus:border-[var(--line)] outline-none"
      />
      <div className="flex justify-end gap-2 mt-4">
        <button className="btn-secondary" onClick={onClose}>
          取消
        </button>
        <button className="btn-primary" disabled={!value.trim()} onClick={submit} style={{ opacity: value.trim() ? 1 : 0.5 }}>
          {confirmText}
        </button>
      </div>
    </Modal>
  );
}

interface ConfirmModalProps {
  open: boolean;
  title: string;
  children?: ReactNode;
  confirmText?: string;
  danger?: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

export function ConfirmModal({
  open,
  title,
  children,
  confirmText = "确定",
  danger,
  onClose,
  onConfirm,
}: ConfirmModalProps) {
  return (
    <Modal open={open} onClose={onClose} title={title} width={380}>
      <div className="text-[13px] text-[var(--ink-2)] leading-relaxed">{children}</div>
      <div className="flex justify-end gap-2 mt-5">
        <button className="btn-secondary" onClick={onClose}>
          取消
        </button>
        <button
          className="btn-primary"
          style={
            danger
              ? {
                  background: "linear-gradient(135deg,#e11d48,#be123c)",
                  color: "#fff",
                }
              : undefined
          }
          onClick={() => {
            onConfirm();
            onClose();
          }}
        >
          {confirmText}
        </button>
      </div>
    </Modal>
  );
}
