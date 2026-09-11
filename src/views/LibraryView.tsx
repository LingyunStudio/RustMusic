import { useEffect, useMemo, useState } from "react";
import { Clock3, Heart, Library, MoveVertical, Play, Search, Shuffle, X } from "lucide-react";
import { useStore } from "../store";
import TrackList, { type SortKey } from "../components/TrackList";
import { matchSearch } from "../utils";
import type { PlaylistEntryMeta, TrackMeta } from "../types";

type Mode = "library" | "liked" | "recent";

type MergedRowList = (
  | { type: "local"; t: TrackMeta }
  | { type: "online"; e: PlaylistEntryMeta }
)[];

const SORTS: { key: SortKey; label: string }[] = [
  { key: "manual", label: "手动排序" },
  { key: "added", label: "添加时间" },
  { key: "title", label: "标题" },
  { key: "artist", label: "艺术家" },
  { key: "album", label: "专辑" },
  { key: "duration", label: "时长" },
];

const META: Record<Mode, { title: string; sub: string; icon: typeof Library }> = {
  library: { title: "资料库", sub: "你的全部音乐", icon: Library },
  liked: { title: "我喜欢", sub: "收藏的心动之歌", icon: Heart },
  recent: { title: "最近播放", sub: "刚刚听过的旋律", icon: Clock3 },
};

