import { LocateFixed } from "lucide-react";

/**
 * 「回到当前歌曲」浮动药丸：当前播放行滚出列表可视区时出现，
 * 点击平滑滚回该行（自动定位的"可撤销"补丁——定位跳过去之后总有路回来）。
 * 需要挂在滚动容器的**父级**（absolute 锚定，不随内容滚动），
 * className 负责在各自布局里的位置。
 */
export default function LocateCurrentPill({
  show,
  onClick,
  className = "absolute bottom-3 left-1/2 -translate-x-1/2",
}: {
  show: boolean;
  onClick: () => void;
  className?: string;
}) {
  if (!show) return null;
  return (
    <button
      className={`z-10 flex items-center gap-1.5 h-8 px-3.5 rounded-full glass-strong shadow-lg text-[12px] font-medium text-[var(--accent-strong)] hover:text-[var(--accent)] anim-fade whitespace-nowrap ${className}`}
      onClick={onClick}
    >
      <LocateFixed size={13} />
      回到当前歌曲
    </button>
  );
}
