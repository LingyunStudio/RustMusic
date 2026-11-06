//! 迷你模式：占底部播放条原位的紧凑条 —— 播放条「左侧信息舱」的放大版
//! （大封面 + 曲目信息 + 音质徽标）加播控按钮（上一首 / 播放 / 下一首 / 退出）。
//! 外观由各界面风格的 CSS 定义（.mini-player / .mini-play / .mini-cover）：
//! 拟物 = 机架金属小舱（skeuo-screw 沉头螺丝仅拟物主题可见），
//! 标准 = 玻璃条，液态玻璃 = 浮岛条。
import { ChevronUp, Pause, Play, SkipBack, SkipForward, X } from "lucide-react";
import { useStore } from "../store";
import CoverImg from "./CoverImg";

export default function MiniPlayer({ centered = false }: { centered?: boolean }) {
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  const togglePlay = useStore((s) => s.togglePlay);
  const next = useStore((s) => s.next);
  const prev = useStore((s) => s.prev);
  const nowPlayingOpen = useStore((s) => s.nowPlayingOpen);
  const setNowPlayingOpen = useStore((s) => s.setNowPlayingOpen);
  const setMiniPlayer = useStore((s) => s.setMiniPlayer);

  return (
    <div
      className={`anim-bar absolute bottom-0 right-0 z-50 pl-3 pr-2 pb-2 pt-1 ${
        centered ? "left-0" : "left-[250px]"
      }`}
    >
      <div
        className="mini-player relative flex h-[76px] items-center gap-4 overflow-hidden rounded-2xl py-2 pl-3 pr-4"
        data-mini
      >
        {/* 拟物机舱沉头螺丝（其余界面风格下无样式，不可见） */}
        <div className="skeuo-screw absolute top-2 left-2 z-10" />
        <div className="skeuo-screw absolute top-2 right-2 z-10" />

        {/* 放大版封面：点击展开/收起播放页（与播放条左舱一致） */}
        <button
          className="group relative shrink-0 overflow-hidden rounded-[9px]"
          onClick={() => setNowPlayingOpen(!nowPlayingOpen)}
          title={nowPlayingOpen ? "收起播放页" : "展开播放页"}
        >
          <CoverImg
            src={current?.cover}
            seed={current?.title ?? ""}
            small
            className="mini-cover h-[56px] w-[56px] rounded-[9px]"
            iconSize={22}
          />
          <div className="absolute inset-0 flex items-center justify-center bg-black/45 opacity-0 transition-opacity group-hover:opacity-100">
            <ChevronUp
              size={17}
              className={`text-white transition-transform ${nowPlayingOpen ? "rotate-180" : ""}`}
            />
          </div>
        </button>

        {/* 曲目信息 */}
        <div className="min-w-0 flex-1">
          <div className="truncate text-[14px] font-semibold text-[var(--ink)]">
            {current?.title ?? "未在播放"}
          </div>
          <div className="mt-1 flex items-center gap-2">
            <span className="truncate text-[11.5px] text-[var(--ink-2)]">
              {current?.artist ?? ""}
            </span>
            {current?.quality && (
              <span className="mini-quality shrink-0 rounded px-1.5 py-px text-[9px] font-bold tracking-wider text-[var(--accent-strong)]">
                {current.quality}
              </span>
            )}
          </div>
        </div>

        {/* 播控按钮 */}
        <div className="flex shrink-0 items-center gap-1">
          <button className="btn-ghost h-9 w-9" onClick={() => prev()} title="上一首">
            <SkipBack size={16} className="fill-current" />
          </button>
          <button
            className="mini-play flex h-[52px] w-[52px] items-center justify-center rounded-full"
            onClick={togglePlay}
            title="播放 / 暂停（空格）"
          >
            {playing ? (
              <Pause size={19} className="fill-current" />
            ) : (
              <Play size={19} className="fill-current ml-0.5" />
            )}
          </button>
          <button className="btn-ghost h-9 w-9" onClick={() => next()} title="下一首">
            <SkipForward size={16} className="fill-current" />
          </button>
          <div className="mx-1.5 h-8 w-px bg-[var(--line)]" />
          <button
            className="btn-ghost h-9 w-9"
            onClick={() => setMiniPlayer(false)}
            title="退出迷你模式"
          >
            <X size={16} />
          </button>
        </div>
      </div>
    </div>
  );
}