export default function LibraryView({ mode }: { mode: Mode }) {
  const tracks = useStore((s) => s.tracks);
  const search = useStore((s) => s.search);
  const setSearch = useStore((s) => s.setSearch);
  const scan = useStore((s) => s.scan);
  const playTracks = useStore((s) => s.playTracks);
  const folders = useStore((s) => s.folders);
  const addFolderByDialog = useStore((s) => s.addFolderByDialog);
  const likedOnline = useStore((s) => s.likedOnline);
  const recentOnline = useStore((s) => s.recentOnline);
  const playEntries = useStore((s) => s.playEntries);
  const manualOrder = useStore((s) => s.manualOrder);
  const rowKeyOf = useStore((s) => s.rowKeyOf);
  const saveManualOrder = useStore((s) => s.saveManualOrder);
  const loadManualOrder = useStore((s) => s.loadManualOrder);
  // 默认排序“添加时间”（资料库=入库顺序；最近播放/我喜欢=列表时间，本地与在线归并）
  const [sortKey, setSortKey] = useState<SortKey>("added");
  const [dir, setDir] = useState<1 | -1>(-1);

  // 手动排序仅对“资料库/我喜欢”开放（最近播放是时间线语义）
  const orderList = mode === "liked" ? "liked" : "library";
  const manualMap = manualOrder[orderList] ?? {};
  const manualActive = sortKey === "manual" && mode !== "recent";

  // 手动序号：本地/在线行统一取 manual_order 里的 pos（0 = 无记录）
  const rowManualPos = (
    r: { type: "local"; t: TrackMeta } | { type: "online"; e: PlaylistEntryMeta }
  ) =>
    manualMap[
      r.type === "local"
        ? rowKeyOf({ kind: "local", trackId: r.t.id })
        : rowKeyOf({ kind: r.e.kind, onlineId: r.e.onlineId })
    ] ?? 0;

  // “我喜欢” = 本地喜欢（含文件已移除的软删除记录，仅为记录不删） + 在线喜欢
  const onlineLikedEntries: PlaylistEntryMeta[] = useMemo(() => {
    if (mode !== "liked") return [];
    return likedOnline.filter((e) => {
      if (!search) return true;
      const s = search.toLowerCase();
      return (
        e.title.toLowerCase().includes(s) ||
        e.artist.toLowerCase().includes(s) ||
        e.album.toLowerCase().includes(s)
      );
    });
  }, [mode, likedOnline, search]);

  // “最近播放”的在线曲目部分（含本地软删除记录：文件没了记录还在）
  const onlineRecentEntries: PlaylistEntryMeta[] = useMemo(() => {
    if (mode !== "recent") return [];
    return recentOnline.filter((e) => {
      if (!search) return true;
      const s = search.toLowerCase();
      return (
        e.title.toLowerCase().includes(s) ||
        e.artist.toLowerCase().includes(s) ||
        e.album.toLowerCase().includes(s)
      );
    });
  }, [mode, recentOnline, search]);

  const filtered = useMemo(() => {
    let list: TrackMeta[];
    if (mode === "liked") {
      // 喜欢：文件被移出文件夹的曲目保留显示（missing 记录仍在 tracks 表）
      list = tracks.filter((t) => t.liked);
    } else if (mode === "recent") {
      list = tracks.filter((t) => t.lastPlayed > 0);
    } else {
      // 资料库：只显示文件存在的曲目
      list = tracks.filter((t) => !t.missing);
    }
    return list.filter((t) => matchSearch(t, search));
  }, [tracks, mode, search]);

  // 排序键的统一取值：本地与在线行都给出可比较值（字符串用 localeCompare，
  // 数字直接相减），归并排序对所有键生效——此前只有“添加时间”走归并，
  // 其他键在线条目不参与排序，表现为“只排了部分歌曲”
  const rowKeys = (
    r: { type: "local"; t: TrackMeta } | { type: "online"; e: PlaylistEntryMeta },
    mode: Mode,
    key: SortKey
  ): string | number => {
    const t = r.type === "local" ? r.t : null;
    const e = r.type === "online" ? r.e : null;
    switch (key) {
      case "manual":
        // 手动顺序：持久序号在前，未记录的新增行按添加时间排在其后
        return rowManualPos(r) > 0
          ? rowManualPos(r)
          : mode === "liked"
            ? -(t ? t.likedAt : (e!.likedAt ?? 0))
            : -(t ? t.id : 0);
      case "added":
        // 按视图取义：资料库=入库顺序；最近播放=最近播放时间；我喜欢=喜欢时间
        if (mode === "liked") return t ? t.likedAt : (e!.likedAt ?? 0);
        if (mode === "recent") return t ? t.lastPlayed : (e!.lastPlayed ?? 0);
        return t ? t.id : 0; // 资料库无在线条目
      case "title":
        return t ? t.title : e!.title;
      case "artist":
        return t ? t.artist : e!.artist;
      case "album":
        return t ? t.album : e!.album;
      case "duration":
        return t ? t.duration : e!.duration;
    }
  };

  const rowCmp = (
    a: { type: "local"; t: TrackMeta } | { type: "online"; e: PlaylistEntryMeta },
    b: { type: "local"; t: TrackMeta } | { type: "online"; e: PlaylistEntryMeta }
  ) => {
    const ka = rowKeys(a, mode, sortKey);
    const kb = rowKeys(b, mode, sortKey);
    // 手动排序时统一升序（序号小的在前）；未记录项用负时间戳天然“新的在前”
    const d =
      typeof ka === "string" || typeof kb === "string"
        ? String(ka).localeCompare(String(kb), "zh")
        : sortKey === "manual"
          ? (ka as number) - (kb as number)
          : dir * ((ka as number) - (kb as number));
    if (d !== 0) return d;
    // 并列项：本地在前，保持稳定
    return a.type === b.type ? 0 : a.type === "local" ? -1 : 1;
  };

  // 资料库视图无在线条目，直接排序本地曲目；
  // 我喜欢/最近播放统一归并排序（本地与在线交叉成一条序列）
  const mergedRows = useMemo(() => {
    if (mode !== "recent" && mode !== "liked") return [];
    const onlineEntries = mode === "recent" ? onlineRecentEntries : onlineLikedEntries;
    const rows: (
      | { type: "local"; t: TrackMeta }
      | { type: "online"; e: PlaylistEntryMeta }
    )[] = [
      ...filtered.map((t) => ({ type: "local" as const, t })),
      ...onlineEntries.map((e) => ({ type: "online" as const, e })),
    ];
    rows.sort((a, b) => (sortKey === "manual" ? 1 : dir) * rowCmp(a, b));
    return rows;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, sortKey, dir, filtered, onlineRecentEntries, onlineLikedEntries, manualMap]);

  // 资料库视图的本地排序结果（mergedRows 不覆盖时 TrackList 需要 tracks 已排序）
  const sortedTracks = useMemo(() => {
    if (mode !== "library") return filtered;
    const m = sortKey === "manual" ? 1 : dir;
    return [...filtered].sort((a, b) => m * rowCmp({ type: "local", t: a }, { type: "local", t: b }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filtered, mode, sortKey, dir, manualMap]);

  const { title, sub, icon: Icon } = META[mode];
  // “最近播放”追加在线曲目；“我喜欢”追加在线喜欢
  const onlineEntries = mode === "recent" ? onlineRecentEntries : onlineLikedEntries;
  // 归并模式（时间排序）下整页可播放序列：本地曲目转 local 伪条目，统一走 playEntries
  const mergedEntries = useMemo(() => {
    if (mergedRows.length !== 0) {
      return mergedRows.map((r) =>
        r.type === "local"
          ? ({
              rowid: 0,
              kind: "local" as const,
              trackId: r.t.id,
              onlineId: null,
              title: r.t.title,
              artist: r.t.artist,
              album: r.t.album,
              cover: r.t.cover,
              duration: r.t.duration,
            } as PlaylistEntryMeta)
          : r.e
      );
    }
    return [];
  }, [mergedRows]);
  const totalSecs = Math.floor(
    filtered.reduce((a, t) => a + t.duration, 0) +
      onlineEntries.reduce((a, e) => a + e.duration, 0)
  );
  const totalDesc =
    totalSecs >= 3600
      ? `${Math.floor(totalSecs / 3600)} 小时 ${Math.floor((totalSecs % 3600) / 60)} 分钟`
      : `${Math.floor(totalSecs / 60)} 分钟`;
  const totalCount = filtered.length + onlineEntries.length;
  // 播放全部 / 随机：归并模式播完整交叉序列，否则只播本地曲目
  const playAll = (random: boolean) => {
    if (mergedEntries.length) {
      playEntries(
        mergedEntries,
        random ? Math.floor(Math.random() * mergedEntries.length) : 0
      );
    } else if (sortedTracks.length) {
      playTracks(
        sortedTracks,
        random ? Math.floor(Math.random() * sortedTracks.length) : 0
      );
    }
  };

  // 全量行序列（未搜索过滤；搜索过滤时拖拽提交仍需知道完整顺序）
  const allRows = useMemo<MergedRowList>(() => {
    if (mode === "liked") {
      const localAll = tracks.filter((t) => t.liked);
      return [
        ...localAll.map((t) => ({ type: "local" as const, t })),
        ...likedOnline.map((e) => ({ type: "online" as const, e })),
      ];
    }
    if (mode === "library") {
      return tracks
        .filter((t) => !t.missing)
        .map((t) => ({ type: "local" as const, t }));
    }
    return [];
  }, [mode, tracks, likedOnline]);

  // 拖拽提交：按渲染序列重排。行序列用 key 表达（track:<id> /
  // netease:<rid> / qq:<rid>），与 manual_order 一致。
  // 搜索过滤时渲染序列只有可见行——按拖拽结果推算全量顺序：
  // 被拖行插入目标位置，其余行保持原相对顺序（未在可见集里的行位置不动）
  const dragRows = useMemo<MergedRowList>(() => {
    if (mode === "recent") return [];
    if (mode === "liked") return mergedRows;
    // 资料库：本地行（无在线条目）
    return sortedTracks.map((t) => ({ type: "local" as const, t }));
  }, [mode, mergedRows, sortedTracks]);

  const keyOfRow = (r: MergedRowList[number]) =>
    r.type === "local"
      ? rowKeyOf({ kind: "local", trackId: r.t.id })
      : rowKeyOf({ kind: r.e.kind, onlineId: r.e.onlineId });

  const onDragReorder = (from: number, to: number) => {
    const rows = dragRows;
    if (from < 0 || from >= rows.length || to < 0 || to >= rows.length) return;
    // 渲染序列（可能被搜索过滤）重排
    const visible = [...rows];
    const [moved] = visible.splice(from, 1);
    visible.splice(to, 0, moved);
    if (!search) {
      // 无过滤：渲染序列即全量，直接存
      saveManualOrder(orderList, visible.map(keyOfRow));
      return;
    }
    // 有过滤：全量序列里先移除被拖行，再把它插到目标行的位置
    const full = allRows.map(keyOfRow);
    const movedKey = keyOfRow(moved);
    const anchorKey = to < visible.length - 1 ? keyOfRow(visible[to + 1]) : null;
    const without = full.filter((k) => k !== movedKey);
    const anchorIdx = anchorKey ? without.indexOf(anchorKey) : -1;
    const insertAt = anchorIdx >= 0 ? anchorIdx : without.length;
    without.splice(insertAt, 0, movedKey);
    saveManualOrder(orderList, without);
  };

  // 首次切入“手动排序”且无历史记录时预填当前显示顺序（否则第一拖只存 2 行）
  const [manualSeeded, setManualSeeded] = useState(false);
  useEffect(() => {
    if (!manualActive || manualSeeded || dragRows.length === 0) return;
    setManualSeeded(true);
    if (Object.keys(manualMap).length === 0) {
      saveManualOrder(
        orderList,
        dragRows.map((r) =>
          r.type === "local"
            ? rowKeyOf({ kind: "local", trackId: r.t.id })
            : rowKeyOf({ kind: r.e.kind, onlineId: r.e.onlineId })
        )
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [manualActive, manualSeeded, dragRows]);

  // 切回其他排序时不再重复预填（下次再切手动排序重新判断即可）
  useEffect(() => {
    if (!manualActive && manualSeeded) setManualSeeded(false);
  }, [manualActive, manualSeeded]);

  const onSort = (k: SortKey) => {
    if (k === sortKey) {
      if (k !== "manual") setDir((d) => (d === 1 ? -1 : 1));
      return;
    }
    setSortKey(k);
    setDir(k === "title" || k === "artist" || k === "album" ? 1 : -1);
  };

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      {/* 头部（可换行：窗口窄时标题块与搜索/按钮组折成上下两行，
          而不是各自内部挤成竖排） */}
      <header className="px-8 pt-7 pb-5">
        <div className="flex flex-wrap items-end justify-between gap-x-6 gap-y-4">
          <div className="min-w-0 anim-rise">
            <div className="flex items-center gap-2 text-[11px] text-[var(--ink-3)] tracking-[0.24em] mb-2">
              <Icon size={13} />
              {sub}
              {mode === "library" && scan.active && (
                <span className="text-[var(--accent)] tracking-normal flex items-center gap-1.5">
                  · <LoaderSpin /> 扫描中 {scan.total ? `${scan.done}/${scan.total}` : ""}
                </span>
              )}
            </div>
            <h1 className="text-[30px] font-extrabold leading-none tracking-tight text-[var(--ink)]">
              {title}
            </h1>
            <div className="flex items-center gap-3 mt-3 text-[12.5px] text-[var(--ink-2)]">
              <span className="tabular-nums">{totalCount} 首</span>
              {totalSecs > 0 && (
                <>
                  <span className="w-1 h-1 rounded-full bg-[var(--ink-3)]" />
                  <span className="tabular-nums">{totalDesc}</span>
                </>
              )}
            </div>
            {mode === "library" && !folders.length && !scan.active && (
              <p className="text-[12.5px] text-[var(--ink-3)] mt-3">
                添加音乐文件夹，或直接把文件 / 文件夹拖进窗口
              </p>
            )}
          </div>

          <div className="flex items-center gap-3 shrink-0 anim-rise max-w-full">
            {/* 搜索框（与网易云/QQ 页风格一致）；极窄时允许收缩 */}
            <div className="relative w-[420px] max-w-full shrink">
              <Search
                size={14}
                className="absolute left-3.5 top-1/2 -translate-y-1/2 text-[var(--ink-3)]"
              />
              <input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="搜索歌曲、艺术家、专辑…"
                className="w-full h-10 rounded-xl bg-[var(--shade)] border border-[var(--line)] pl-9 pr-8 text-[12.5px] text-[var(--ink)] placeholder:text-[var(--ink-3)] focus:border-[rgba(240,162,74,0.45)] transition-colors"
              />
              {search && (
                <button
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[var(--ink-3)] hover:text-[var(--ink)]"
                  onClick={() => setSearch("")}
                >
                  <X size={13} />
                </button>
              )}
            </div>
            {(sortedTracks.length > 0 || mergedEntries.length > 0) && (
              <>
                <button
                  className="btn-secondary"
                  onClick={() => playAll(true)}
                >
                  <Shuffle size={14} />
                  随机播放
                </button>
                <button className="btn-primary" onClick={() => playAll(false)}>
                  <Play size={14} className="fill-current" />
                  播放全部
                </button>
              </>
            )}
          </div>
        </div>

        {/* 排序 */}
        <div className="flex items-center gap-2 mt-5">
          <span className="text-[11.5px] text-[var(--ink-3)] mr-1">排序</span>
          {SORTS.filter((s) => s.key !== "manual" || mode !== "recent").map((s) => (
            <button
              key={s.key}
              onClick={() => onSort(s.key)}
              className={`chip ${
                sortKey === s.key
                  ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                  : "text-[var(--ink-3)] hover:text-[var(--ink-2)] hover:bg-[var(--shade-hover)]"
              }`}
            >
              {s.key === "manual" && <MoveVertical size={11} className="mr-0.5 -ml-0.5 inline" />}
              {s.label}
              {sortKey === s.key && s.key !== "manual" && (
                <span className="ml-0.5">{dir === 1 ? "↑" : "↓"}</span>
              )}
            </button>
          ))}
          {manualActive && (
            <span className="text-[11px] text-[var(--ink-3)] ml-1">
              长按歌曲可拖动调序
            </span>
          )}
        </div>
      </header>

      {/* 曲目卡片（底边界抬到播放条上方，留 4px 空隙：播放条总占位 64+16+4=84px） */}
      <div className="flex-1 min-h-0 flex flex-col px-6 pb-[86px]">
        <div className="glass rounded-3xl flex-1 min-h-0 flex flex-col overflow-hidden">
          {(sortedTracks.length > 0 || mergedRows.length > 0) && (
            <div className="grid grid-cols-[56px_minmax(200px,460px)_minmax(180px,300px)_92px_136px] items-center gap-4 h-10 px-5 border-b border-[var(--line)] text-[10.5px] text-[var(--ink-3)] tracking-[0.18em]">
              <span className="text-center">序号</span>
              <span>歌曲</span>
              <span>专辑</span>
              <span className="text-right">时长</span>
              <span className="text-right">操作</span>
            </div>
          )}
          <TrackList
            tracks={sortedTracks}
            inCard
            onlineEntries={onlineEntries}
            mergedRows={mergedRows.length ? mergedRows : undefined}
            dragSortable={manualActive}
            onDragReorder={manualActive ? onDragReorder : undefined}
            emptyHint={
              mode === "library"
                ? "资料库还是空的"
                : mode === "liked"
                  ? "还没有喜欢的音乐"
                  : "还没有播放记录"
            }
            emptyAction={
              mode === "library"
                ? { label: "添加音乐文件夹", onClick: addFolderByDialog }
                : mode === "liked"
                  ? {
                      label: "去资料库逛逛",
                      onClick: () => useStore.getState().setView("library"),
                    }
                  : undefined
            }
          />
        </div>
      </div>
    </div>
  );
}

function LoaderSpin() {
  return (
    <svg
      className="animate-spin"
      width="12"
      height="12"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2.5"
      strokeLinecap="round"
    >
      <path d="M21 12a9 9 0 1 1-6.219-8.56" />
    </svg>
  );
}
