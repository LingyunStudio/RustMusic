import { ListMusic, Pause, Play, Trash2, X } from "lucide-react";
import { useStore } from "../store";
import { fmtTime } from "../utils";
import type { QueueItem } from "../types";

function queueLabel(item: QueueItem): { title: string; artist: string; cover?: string } {
  const s = useStore.getState();
  if (item.kind === "track") {
    const t = s.tracks.find((x) => x.id === item.id);
    if (t)
      return {
        title: t.title,
        artist: t.artist || "未知艺术家",
        cover: t.cover,
      };
    return { title: `曲目 #${item.id}`, artist: "" };
  }
  if (item.kind === "netease") {
    const t = s.neteaseCache[item.id];
    if (t)
      return {
        title: t.name,
        artist: t.ar.map((a) => a.name).join(" / ") || "网易云",
        cover: t.al?.picUrl ?? undefined,
      };
    return { title: `网易云 #${item.id}`, artist: "在线曲库" };
  }
  if (item.kind === "qq") {
    const t = s.qqCache[item.id];
    if (t)
      return {
        title: t.name,
        artist: t.singer || "QQ音乐",
        cover: t.albumMid
          ? `https://y.gtimg.cn/music/photo_new/T002R300x300M000${t.albumMid}.jpg`
          : undefined,
      };
    return { title: `QQ音乐 #${item.id}`, artist: "在线曲库" };
  }
  const src = s.sources.find((x) => x.id === item.id);
  return {
    title: src?.title || src?.url.split("/").pop() || "在线音源",
    artist: "在线音源",
  };
}

export default function QueuePanel() {
  const queue = useStore((s) => s.queue);
  const qIndex = useStore((s) => s.qIndex);
  const playing = useStore((s) => s.playing);
  const pos = useStore((s) => s.pos);
  const dur = useStore((s) => s.dur);
  const jumpTo = useStore((s) => s.jumpTo);
  const removeQueueItem = useStore((s) => s.removeQueueItem);
  const clearQueue = useStore((s) => s.clearQueue);
  const setQueueOpen = useStore((s) => s.setQueueOpen);

  const upcoming = queue.slice(qIndex + 1);

  return (
    <aside className="w-[300px] shrink-0 flex flex-col border-l border-[var(--line)] bg-black/25 relative z-20 anim-fade">
      <div className="h-14 px-5 flex items-center justify-between shrink-0">
        <span className="text-[13.5px] font-semibold flex items-center gap-2.5 text-[var(--ink)]">
          <ListMusic size={15} className="text-[var(--accent)]" />
          播放队列
          <span className="text-[11.5px] text-[var(--ink-3)] font-normal">
            {queue.length} 首
          </span>
        </span>
        <div className="flex items-center gap-1">
          <button className="btn-ghost w-8 h-8" onClick={clearQueue} title="清空队列">
            <Trash2 size={14} />
          </button>
          <button className="btn-ghost w-8 h-8" onClick={() => setQueueOpen(false)}>
            <X size={15} />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-2.5 pb-3">
        {queue.length === 0 && (
          <div className="text-[12.5px] text-[var(--ink-3)] text-center pt-10">队列是空的</div>
        )}
        {queue.map((item, i) => {
          const { title, artist, cover } = queueLabel(item);
          const active = i === qIndex;
          return (
            <div
              key={`${item.kind}-${item.id}-${i}`}
              className={`group flex items-center gap-3 h-12 px-2.5 rounded-xl cursor-pointer transition-colors ${
                active ? "bg-[rgba(240,162,74,0.1)]" : "hover:bg-white/[0.05]"
              }`}
              onClick={() => jumpTo(i)}
            >
              <div className="w-9 h-9 rounded-lg bg-white/[0.06] flex items-center justify-center overflow-hidden shrink-0">
                {active && playing ? (
                  <div className="eq-bars">
                    <i />
                    <i />
                    <i />
                  </div>
                ) : (
                  <Play
                    size={11}
                    className={`text-[var(--ink-3)] group-hover:text-[var(--ink)] ${
                      active ? "" : "opacity-0 group-hover:opacity-100"
                    }`}
                  />
                )}
              </div>
              <div className="min-w-0 flex-1">
                <div
                  className={`text-[12.5px] truncate ${
                    active ? "text-[var(--accent-strong)]" : "text-[var(--ink)]"
                  }`}
                >
                  {title}
                </div>
                <div className="text-[11px] text-[var(--ink-3)] truncate mt-0.5">{artist}</div>
              </div>
              <button
                className="btn-ghost w-7 h-7 opacity-0 group-hover:opacity-100 shrink-0"
                onClick={(e) => {
                  e.stopPropagation();
                  removeQueueItem(i);
                }}
              >
                <X size={13} />
              </button>
            </div>
          );
        })}
      </div>

      {queue.length > 0 && (
        <div className="px-5 py-3 border-t border-[var(--line)] text-[11px] text-[var(--ink-3)] flex items-center justify-between shrink-0 tabular-nums">
          <span>{upcoming.length > 0 ? `接下来 ${upcoming.length} 首` : "播放到列表末尾"}</span>
          <span>
            {fmtTime(pos)} / {fmtTime(dur)}
          </span>
        </div>
      )}
    </aside>
  );
}
