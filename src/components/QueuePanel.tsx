import { useEffect, useLayoutEffect, useRef } from "react";
import { ListMusic, Pause, Play, Trash2, X } from "lucide-react";
import { useStore } from "../store";
import { useLocatePill } from "../hooks/useLocatePill";
import LocateCurrentPill from "./LocateCurrentPill";
import { fmtTime } from "../utils";
import CoverImg from "./CoverImg";
import { useVirtualWindow } from "../hooks/useVirtualWindow";
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
  if (item.kind === "kugou") {
    const t = s.kugouCache[item.id];
    if (t)
      return {
        title: t.name,
        artist: t.singer || "酷狗",
        cover: t.cover || undefined,
      };
    return { title: `酷狗 #${item.id}`, artist: "在线曲库" };
  }
  if (item.kind === "bilibili") {
    const t = s.biliCache[item.id as string];
    if (t)
      return {
        title: t.title,
        artist: t.artist || "哔哩哔哩",
        cover: t.cover || undefined,
      };
    return { title: `B站 #${item.id}`, artist: "哔哩哔哩" };
  }
  const src = s.sources.find((x) => x.id === item.id);
  return {
    title: src?.title || src?.url.split("/").pop() || "在线音源",
    artist: "在线音源",
  };
}

export default function QueuePanel() {
  const queue = useStore((s) => s.queue);
  const unavailable = useStore((s) => s.unavailable);
  const qIndex = useStore((s) => s.qIndex);
  const playing = useStore((s) => s.playing);
  const pos = useStore((s) => s.pos);
  const dur = useStore((s) => s.dur);
  const jumpTo = useStore((s) => s.jumpTo);
  const removeQueueItem = useStore((s) => s.removeQueueItem);
  const clearQueue = useStore((s) => s.clearQueue);
  const setQueueOpen = useStore((s) => s.setQueueOpen);

  const upcoming = queue.slice(qIndex + 1);

  // 队列可达全库规模（“播放全部”）：窗口化渲染，只挂可视区行
  const ROW_H = 48;
  const win = useVirtualWindow(queue.length, ROW_H);

  // 面板是条件挂载（打开 = 挂载）：打开即定位到当前播放行
  useLayoutEffect(() => {
    followRef.current = true;
    if (qIndex >= 0 && queue.length > 0) win.scrollToIndex(qIndex);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 跟随播放：面板开着切歌时滚到新当前行；用户手动滚动/触摸即视为主动
  // 浏览、暂停跟随（点药丸回当前行会恢复），避免滚动条被抢
  const followRef = useRef(true);
  const prevQIdxRef = useRef(qIndex);
  useEffect(() => {
    if (qIndex !== prevQIdxRef.current) {
      prevQIdxRef.current = qIndex;
      if (followRef.current && qIndex >= 0) win.scrollToIndex(qIndex);
    }
  }, [qIndex]);

  // 当前播放行滚出可视区时浮出「回到当前歌曲」药丸
  const pill = useLocatePill({
    containerRef: win.containerRef,
    rowHeight: ROW_H,
    currentIndex: qIndex,
  });

  return (
    // 窗口级浮层（与音量弹出/右键菜单同语言：glass-strong + 圆角 + 浮起阴影）：
    // 悬挂在播放条上方右侧，不挤占内容区布局——侧栏挤位会把窄页面的
    // 标题/搜索框压到竖排换行；bottom 88px = 播放条占位 84px + 4px 空隙
    <aside className="fixed bottom-[88px] right-6 z-[61] w-[340px] max-h-[min(64vh,520px)] flex flex-col glass-strong rounded-2xl shadow-2xl anim-menu overflow-hidden">
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

      <div className="relative flex-1 min-h-0 flex flex-col">
      <div
        ref={win.containerRef}
        onScroll={win.onScroll}
        onWheel={() => (followRef.current = false)}
        onTouchStart={() => (followRef.current = false)}
        className="flex-1 overflow-y-auto px-2.5 py-1.5"
      >
        {queue.length === 0 && (
          <div className="text-[12.5px] text-[var(--ink-3)] text-center pt-10">队列是空的</div>
        )}
        <div style={{ height: win.start * ROW_H }} aria-hidden />
        {queue.slice(win.start, win.end).map((item, k) => {
          const i = win.start + k;
          const { title, artist, cover } = queueLabel(item);
          const active = i === qIndex;
          const dead = unavailable[`${item.kind}:${item.id}`] != null;
          return (
            <div
              key={`${item.kind}-${item.id}-${i}`}
              style={{ ["--row-idx" as string]: Math.min(i, 12) }}
              className={`${i < 24 ? "anim-row" : ""} group flex items-center gap-3 h-12 px-2.5 rounded-xl cursor-pointer transition-colors ${
                active ? "bg-[var(--accent-weak)]" : "hover:bg-[var(--shade-hover)]"
              } ${dead ? "opacity-45" : ""}`}
              title={dead ? `无法播放：${unavailable[`${item.kind}:${item.id}`]}` : undefined}
              onClick={() => jumpTo(i)}
            >
              <div className="relative w-9 h-9 rounded-lg overflow-hidden shrink-0">
                <CoverImg src={cover} seed={title} small className="w-9 h-9" iconSize={13} />
                {/* 播放意图反馈：hover 显示播放/暂停，当前行播放中叠均衡器动画 */}
                {active && playing ? (
                  <div className="absolute inset-0 flex items-center justify-center bg-black/35">
                    <div className="eq-bars">
                      <i />
                      <i />
                      <i />
                    </div>
                  </div>
                ) : (
                  <div className="absolute inset-0 flex items-center justify-center bg-black/35 opacity-0 group-hover:opacity-100 transition-opacity">
                    <Play size={11} className="text-white fill-current" />
                  </div>
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
        <div style={{ height: Math.max(0, queue.length - win.end) * ROW_H }} aria-hidden />
      </div>
      <LocateCurrentPill
        show={pill.show}
        onClick={() => {
          followRef.current = true;
          pill.locate();
        }}
      />
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
