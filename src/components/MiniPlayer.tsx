//! 迷你模式浮窗：放大版「播放条左侧信息舱」——封面 + 曲目信息 + 播控按钮。
//! 外观由各界面风格的 CSS 定义（.mini-player / .mini-play）：
//! 拟物 = 机架金属小舱（skeuo-screw 沉头螺丝仅拟物主题可见），
//! 标准 = 玻璃卡片，液态玻璃 = 浮岛卡片。
import { Pause, Play, SkipBack, SkipForward, X } from "lucide-react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";

export default function MiniPlayer() {
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  const togglePlay = useStore((s) => s.togglePlay);
  const next = useStore((s) => s.next);
  const prev = useStore((s) => s.prev);
  const setMiniPlayer = useStore((s) => s.setMiniPlayer);

  return (
    <div
      className="mini-player anim-rise fixed bottom-2 left-2 z-[62] flex items-center gap-3 rounded-2xl p-2 pr-2.5"
      data-mini
    >
      {/* 拟物机舱沉头螺丝（其余界面风格下无样式，不可见） */}
      <div className="skeuo-screw absolute top-2 left-2 z-10" />
      <div className="skeuo-screw absolute top-2 right-2 z-10" />

      <CoverImg
        src={current?.cover}
        seed={current?.title ?? ""}
        small
        className="mini-cover h-[52px] w-[52px] shrink-0 rounded-lg"
        iconSize={20}
      />

      <div className="min-w-0 flex-1">
        <div className="truncate text-[12.5px] font-semibold text-[var(--ink)]">
          {current?.title ?? "未在播放"}
        </div>
        <div className="mt-0.5 flex items-center gap-1.5">
          <span className="truncate text-[11px] text-[var(--ink-2)]">
            {current?.artist ?? ""}
          </span>
          {current?.quality && (
            <span className="mini-quality shrink-0 rounded px-1.5 py-px text-[9px] font-bold tracking-wider text-[var(--accent-strong)]">
              {current.quality}
            </span>
          )}
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-0.5">
        <button className="btn-ghost h-8 w-8" onClick={() => prev()} title="上一首">
          <SkipBack size={15} className="fill-current" />
        </button>
        <button
          className="mini-play flex h-10 w-10 items-center justify-center rounded-full"
          onClick={togglePlay}
          title="播放 / 暂停（空格）"
        >
          {playing ? (
            <Pause size={16} className="fill-current" />
          ) : (
            <Play size={16} className="fill-current ml-0.5" />
          )}
        </button>
        <button className="btn-ghost h-8 w-8" onClick={() => next()} title="下一首">
          <SkipForward size={15} className="fill-current" />
        </button>
        <button
          className="btn-ghost h-8 w-8"
          onClick={() => setMiniPlayer(false)}
          title="退出迷你模式"
        >
          <X size={15} />
        </button>
      </div>
    </div>
  );
}
