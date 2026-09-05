import { AlertCircle, CheckCircle2, Info } from "lucide-react";
import { useStore } from "../store";

export default function ToastContainer() {
  const toasts = useStore((s) => s.toasts);
  const dismiss = useStore((s) => s.dismissToast);
  if (!toasts.length) return null;
  return (
    <div className="fixed right-5 bottom-[112px] z-[90] flex flex-col gap-2 items-end">
      {toasts.map((t) => (
        <div
          key={t.id}
          onClick={() => dismiss(t.id)}
          className="glass-strong rounded-xl px-4 py-2.5 flex items-center gap-2.5 text-[13px] shadow-xl anim-toast cursor-pointer max-w-[380px]"
        >
          {t.type === "error" ? (
            <AlertCircle size={15} className="text-rose-400 shrink-0" />
          ) : t.type === "success" ? (
            <CheckCircle2 size={15} className="text-emerald-400 shrink-0" />
          ) : (
            <Info size={15} className="text-[var(--accent)] shrink-0" />
          )}
          <span className="text-zinc-200 leading-snug">{t.msg}</span>
        </div>
      ))}
    </div>
  );
}
