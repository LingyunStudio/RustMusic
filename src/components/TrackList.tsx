import { Ban, Heart, ListMusic, MoreHorizontal, Play } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useStore } from "../store";
import { useDragList } from "../hooks/useDragList";
import { useVirtualWindow } from "../hooks/useVirtualWindow";
import type { PlaylistEntryMeta, TrackMeta } from "../types";
import { clampMenuPos, fmtTime, refineMenuPos, trackArtist, trackTitle } from "../utils";
import CoverImg from "./CoverImg";
import Modal from "./Modal";

interface MenuState {
  x: number;
  y: number;
  track: TrackMeta;
}

interface TrackListProps {
  tracks: TrackMeta[];
  /** 追加在本地曲目之后的在线条目（如“我喜欢”里的在线收藏） */
  onlineEntries?: PlaylistEntryMeta[];
  /** 本地/在线交叉的归并行序列（“最近播放/我喜欢”按时间合并排序时使用）；
   *  提供时优先于 tracks + onlineEntries 的拼接 */
  mergedRows?: (
    | { type: "local"; t: TrackMeta }
    | { type: "online"; e: PlaylistEntryMeta }
  )[];
  inCard?: boolean;
  emptyHint?: string;
  emptyAction?: { label: string; onClick: () => void };
  /** 是否允许长按拖拽调序（仅手动排序视图开启；提交顺序由调用方持久化） */
  dragSortable?: boolean;
  /** 拖拽提交（from → to 都是渲染序列里的下标） */
  onDragReorder?: (from: number, to: number) => void;
}

export type SortKey = "manual" | "title" | "artist" | "album" | "duration" | "added";

type MergedRow =
  | { type: "local"; t: TrackMeta }
  | { type: "online"; e: PlaylistEntryMeta };

/** 统一行描述：i = 播放下标（本地在 tracks / 在线在 onlineEntries 里），
 *  idxNum = 显示序号（归并模式下 = 归并序列位置） */
type RowEntry =
  | { type: "local"; t: TrackMeta; i: number; idxNum: number }
  | { type: "online"; e: PlaylistEntryMeta; i: number; idxNum: number };

const ROW_H = 64;
/** 入场交错动画只给首屏行：窗口化后滚动新挂载的行不再重播 rowIn */
const ANIM_ROWS = 24;

