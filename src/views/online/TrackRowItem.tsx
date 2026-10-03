//! 在线曲目行（虚拟窗口渲染的单行）：当前行高亮、多选勾选、行内操作。
//! 组件自身从 store 读 current/playing/savedOnline/openDetailPage/toggleLikeOnline，
//! 父组件只传行数据与回调，保证列表逻辑集中在上层。
import { Heart, MoreHorizontal, Play, Check } from "lucide-react";
import { useStore } from "../../store";
import { fmtTime } from "../../utils";
import type { OnlineRow, Source } from "./shared";

export function TrackRowItem({
  row: t,
  index: i,
  batchMode,
  checked,
  onToggleSel,
  onPlay,
  onNext,
  onLike,
  onMore,
  onContextMenu,
  onOpenDetail,
}: {
  row: OnlineRow;
  index: number;
  batchMode: boolean;
  checked: boolean;
  onToggleSel: () => void;
  onPlay: () => void;
  onNext: () => void;
  onLike: () => void;
  onMore: (e: React.MouseEvent<HTMLButtonElement>) => void;
  onContextMenu: (e: React.MouseEvent<HTMLDivElement>) => void;
  onOpenDetail: (kind: "artist" | "album", name: string) => void;
}) {
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  const savedOnline = useStore((s) => s.savedOnline);
  const active =
    current?.kind === t.kind &&
    (t.kind === "qq"
      ? current.qid === t.id
      : t.kind === "kugou"
        ? current.kgid === t.id
        : t.kind === "netease"
          ? current.nid === t.id
          : false);
  const saved = !!savedOnline[`${t.kind}-${t.id}`];

  return (
                <div
                  key={`${t.kind}-${t.id}`}
                  style={{ ["--row-idx" as string]: Math.min(i, 12) }}
                  className={`${i < 24 ? "anim-row" : ""} group grid grid-cols-[56px_minmax(200px,460px)_minmax(180px,300px)_92px_136px] items-center gap-4 h-[60px] px-4 rounded-[13px] transition-colors cursor-default ${
                    batchMode
                      ? checked
                        ? "bg-[var(--accent-weak)]"
                        : "hover:bg-[var(--shade-hover)]"
                      : active
                        ? "bg-[var(--accent-weak)]"
                        : "hover:bg-[var(--shade-hover)]"
                  }`}
                  onClick={batchMode ? (ev) => { ev.stopPropagation(); onToggleSel(); } : undefined}
                  onDoubleClick={() => !batchMode && onPlay()}
                  onContextMenu={onContextMenu}
                >
                  <div className="relative h-11 flex items-center justify-center">
                    {batchMode ? (
                      <span
                        className={`w-[18px] h-[18px] rounded-[5px] border flex items-center justify-center transition-colors ${
                          checked
                            ? "bg-[var(--accent)] border-[var(--accent)] text-[var(--accent-on)]"
                            : "border-[var(--line)] group-hover:border-[var(--accent)]"
                        }`}
                      >
                        {checked && <Check size={13} strokeWidth={3} />}
                      </span>
                    ) : (
                      <>
                        <span
                          className={`text-[12.5px] tabular-nums transition-opacity ${
                            active
                              ? "text-[var(--accent)] font-bold opacity-100 group-hover:opacity-0"
                              : "text-[var(--ink-3)] group-hover:opacity-0"
                          }`}
                        >
                          {String(i + 1).padStart(2, "0")}
                        </span>
                        <button
                          className={`absolute inset-0 m-auto w-9 h-9 rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all hover:scale-105 ${
                            active
                              ? "text-[var(--accent)]"
                              : "bg-[var(--accent)] text-[var(--accent-on)]"
                          }`}
                          onClick={onPlay}
                          title="播放"
                        >
                          <Play size={14} className="fill-current ml-px" />
                        </button>
                      </>
                    )}
                  </div>

                  <div className="flex items-center gap-4 min-w-0">
                    {t.cover ? (
                      <img
                        src={t.cover}
                        alt=""
                        className="hover-lift w-11 h-11 rounded-xl object-cover shadow-[var(--cover-shadow-sm)] shrink-0"
                        draggable={false}
                      />
                    ) : (
                      <div
                        className="w-11 h-11 rounded-xl shrink-0"
                        style={{ background: "rgba(243,233,216,0.07)" }}
                      />
                    )}
                    <div className="min-w-0">
                      <div
                        className={`text-[13.5px] truncate flex items-center gap-2 ${
                          active
                            ? "text-[var(--accent-strong)] font-semibold"
                            : "text-[var(--ink)]"
                        }`}
                      >
                        <span className="truncate">{t.name}</span>
                        {t.vip && (
                          <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-[var(--accent-weak)] text-[var(--accent-strong)] font-bold shrink-0">
                            VIP
                          </span>
                        )}
                        {active && (
                          <span className="shrink-0 inline-flex">
                            <span className={`eq-bars ${playing ? "" : "paused"}`}>
                              <i />
                              <i />
                              <i />
                            </span>
                          </span>
                        )}
                      </div>
                      <button
                        className={`block text-left text-[12px] truncate mt-1 max-w-full hover:text-[var(--accent-strong)] transition-colors ${
                          active ? "text-[var(--accent-strong)]" : "text-[var(--ink-3)]"
                        }`}
                        onClick={(ev) => { ev.stopPropagation(); onOpenDetail("artist", t.artist); }}
                        title={`查看歌手：${t.artist || "未知艺术家"}`}
                      >
                        {t.artist || "未知艺术家"}
                      </button>
                    </div>
                  </div>

                  <button
                    className="block text-left text-[12.5px] text-[var(--ink-3)] truncate max-w-full hover:text-[var(--accent-strong)] transition-colors"
                    onClick={(ev) => { ev.stopPropagation(); onOpenDetail("album", t.album); }}
                    title={`查看专辑：${t.album || "未知专辑"}`}
                  >
                    {t.album || "未知专辑"}
                  </button>

                  <div className="text-right text-[12.5px] text-[var(--ink-2)] tabular-nums">
                    {fmtTime(t.durationMs)}
                  </div>

                  <div className="flex items-center justify-end gap-1 pr-1">
                    <button
                      className="btn-ghost w-8 h-8"
                      onClick={(e) => { e.stopPropagation(); onLike(); }}
                      title={saved ? "取消喜欢" : "收藏到“我喜欢”"}
                    >
                      <Heart
                        size={15}
                        className={
                          saved
                            ? "fill-[#e0533f] text-[#e0533f]"
                            : "opacity-0 group-hover:opacity-100"
                        }
                      />
                    </button>
                    <button
                      className="btn-ghost w-8 h-8"
                      onClick={(e) => { e.stopPropagation(); onNext(); }}
                      title="下一首播放"
                    >
                      <Play
                        size={14}
                        className="opacity-0 group-hover:opacity-100"
                      />
                    </button>
                    <button
                      className="btn-ghost w-8 h-8"
                      onClick={onMore}
                    >
                      <MoreHorizontal
                        size={16}
                        className="opacity-0 group-hover:opacity-100"
                      />
                    </button>
                  </div>
                </div>
  );
}
