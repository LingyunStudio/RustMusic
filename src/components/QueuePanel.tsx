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
  const jumpTo = useStore((s) => s.jumpTo);
  const removeQueueItem = useStore((s) => s.removeQueueItem);
  const clearQueue = useStore((s) => s.clearQueue);
  const setQueueOpen = useStore((s) => s.setQueueOpen);

  const upcoming = queue.slice(qIndex + 1);

  return (
    <aside className="w-[300px] shrink-0 flex flex-col border-l border-white/[0.07] bg-black/20 relative z-20 anim-fade">
      <div className="h-12 px-4 flex items-center justify-between shrink-0">
        <span className="text-[13px] font-semibold flex items-center gap-2">
          <ListMusic size={15} className="text-[var(--dyn)]" />
          播放队列
          <span className="text-[11px] text-zinc-500 font-normal">{queue.length} 首</span>
        </span>
        <div className="flex items-center gap-0.5">
          <button className="btn-ghost w-7 h-7" onClick={clearQueue} title="清空队列">
            <Trash2 size={14} />
          </button>
          <button className="btn-ghost w-7 h-7" onClick={() => setQueueOpen(false)}>
            <X size={14} />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto px-2 pb-3">
        {queue.length === 0 && (
          <div className="text-[12.5px] text-zinc-600 text-center pt-10">队列是空的</div>
        )}
        {queue.map((item, i) => {
          const { title, artist, cover } = queueLabel(item);
          const active = i === qIndex;
          return (
            <div
              key={`${item.kind}-${item.id}-${i}`}
              className={`group flex items-center gap-2.5 h-11 px-2 rounded-lg cursor-pointer transition-colors ${
                active ? "bg-white/[0.08]" : "hover:bg-white/[0.05]"
              }`}
              onClick={() => jumpTo(i)}
            >
              <div className="w-8 h-8 rounded-md bg-white/[0.06] flex items-center justify-center overflow-hidden shrink-0">
                {active && playing ? (
                  <div className="eq-bars">
                    <i />
                    <i />
                    <i />
                  </div>
                ) : (
                  <Play
                    size={11}
                    className={`text-zinc-500 group-hover:text-zinc-200 ${
                      active ? "" : "opacity-0 group-hover:opacity-100"
                    }`}
                  />
                )}
              </div>
              <div className="min-w-0 flex-1">
                <div
                  className={`text-[12.5px] truncate ${active ? "text-[var(--dyn)]" : "text-zinc-200"}`}
                >
                  {title}
                </div>
                <div className="text-[11px] text-zinc-500 truncate">{artist}</div>
              </div>
              <button
                className="btn-ghost w-6 h-6 opacity-0 group-hover:opacity-100 shrink-0"
                onClick={(e) => {
                  e.stopPropagation();
                  removeQueueItem(i);
                }}
              >
                <X size={12} />
              </button>
            </div>
          );
        })}
      </div>

      {queue.length > 0 && (
        <div className="px-4 py-2.5 border-t border-white/[0.06] text-[11px] text-zinc-600 flex items-center justify-between shrink-0">
          <span>
            {upcoming.length > 0 ? `接下来 ${upcoming.length} 首` : "播放到列表末尾"}
          </span>
          <span className="tabular-nums">
            {fmtTime(useStore.getState().pos)} / {fmtTime(useStore.getState().dur)}
          </span>
        </div>
      )}
    </aside>
  );
}