export default function TrackList({
  tracks,
  onlineEntries = [],
  mergedRows,
  inCard,
  emptyHint,
  emptyAction,
  dragSortable,
  onDragReorder,
}: TrackListProps) {
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [pickerFor, setPickerFor] = useState<TrackMeta | null>(null);
  const [onlineMenu, setOnlineMenu] = useState<{
    x: number;
    y: number;
    entry: PlaylistEntryMeta;
  } | null>(null);
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  const playTracks = useStore((s) => s.playTracks);
  const playEntries = useStore((s) => s.playEntries);
  const entryToQueueItem = useStore((s) => s.entryToQueueItem);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const downloadOnline = useStore((s) => s.downloadOnline);
  const toggleLike = useStore((s) => s.toggleLike);
  const addToQueue = useStore((s) => s.addToQueue);
  const playNext = useStore((s) => s.playNext);
  const playlists = useStore((s) => s.playlists);
  const unavailable = useStore((s) => s.unavailable);
  const createPlaylist = useStore((s) => s.createPlaylist);
  const [newPlName, setNewPlName] = useState("");

  useEffect(() => {
    if (!menu && !onlineMenu) return;
    const close = () => {
      setMenu(null);
      setOnlineMenu(null);
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("wheel", close);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("wheel", close);
    };
  }, [menu, onlineMenu]);

  // 菜单定位：右键/锚点位置先按估算尺寸夹取渲染，渲染后实测校正
  // （菜单项数量不定，估算高度会造成贴底菜单出屏）
  const rawX = menu?.x ?? onlineMenu?.x ?? 0;
  const rawY = menu?.y ?? onlineMenu?.y ?? 0;
  const menuRef = useRef<HTMLDivElement>(null);
  const [refined, setRefined] = useState<{ x: number; y: number } | null>(null);
  useEffect(() => {
    setRefined(null);
    if (!menu && !onlineMenu) return;
    let raf = requestAnimationFrame(() => {
      const el = menuRef.current;
      if (el) {
        const r = refineMenuPos(el, rawX, rawY);
        if (r) setRefined(r);
      }
    });
    return () => cancelAnimationFrame(raf);
  }, [menu, onlineMenu, rawX, rawY]);
  const menuPos = refined ?? clampMenuPos(rawX, rawY, 190, 200);
  const menuX = menuPos.x;
  const menuY = menuPos.y;

  const hasMerged = !!mergedRows && mergedRows.length > 0;
  // 拖拽调序只在启用且回调存在时生效（视觉反馈由 hook 直接写 DOM）
  const { rowProps, setEnabled } = useDragList((from, to) => {
    if (dragSortable && onDragReorder) onDragReorder(from, to);
  });
  setEnabled(!!dragSortable && !!onDragReorder);

  // 拖拽模式下退回全量渲染：useDragList 依赖“行 = 滚动容器全部直接子元素”
  // 直接读写 DOM，与窗口化互斥（手动排序是低频编辑态，临时全量可接受）
  const dragMode = !!dragSortable && !!onDragReorder;

  // 播放下标映射：归并序列回填原列表下标，消掉渲染期的 indexOf O(n²)
  const localIndexById = useMemo(() => {
    const m = new Map<number, number>();
    tracks.forEach((t, i) => {
      if (!m.has(t.id)) m.set(t.id, i);
    });
    return m;
  }, [tracks]);
  const onlineIndexByKey = useMemo(() => {
    const m = new Map<string, number>();
    onlineEntries.forEach((e, k) => {
      const key = `${e.kind}-${e.onlineId ?? ""}`;
      if (!m.has(key)) m.set(key, k);
    });
    return m;
  }, [onlineEntries]);

  // 统一行序列（归并模式用 mergedRows，否则 tracks + onlineEntries 拼接）
  const rows = useMemo<RowEntry[]>(() => {
    if (hasMerged) {
      return (mergedRows as MergedRow[]).map((row, p) =>
        row.type === "local"
          ? {
              type: "local" as const,
              t: row.t,
              i: localIndexById.get(row.t.id) ?? 0,
              idxNum: p,
            }
          : {
              type: "online" as const,
              e: row.e,
              i: onlineIndexByKey.get(`${row.e.kind}-${row.e.onlineId ?? ""}`) ?? 0,
              idxNum: p,
            }
      );
    }
    return [
      ...tracks.map((t, i) => ({ type: "local" as const, t, i, idxNum: i })),
      ...onlineEntries.map((e, k) => ({
        type: "online" as const,
        e,
        i: k,
        idxNum: tracks.length + k,
      })),
    ];
  }, [hasMerged, mergedRows, tracks, onlineEntries, localIndexById, onlineIndexByKey]);

  // 窗口化：DOM 只保留可视区 ± overscan 行（拖拽模式下计算了但不用）
  const win = useVirtualWindow(rows.length, ROW_H);
  const renderRow = (r: RowEntry) =>
    r.type === "local" ? renderLocal(r.t, r.i, r.idxNum) : renderOnline(r.e, r.i, r.idxNum);

  if (!tracks.length && !onlineEntries.length && !hasMerged) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-5 anim-fade">
        <div className="relative">
          <div
            className="absolute -inset-8 rounded-full"
            style={{ background: "radial-gradient(circle, var(--accent) 0%, transparent 65%)", opacity: 0.14 }}
          />
          <div
            className="relative w-[76px] h-[76px] rounded-3xl flex items-center justify-center"
            style={{
              background:
                "linear-gradient(135deg, rgba(243,233,216,0.1), rgba(243,233,216,0.03))",
              border: "1px solid rgba(243,233,216,0.12)",
            }}
          >
            <Play size={30} className="text-[var(--ink-2)] ml-1" />
          </div>
        </div>
        <div className="text-[13.5px] text-[var(--ink-2)]">{emptyHint ?? "这里空空如也"}</div>
        {emptyAction && (
          <button className="btn-primary mt-1" onClick={emptyAction.onClick}>
            {emptyAction.label}
          </button>
        )}
      </div>
    );
  }

  const renderLocal = (t: TrackMeta, i: number, idxNum: number) => {
    const active = current?.kind === "track" && current.id === t.id;
    // 窗口化下滚动新挂载的行不重播入场动画（只有首屏行交错浮现）
    const animCls = dragMode || idxNum < ANIM_ROWS ? "anim-row" : "";
    return (
      <div
        key={`local-${t.id}`}
        {...(dragSortable && onDragReorder ? rowProps(idxNum) : {})}
        style={{ ["--row-idx" as string]: Math.min(idxNum, 12) }}
        className={`${animCls} group grid grid-cols-[56px_minmax(200px,460px)_minmax(180px,300px)_92px_136px] items-center gap-4 h-[64px] px-4 rounded-2xl transition-colors duration-150 cursor-default ${
          active ? "bg-[var(--accent-weak)]" : "hover:bg-[var(--shade-hover)]"
        }`}
        onDoubleClick={() => playTracks(tracks, i)}
        onContextMenu={(e) => {
          e.preventDefault();
          setMenu({ x: e.clientX, y: e.clientY, track: t });
        }}
      >
        {/* 序号 / 播放 */}
        <div className="relative h-12 flex items-center justify-center">
          <span
            className={`text-[12.5px] tabular-nums transition-opacity ${
              active
                ? "text-[var(--accent)] font-bold"
                : "text-[var(--ink-3)] group-hover:opacity-0"
            }`}
          >
            {String(idxNum + 1).padStart(2, "0")}
          </span>
          <button
            className={`absolute inset-0 m-auto w-9 h-9 rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all hover:scale-105 ${
              active
                ? "text-[var(--accent)]"
                : "bg-[var(--play-btn-bg)] text-[var(--play-btn-on)] shadow-[0_2px_10px_rgba(0,0,0,0.18)]"
            }`}
            onClick={() => (active ? useStore.getState().togglePlay() : playTracks(tracks, i))}
            title={active ? "播放 / 暂停" : "播放"}
          >
            <Play size={14} className="fill-current ml-px" />
          </button>
        </div>

        {/* 封面 + 标题 */}
        <div className="flex items-center gap-4 min-w-0">
          <CoverImg
            src={t.cover}
            seed={t.title}
            className="hover-lift w-11 h-11 rounded-xl shadow-[var(--cover-shadow-sm)] shrink-0"
            iconSize={16}
          />
          <div className="min-w-0">
            <div className="flex items-center gap-2.5 min-w-0">
              <span
                className={`text-[13.5px] truncate ${
                  active ? "text-[var(--accent-strong)] font-semibold" : "text-[var(--ink)]"
                }`}
              >
                {trackTitle(t)}
              </span>
              {active && (
                <span className="shrink-0 inline-flex">
                  <span className={`eq-bars ${playing ? "" : "paused"}`}>
                    <i />
                    <i />
                    <i />
                  </span>
                </span>
              )}
              {t.missing && (
                <span
                  className="text-[9.5px] px-1.5 py-0.5 rounded bg-[var(--shade-strong)] text-[var(--ink-3)] font-medium shrink-0"
                  title="文件已不在监控文件夹中（记录保留，重新添加文件夹后恢复）"
                >
                  文件缺失
                </span>
              )}
            </div>
            <div className="text-[12px] text-[var(--ink-3)] truncate mt-1">
              {trackArtist(t)}
            </div>
          </div>
        </div>

        {/* 专辑 */}
        <div className="text-[12.5px] text-[var(--ink-3)] truncate">
          {t.album || "未知专辑"}
        </div>

        {/* 格式 / 时长（与表头同列，右对齐） */}
        <div className="flex items-center justify-end gap-3">
          <span className="text-[10.5px] px-2 py-[3px] rounded-md bg-[var(--shade)] text-[var(--ink-2)] font-semibold tracking-wider">
            {t.format || "AUDIO"}
          </span>
          <span className="text-[12.5px] text-[var(--ink-2)] tabular-nums w-10 text-right">
            {fmtTime(t.duration * 1000)}
          </span>
        </div>

        {/* 操作 */}
        <div className="flex items-center justify-end gap-1 pr-1">
          <button
            className="btn-ghost w-8 h-8"
            onClick={() => toggleLike(t.id)}
            title={t.liked ? "取消喜欢" : "喜欢"}
          >
            <Heart
              size={15}
              className={
                t.liked
                  ? "fill-[#e0533f] text-[#e0533f]"
                  : "opacity-0 group-hover:opacity-100"
              }
            />
          </button>
          <button
            className="btn-ghost w-8 h-8"
            onClick={(e) => {
              const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
              // 菜单右缘对齐按钮右缘、顶边贴按钮下方（190 宽，clamp 负责防出屏）
              setMenu({ x: r.right - 190, y: r.bottom + 4, track: t });
            }}
          >
            <MoreHorizontal size={16} className="opacity-0 group-hover:opacity-100" />
          </button>
        </div>
      </div>
    );
  };

  const renderOnline = (e: PlaylistEntryMeta, i: number, idxNum: number) => {
    const active =
      current?.kind === e.kind &&
      (e.kind === "qq"
        ? current.qid === e.onlineId
        : current.nid === Number(e.onlineId));
    // 播放失败（无版权/下架）：整行置灰 + 无版权标记
    const failKey = e.kind === "qq" ? `qq:${e.onlineId}` : `netease:${e.onlineId}`;
    const dead = unavailable[failKey] != null;
    const animCls = dragMode || idxNum < ANIM_ROWS ? "anim-row" : "";
    return (
      <div
        key={`online-${e.kind}-${e.onlineId}`}
        {...(dragSortable && onDragReorder ? rowProps(idxNum) : {})}
        style={{ ["--row-idx" as string]: Math.min(idxNum, 12) }}
        title={dead ? `无法播放：${unavailable[failKey]}` : undefined}
        className={`${animCls} group grid grid-cols-[56px_minmax(200px,460px)_minmax(180px,300px)_92px_136px] items-center gap-4 h-[64px] px-4 rounded-2xl transition-colors duration-150 cursor-default ${
          active ? "bg-[var(--accent-weak)]" : "hover:bg-[var(--shade-hover)]"
        } ${dead ? "opacity-45" : ""}`}
        onDoubleClick={() => playEntries(onlineEntries, Math.max(0, i))}
        onContextMenu={(ev) => {
          ev.preventDefault();
          setOnlineMenu({ x: ev.clientX, y: ev.clientY, entry: e });
        }}
      >
        <div className="relative h-12 flex items-center justify-center">
          <span
            className={`text-[12.5px] tabular-nums transition-opacity ${
              active
                ? "text-[var(--accent)] font-bold"
                : "text-[var(--ink-3)] group-hover:opacity-0"
            }`}
          >
            {String(idxNum + 1).padStart(2, "0")}
          </span>
          <button
            className={`absolute inset-0 m-auto w-9 h-9 rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all hover:scale-105 ${
              active ? "text-[var(--accent)]" : "bg-[var(--accent)] text-[var(--accent-on)]"
            }`}
            onClick={() => playEntries(onlineEntries, Math.max(0, i))}
            title="播放"
          >
            <Play size={14} className="fill-current ml-px" />
          </button>
        </div>
        <div className="flex items-center gap-4 min-w-0">
          <CoverImg
            src={e.cover}
            seed={e.title}
            className="hover-lift w-11 h-11 rounded-xl shadow-[var(--cover-shadow-sm)] shrink-0"
            iconSize={16}
          />
          <div className="min-w-0">
            <div className="flex items-center gap-2.5 min-w-0">
              <span
                className={`text-[13.5px] truncate ${
                  active ? "text-[var(--accent-strong)] font-semibold" : "text-[var(--ink)]"
                }`}
              >
                {e.title}
              </span>
              <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-[var(--shade-strong)] text-[var(--ink-3)] font-medium shrink-0">
                {e.kind === "netease" ? "网易云" : "QQ音乐"}
              </span>
              {e.vip && !dead && (
                <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-[var(--accent-weak)] text-[var(--accent-strong)] font-bold shrink-0">
                  VIP
                </span>
              )}
              {dead && (
                <span
                  className="text-[9.5px] px-1.5 py-0.5 rounded bg-[var(--shade-strong)] text-[var(--ink-3)] font-medium shrink-0 flex items-center gap-1"
                  title={unavailable[failKey]}
                >
                  <Ban size={9} />
                  无版权
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
            <div className="text-[12px] text-[var(--ink-3)] truncate mt-1">
              {e.artist || "未知艺术家"}
            </div>
          </div>
        </div>
        <div className="text-[12.5px] text-[var(--ink-3)] truncate">
          {e.album || "未知专辑"}
        </div>
        <div className="flex items-center justify-end gap-3">
          <span className="text-[10.5px] px-2 py-[3px] rounded-md bg-[var(--shade)] text-[var(--ink-2)] font-semibold tracking-wider">
            在线
          </span>
          <span className="text-[12.5px] text-[var(--ink-2)] tabular-nums w-10 text-right">
            {fmtTime(e.duration * 1000)}
          </span>
        </div>
        <div className="flex items-center justify-end gap-1 pr-1">
          <button
            className="btn-ghost w-8 h-8"
            onClick={() =>
              toggleLikeOnline({
                kind: e.kind,
                id: e.kind === "qq" ? (e.onlineId ?? "") : Number(e.onlineId),
                name: e.title,
                artist: e.artist,
                album: e.album,
                cover: e.cover,
                durationMs: Math.round(e.duration * 1000),
                mediaMid: e.mediaMid,
                vip: e.vip,
              })
            }
            title="取消喜欢"
          >
            <Heart size={15} className="fill-[#e0533f] text-[#e0533f]" />
          </button>
          <button
            className="btn-ghost w-8 h-8"
            onClick={(ev) => {
              const r = (ev.currentTarget as HTMLElement).getBoundingClientRect();
              // 与本地行一致：菜单右缘对齐按钮、顶边贴按钮下方
              setOnlineMenu({ x: r.right - 190, y: r.bottom + 4, entry: e });
            }}
          >
            <MoreHorizontal size={16} className="opacity-0 group-hover:opacity-100" />
          </button>
        </div>
      </div>
    );
  };

  return (
    <>
      <div
        ref={win.containerRef}
        onScroll={win.onScroll}
        className={`flex-1 min-h-0 overflow-y-auto ${inCard ? "px-2.5 pt-2.5 pb-[90px]" : "px-5 pb-[62px]"}`}
      >
      {dragMode ? (
        // 拖拽模式：全量渲染（useDragList 直接按 container.children 顺序读写 DOM）
        rows.map(renderRow)
      ) : (
        // 窗口化：上下 spacer 撑起总高度，DOM 只有可视区 ± overscan 行
        <>
          <div style={{ height: win.start * ROW_H }} aria-hidden />
          {rows.slice(win.start, win.end).map(renderRow)}
          <div style={{ height: Math.max(0, rows.length - win.end) * ROW_H }} aria-hidden />
        </>
      )}
      </div>

      {/* 在线条目右键菜单（Portal 到 body：fixed 定位的包含块必须是视口；
          留在 .glass 卡片内会被 backdrop-filter 变成相对卡片定位，
          导致菜单弹到远离按钮的位置） */}
      {onlineMenu &&
        createPortal(
          <div
            ref={menuRef}
            className="fixed z-[75] w-[190px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
            style={{ left: menuX, top: menuY }}
            onMouseDown={(ev) => ev.stopPropagation()}
            onMouseLeave={() => setOnlineMenu(null)}
          >
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              const idx = Math.max(0, onlineEntries.indexOf(onlineMenu.entry));
              playEntries(onlineEntries, idx);
              setOnlineMenu(null);
            }}
          >
            <Play size={13} /> 播放
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              const item = entryToQueueItem(onlineMenu.entry);
              if (item) playNext(item);
              setOnlineMenu(null);
            }}
          >
            <Play size={13} /> 下一首播放
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              const item = entryToQueueItem(onlineMenu.entry);
              if (item) addToQueue(item);
              setOnlineMenu(null);
            }}
          >
            <Play size={13} /> 加入队列
          </button>
          <div className="my-1 mx-2 border-t border-[var(--line)]" />
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              downloadOnline({
                kind: onlineMenu.entry.kind,
                id:
                  onlineMenu.entry.kind === "qq"
                    ? (onlineMenu.entry.onlineId ?? "")
                    : Number(onlineMenu.entry.onlineId),
                name: onlineMenu.entry.title,
                artist: onlineMenu.entry.artist,
                album: onlineMenu.entry.album,
                cover: onlineMenu.entry.cover,
                durationMs: Math.round(onlineMenu.entry.duration * 1000),
                mediaMid: onlineMenu.entry.mediaMid,
              });
              setOnlineMenu(null);
            }}
          >
            <Play size={13} /> 下载到本地
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              toggleLikeOnline({
                kind: onlineMenu.entry.kind,
                id:
                  onlineMenu.entry.kind === "qq"
                    ? (onlineMenu.entry.onlineId ?? "")
                    : Number(onlineMenu.entry.onlineId),
                name: onlineMenu.entry.title,
                artist: onlineMenu.entry.artist,
                album: onlineMenu.entry.album,
                cover: onlineMenu.entry.cover,
                durationMs: Math.round(onlineMenu.entry.duration * 1000),
                mediaMid: onlineMenu.entry.mediaMid,
                vip: onlineMenu.entry.vip,
              });
              setOnlineMenu(null);
            }}
          >
            <Play size={13} /> 取消喜欢
          </button>
        </div>,
          document.body
        )}

      {/* 右键菜单（Portal 到 body，同上：脱离 .glass 祖保，fixed 才相对视口） */}
      {menu &&
        createPortal(
        <div
          ref={menuRef}
          className="fixed z-[75] w-[190px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
          style={{ left: menuX, top: menuY }}
          onMouseDown={(ev) => ev.stopPropagation()}
          onMouseLeave={() => setMenu(null)}
        >
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              playTracks(tracks, tracks.indexOf(menu.track));
              setMenu(null);
            }}
          >
            <Play size={13} /> 播放
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              playNext({ kind: "track", id: menu.track.id });
              setMenu(null);
            }}
          >
            <Play size={13} /> 下一首播放
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              addToQueue({ kind: "track", id: menu.track.id });
              setMenu(null);
            }}
          >
            <Play size={13} /> 加入队列
          </button>
          <div className="my-1 mx-2 border-t border-[var(--line)]" />
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              setPickerFor(menu.track);
              setMenu(null);
            }}
          >
            <ListMusic size={13} /> 添加到播放列表…
          </button>
        </div>,
        document.body
      )}

      {/* 添加到播放列表弹窗 */}
      <Modal
        open={!!pickerFor}
        onClose={() => setPickerFor(null)}
        title="添加到播放列表"
        width={380}
      >
        <div className="flex flex-col gap-1.5 max-h-[260px] overflow-y-auto">
          {playlists.map((p) => (
            <button
              key={p.id}
              className="h-10 px-3 rounded-lg text-left text-[13px] text-[var(--ink)] hover:bg-[var(--shade)] flex items-center justify-between transition-colors"
              onClick={async () => {
                if (pickerFor) await useStore.getState().addToPlaylist(p.id, pickerFor.id);
                setPickerFor(null);
              }}
            >
              <span className="truncate">{p.name}</span>
              <span className="text-[11px] text-[var(--ink-2)]">
                {p.entries.length} 首
              </span>
            </button>
          ))}
          {!playlists.length && (
            <div className="text-[12.5px] text-[var(--ink-2)] py-2">
              还没有播放列表，在下方创建
            </div>
          )}
        </div>
        <div className="flex gap-2 mt-3">
          <input
            type="text"
            value={newPlName}
            onChange={(e) => setNewPlName(e.target.value)}
            placeholder="新播放列表名称"
            className="flex-1 h-9 rounded-lg bg-[var(--shade)] border border-[var(--line)] px-3 text-[13px] focus:border-[var(--line)] outline-none"
          />
          <button
            className="btn-secondary"
            onClick={async () => {
              if (!newPlName.trim() || !pickerFor) return;
              const pid = await createPlaylist(newPlName.trim());
              if (pid >= 0) await useStore.getState().addToPlaylist(pid, pickerFor.id);
              setNewPlName("");
              setPickerFor(null);
            }}
          >
            创建并添加
          </button>
        </div>
      </Modal>
    </>
  );
}
