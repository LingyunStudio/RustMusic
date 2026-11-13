import { LocateFixed } from "lucide-react";
import { useI18n } from "../i18n";
import { useStore } from "../store";

export default function LocateCurrentPill({
  show,
  onClick,
  className = "",
}: {
  show: boolean;
  onClick: () => void;
  className?: string;
}) {
  const { tr } = useI18n();
  const uiTheme = useStore((s) => s.uiTheme);
  const isLiquid = uiTheme === "liquid";

  if (!show) return null;
  return (
    <button
      className={`z-10 flex items-center gap-1.5 h-8 glass-strong text-[12px] anim-fade whitespace-nowrap ${
        isLiquid
          ? "px-4 rounded-full font-semibold text-[var(--accent-strong)] hover:text-[var(--accent)] hover:scale-105 active:scale-95 transition-all"
          : "px-3.5 rounded-full shadow-lg font-medium text-[var(--accent-strong)] hover:text-[var(--accent)]"
      } ${className}`}
      onClick={onClick}
    >
      <LocateFixed size={13} />
      {tr("定位到正在播放", "Locate Playing")}
    </button>
  );
}
