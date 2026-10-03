//! B 站结果卡片（16:9 封面网格）：播放浮层、多选勾选、收藏标记、移除按钮。
import { Check, Heart, Play, X } from "lucide-react";
import CoverImg from "../../components/CoverImg";
import { fmtDate, fmtTime } from "../../utils";
import type { BiliSpaceItem } from "../../types";

function fmtPlay(n: number): string {
  if (n >= 10000_0000) return `${(n / 10000_0000).toFixed(1)}亿`;
  if (n >= 10000) return `${(n / 10000).toFixed(1)}万`;
  return String(n);
}

export function BiliSpaceCard({
  row: r,
  active,
  checked,
  selMode,
  liked,
  onPlay,
  onToggle,
  onRemove,
  onContextMenu,
  onCardClick,
}: {
  row: BiliSpaceItem;
  active: boolean;
  checked: boolean;
  selMode: boolean;
  liked: boolean;
  onPlay: (e: React.MouseEvent<HTMLButtonElement>) => void;
  onToggle: () => void;
  onRemove: () => void;
  onContextMenu: (e: React.MouseEvent<HTMLDivElement>) => void;
  onCardClick: () => void;
}) {
  return (
      <div
        key={r.rid}
        data-now-playing={active ? "1" : undefined}
        className={`group cursor-pointer rounded-xl p-1.5 transition-colors ${
          checked ? "bg-[var(--accent-weak)]" : "hover:bg-[var(--shade)]"
        }`}
        onContextMenu={onContextMenu}
        onClick={onCardClick}
      >
        {/* 16:9 封面 */}
        <div className="relative aspect-video rounded-lg overflow-hidden bg-[var(--shade)]">
          <CoverImg
            src={r.cover}
            seed={r.title}
            className="absolute inset-0 w-full h-full"
            iconSize={26}
          />
          {/* 时长角标 */}
          {r.durationMs > 0 && (
            <span className="absolute bottom-1 right-1 px-1.5 py-0.5 rounded bg-black/70 text-white text-[10.5px] tabular-nums">
              {fmtTime(r.durationMs)}
            </span>
          )}
          {/* 播放按钮浮层 */}
          {!selMode && (
            <button
              className="absolute inset-0 m-auto w-10 h-10 rounded-full bg-black/55 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
              title="播放（整个列表连播）"
              onClick={onPlay}
            >
              <Play size={16} className="fill-current text-white ml-0.5" />
            </button>
          )}
          {/* 多选勾选框 */}
          {selMode && (
            <span
              className={`absolute top-1.5 left-1.5 w-[18px] h-[18px] rounded-[5px] border flex items-center justify-center transition-colors ${
                checked
                  ? "bg-[var(--accent)] border-[var(--accent)] text-[var(--accent-on)]"
                  : "border-white/70 bg-black/30"
              }`}
            >
              {checked && <Check size={13} strokeWidth={3} />}
            </span>
          )}
          {/* 已收藏标记 */}
          {liked && !selMode && (
            <span className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full bg-black/45 flex items-center justify-center">
              <Heart size={10} className="fill-[#e0533f] text-[#e0533f]" />
            </span>
          )}
          {/* 移除 */}
          {!selMode && (
            <button
              className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full bg-black/45 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity hover:bg-black/70"
              title="从结果中移除"
              onClick={(e) => {
                e.stopPropagation();
                onRemove();
              }}
            >
              <X size={11} className="text-white" />
            </button>
          )}
          {/* 正在播放标记 */}
          {active && (
            <span className="absolute left-1.5 bottom-1 px-1.5 py-0.5 rounded bg-[#fb7299] text-white text-[10px] font-semibold">
              播放中
            </span>
          )}
        </div>
        {/* 标题 + 元信息 */}
        <div
          className={`text-[12.5px] leading-snug mt-1.5 ${
            active ? "text-[#fb7299] font-semibold" : "text-[var(--ink)]"
          }`}
          style={{
            display: "-webkit-box",
            WebkitLineClamp: 2,
            WebkitBoxOrient: "vertical",
            overflow: "hidden",
          }}
        >
          {r.title || "B站视频"}
        </div>
        <div className="text-[11px] text-[var(--ink-3)] truncate mt-0.5 flex items-center gap-1.5">
          <span className="truncate">{r.artist || "哔哩哔哩"}</span>
          {r.play > 0 && (
            <>
              <span>·</span>
              <span className="shrink-0">{fmtPlay(r.play)}</span>
            </>
          )}
          {r.created > 0 && (
            <>
              <span>·</span>
              <span className="shrink-0">{fmtDate(r.created)}</span>
            </>
          )}
        </div>
      </div>
  );
}
