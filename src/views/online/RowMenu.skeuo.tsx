//! 在线曲目右键菜单（Portal 到 body）。
import { createPortal } from "react-dom";
import { Heart, Play } from "lucide-react";
import { useStore } from "../../store";
import { clampMenuPos } from "../../utils";
import type { OnlineRow } from "./shared";

export function RowMenu({
  menu,
  onClose,
  menuAction,
  onAddToPlaylist,
}: {
  menu: { x: number; y: number; row: OnlineRow };
  onClose: () => void;
  menuAction: (row: OnlineRow, action: "play" | "next" | "queue") => void;
  onAddToPlaylist: (row: OnlineRow) => void;
}) {
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const downloadOnline = useStore((s) => s.downloadOnline);

  return (
    createPortal(
        <div
          className="fixed z-[75] w-[200px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
          style={(() => {
            const p = clampMenuPos(menu.x, menu.y, 200, 240);
            return { left: p.x, top: p.y };
          })()}
          onMouseDown={(e) => e.stopPropagation()}
          onMouseLeave={onClose}
        >
          {[
            { label: "播放", action: "play" as const },
            { label: "下一首播放", action: "next" as const },
            { label: "加入队列", action: "queue" as const },
          ].map((item) => (
            <button
              key={item.action}
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                menuAction(menu.row, item.action);
                onClose();
              }}
            >
              <Play size={13} /> {item.label}
            </button>
          ))}
          <div className="my-1 mx-2 border-t border-[var(--line)]" />
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              onAddToPlaylist(menu.row);
              onClose();
            }}
          >
            <Heart size={13} /> 添加到播放列表…
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              toggleLikeOnline({
                kind: menu.row.kind,
                id: menu.row.id,
                name: menu.row.name,
                artist: menu.row.artist,
                album: menu.row.album,
                cover: menu.row.cover,
                durationMs: menu.row.durationMs,
                mediaMid: menu.row.mediaMid,
                vip: menu.row.vip,
              });
              onClose();
            }}
          >
            <Heart size={13} /> 收藏到“我喜欢”
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              downloadOnline({
                kind: menu.row.kind,
                id: menu.row.id,
                name: menu.row.name,
                artist: menu.row.artist,
                album: menu.row.album,
                cover: menu.row.cover,
                durationMs: menu.row.durationMs,
                mediaMid: menu.row.mediaMid,
              });
              onClose();
            }}
          >
            <Play size={13} /> 下载到本地
          </button>
        </div>,
        document.body
      )
    );
}
