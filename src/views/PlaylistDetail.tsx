import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  Ban,
  Check,
  FileDown,
  FileUp,
  Heart,
  ListChecks,
  ListMusic,
  MoreHorizontal,
  Pause,
  Pencil,
  Play,
  Search,
  Shuffle,
  Trash2,
  Cloud,
  X,
} from "lucide-react";
import { useStore } from "../store";
import { useI18n } from "../i18n";
import { useDragList } from "../hooks/useDragList";
import { useVirtualWindow } from "../hooks/useVirtualWindow";
import { ConfirmModal, InputModal } from "../components/Dialogs";
import LocateCurrentPill from "../components/LocateCurrentPill";
import { useLocatePill } from "../hooks/useLocatePill";
import { clampMenuPos, fmtTime, matchSearch } from "../utils";
import CoverImg from "../components/CoverImg";
import ViewShell from "../components/ViewShell";
import Modal from "../components/Modal";
import type { PlaylistEntryMeta } from "../types";

type DetailRow =
  | {
      entry: PlaylistEntryMeta;
      kind: "track";
      id: number;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      liked: boolean;
    }
  | {
      entry: PlaylistEntryMeta;
      kind: "netease";
      id: number;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      liked: boolean;
    }
  | {
      entry: PlaylistEntryMeta;
      kind: "qq";
      id: string;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      liked: boolean;
    }
  | {
      entry: PlaylistEntryMeta;
      kind: "kugou";
      id: string;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      liked: boolean;
    }
  | {
      entry: PlaylistEntryMeta;
      kind: "bilibili";
      id: string;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      liked: boolean;
    }
  | {
      entry: PlaylistEntryMeta;
      kind: "navidrome";
      id: string;
      name: string;
      artist: string;
      album: string;
      cover: string;
      durationMs: number;
      liked: boolean;
    };

export default function PlaylistDetail({ id }: { id: number }) {
  const { tr } = useI18n();
  const uiTheme = useStore((s) => s.uiTheme);
  const isLiquid = uiTheme === "liquid";
  const isSkeuo = uiTheme === "skeuo";
  const playlists = useStore((s) => s.playlists);
  const tracks = useStore((s) => s.tracks);
  const search = useStore((s) => s.search);
  const openDetailPage = useStore((s) => s.openDetailPage);
  const playing = useStore((s) => s.playing);
  const playEntries = useStore((s) => s.playEntries);
  const entryToQueueItem = useStore((s) => s.entryToQueueItem);
  const removePlaylistEntryRow = useStore((s) => s.removePlaylistEntryRow);
  const removePlaylistEntryRows = useStore((s) => s.removePlaylistEntryRows);
  const toast = useStore((s) => s.toast);
  const toggleLike = useStore((s) => s.toggleLike);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const downloadOnline = useStore((s) => s.downloadOnline);
  const playNext = useStore((s) => s.playNext);
  const addToQueue = useStore((s) => s.addToQueue);
  const deletePlaylist = useStore((s) => s.deletePlaylist);
  const current = useStore((s) => s.current);
  const savedOnline = useStore((s) => s.savedOnline);
  const [confirmDel, setConfirmDel] = useState(false);
  const [moreOpen, setMoreOpen] = useState(false);
  const [batchMode, setBatchMode] = useState(false);
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [confirmBatchRemove, setConfirmBatchRemove] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; e: PlaylistEntryMeta } | null>(
    null
  );
  const reorderPlaylist = useStore((s) => s.reorderPlaylist);
  const renamePlaylist = useStore((s) => s.renamePlaylist);
  const setSearch = useStore((s) => s.setSearch);
  // Ctrl+F 聚焦本页搜索框
  const plSearchRef = useRef<HTMLInputElement | null>(null);
  useEffect(() => {
    const focus = () => plSearchRef.current?.focus();
    window.addEventListener("rm:focus-search", focus);
    return () => window.removeEventListener("rm:focus-search", focus);
  }, []);
  const exportM3u = useStore((s) => s.exportPlaylistM3u);
  const importM3u = useStore((s) => s.importPlaylistM3u);

  // m3u 导出：保存对话框选路径后交后端写文件
  const doExportM3u = async () => {
    if (!pl) return;
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const path = await save({
        title: tr("导出歌单为 M3U", "Export Playlist as M3U"),
        defaultPath: `${pl.name}.m3u8`,
        filters: [{ name: tr("M3U 歌单", "M3U Playlist"), extensions: ["m3u8", "m3u"] }],
      });
      if (path) await exportM3u(pl.id, path);
    } catch (e) {
      toast(String(e), "error");
    }
  };

  // m3u 导入：打开对话框选文件，以歌单名（默认文件名）新建列表
  const doImportM3u = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        title: tr("导入 M3U 歌单", "Import M3U Playlist"),
        multiple: false,
        filters: [{ name: tr("M3U 歌单", "M3U Playlist"), extensions: ["m3u8", "m3u"] }],
      });
      if (!picked || typeof picked !== "string") return;
      const fallbackName = tr("导入歌单", "Imported Playlist");
      const stem = picked.split(/[\\/]/).pop() ?? fallbackName;
      const name = stem.replace(/\.(m3u8?|M3U8?)$/, "") || fallbackName;
      await importM3u(name, picked);
    } catch (e) {
      toast(String(e), "error");
    }
  };

  const pl = playlists.find((p) => p.id === id);
  const unavailable = useStore((s) => s.unavailable);
  const [renameOpen, setRenameOpen] = useState(false);

  useEffect(() => {
    setBatchMode(false);
    setSel(new Set());
  }, [id]);

  const list = useMemo<DetailRow[]>(() => {
    if (!pl) return [];
    const byId = new Map(tracks.map((t) => [t.id, t]));
    return pl.entries
      .filter((e) => matchSearchEntry(e, search))
      .map((e): DetailRow => {
        const base = {
          entry: e,
          name: e.title,
          artist: e.artist,
          album: e.album,
          cover: e.cover,
          durationMs: e.duration * 1000,
        };
        if (e.kind === "local") {
          const t = e.trackId != null ? byId.get(e.trackId) : undefined;
          return {
            ...base,
            kind: "track",
            id: e.trackId ?? 0,
            name: t ? t.title : e.title,
            artist: t ? t.artist : e.artist,
            album: t ? t.album : e.album,
            cover: t ? t.cover : e.cover,
            durationMs: t ? t.duration * 1000 : e.duration * 1000,
            liked: t?.liked ?? false,
          };
        }
        if (e.kind === "netease") {
          return {
            ...base,
            kind: "netease",
            id: Number(e.onlineId ?? 0),
            liked: !!savedOnline[`netease-${e.onlineId}`],
          };
        }
        if (e.kind === "kugou") {
          return {
            ...base,
            kind: "kugou",
            id: e.onlineId ?? "",
            liked: !!savedOnline[`kugou-${e.onlineId}`],
          };
        }
        if (e.kind === "bilibili") {
          return {
            ...base,
            kind: "bilibili",
            id: e.onlineId ?? "",
            liked: !!savedOnline[`bilibili-${e.onlineId}`],
          };
        }
        if (e.kind === "navidrome") {
          return {
            ...base,
            kind: "navidrome",
            id: e.onlineId ?? "",
            liked: !!savedOnline[`navidrome-${e.onlineId}`],
          };
        }
        return {
          ...base,
          kind: "qq",
          id: e.onlineId ?? "",
          liked: !!savedOnline[`qq-${e.onlineId}`],
        };
      });
  }, [pl, tracks, search, savedOnline]);

  const toggleSel = (rowid: number) => {
    setSel((prev) => {
      const next = new Set(prev);
      if (next.has(rowid)) next.delete(rowid);
      else next.add(rowid);
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (sel.size === list.length) {
      setSel(new Set());
    } else {
      setSel(new Set(list.map((r) => r.entry.rowid)));
    }
  };

  const batchAddToQueue = () => {
    const selectedRows = list.filter((r) => sel.has(r.entry.rowid));
    for (const r of selectedRows) {
      const item = entryToQueueItem(r.entry);
      if (item) addToQueue(item);
    }
    toast(
      tr(
        `已将 ${selectedRows.length} 首歌曲加入播放队列`,
        `Added ${selectedRows.length} ${selectedRows.length === 1 ? "track" : "tracks"} to queue`
      )
    );
  };

  const handleBatchRemove = async () => {
    const rowids = Array.from(sel);
    if (rowids.length > 0) {
      await removePlaylistEntryRows(rowids);
      setSel(new Set());
      setBatchMode(false);
    }
    setConfirmBatchRemove(false);
  };

  // 长按拖拽调序：提交时按 rowid 序列重写 position。
  // 搜索过滤时 list 只是可见子集——基于全量 entries 重排（被拖行插到
  // 目标行之后，其余行保持相对顺序），保证隐藏行不丢
  const { rowProps } = useDragList((from, to) => {
    if (!pl) return;
    const allEntries = pl.entries;
    if (!search) {
      const rows = list.map((r) => r.entry);
      const [moved] = rows.splice(from, 1);
      rows.splice(to, 0, moved);
      reorderPlaylist(pl.id, rows.map((r) => r.rowid));
      return;
    }
    const movedRow = list[from]?.entry;
    if (!movedRow || from === to) return;
    // 插入锚点按方向取：向下拖落点在 to，锚点是 to 的下一行（list[to+1]）；
    // 向上拖正确锚点是 list[to]（新序列中 to+1 位是原 list[to]）——
    // 此前统一用 list[to+1]，向上拖松手后歌曲落在目标下一行，反复拖越拖越乱
    const anchor = list[from < to ? to + 1 : to]?.entry ?? null;
    const rows = allEntries.filter((e) => e.rowid !== movedRow.rowid);
    const anchorIdx = anchor ? rows.findIndex((e) => e.rowid === anchor.rowid) : -1;
    const insertAt = anchorIdx >= 0 ? anchorIdx : rows.length;
    rows.splice(insertAt, 0, movedRow);
    reorderPlaylist(pl.id, rows.map((e) => e.rowid));
  });

  // 窗口化：超长列表（导入的在线歌单可达上千首）只渲染可视区行。
  // useDragList 依赖“行 = 滚动容器全部直接子元素”，窗口化时拖拽不可用——
  // 常规规模（≤阈值）保持原样（拖拽可用），超过阈值才切换
  const ROW_H = 60;
  const WINDOW_THRESHOLD = 500;
  const windowed = list.length > WINDOW_THRESHOLD;
  const win = useVirtualWindow(list.length, ROW_H);

  // 当前行判定（行 vs 播放中曲目）：本地曲目比 current.id；在线行按音源
  // 比对应 id 字段（qq/bilibili/navidrome 都落在 current.qid）。旧实现
  // 本地曲目落到 qid 分支（null === number）永不命中——歌单里播本地歌
  // 从不显示“正在播放”标识，这里一并修正
  const isCurrentRow = (r: DetailRow) =>
    current != null &&
    (r.kind === "track"
      ? current.kind === "track" && current.id === r.id
      : current.kind === r.kind &&
        (r.kind === "netease"
          ? current.nid === r.id
          : r.kind === "kugou"
            ? current.kgid === r.id
            : current.qid === r.id));
  const currentIdx = useMemo(() => list.findIndex(isCurrentRow), [list, current]);

  // 当前播放行滚出可视区时浮出「回到当前歌曲」药丸（打开定位之外的可回补路径）
  const pill = useLocatePill({
    containerRef: win.containerRef,
    rowHeight: ROW_H,
    currentIndex: currentIdx,
  });

  // 打开 / 切换歌单时定位到当前播放行（行高恒定，数学定位对窗口化与
  // 全量渲染都成立）；当前曲不在本歌单则回到顶部。
  // 布局/封面加载时序敏感：连续三帧重试确保滚动到位
  useLayoutEffect(() => {
    const doLocate = () => {
      const el = win.containerRef.current;
      if (!el) return;
      if (currentIdx >= 0) win.scrollToIndex(currentIdx);
      else el.scrollTop = 0;
    };
    doLocate();
    let raf = 0;
    let frame = 0;
    const tick = () => {
      doLocate();
      frame += 1;
      if (frame < 3) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);

  if (!pl) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--ink-2)]">
        {tr("播放列表不存在", "Playlist not found")}
      </div>
    );
  }

  const headerContent = (
    <>
      {pl.cover ? (
        <CoverImg
          src={pl.cover}
          seed={pl.name}
          className="w-[116px] h-[116px] rounded-2xl shadow-xl shrink-0"
          iconSize={36}
        />
      ) : (
        <div
          className="w-[116px] h-[116px] rounded-2xl shadow-xl flex items-center justify-center shrink-0"
          style={{
            background:
              "linear-gradient(135deg, var(--accent-soft), var(--shade-strong))",
          }}
        >
          <ListMusic size={36} className="text-[var(--accent)]" />
        </div>
      )}
      <div className="min-w-0 flex-1">
        <div className="text-[11px] font-semibold text-[var(--ink-2)] tracking-wider mb-1 flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[var(--accent)]" />
          {tr("播放列表", "Playlist")}
        </div>
        <h1 className="text-[24px] font-bold truncate leading-tight text-[var(--ink)]">{pl.name}</h1>
        {/* 改名后保留原始导入名，来源一目了然（重导入也按它兜底认出） */}
        {pl.originName && pl.originName !== pl.name && (
          <div className="text-[11.5px] text-[var(--ink-3)] mt-1 truncate">
            {tr(`导入名「${pl.originName}」`, `Imported as "${pl.originName}"`)}
            {pl.remoteKind && (
              <span className="ml-1">
                ·{" "}
                {tr(
                  `来源${pl.remoteKind === "netease" ? "网易云" : "QQ 音乐"}`,
                  `from ${pl.remoteKind === "netease" ? "NetEase" : "QQ Music"}`,
                )}
              </span>
            )}
          </div>
        )}
        <div className={`text-[12.5px] text-[var(--ink-2)] ${isSkeuo ? "mt-1" : "mt-1.5"}`}>
          {tr(
            `${list.length} 首曲目`,
            `${list.length} ${list.length === 1 ? "track" : "tracks"}`,
          )}
          {list.length > 0 && !windowed && (
            <span className="text-[var(--ink-3)] ml-2">
              · {tr("长按歌曲可拖动调序", "Long-press a track to drag and reorder")}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2 mt-3">
          <button className="btn-primary shrink-0 whitespace-nowrap" onClick={() => playEntries(list.map((r) => r.entry), 0)}>
            <Play size={13} className="fill-current" />
            {tr("播放全部", "Play All")}
          </button>
          <button
            className="btn-secondary shrink-0 whitespace-nowrap"
            onClick={() =>
              playEntries(
                list.map((r) => r.entry),
                Math.floor(Math.random() * Math.max(1, list.length))
              )
            }
          >
            <Shuffle size={13} />
            {tr("随机", "Shuffle")}
          </button>
          <button
            className={`btn-secondary shrink-0 whitespace-nowrap ${batchMode ? "!text-[var(--accent)] !border-[var(--accent)]" : ""}`}
            onClick={() => {
              setBatchMode((v) => !v);
              setSel(new Set());
            }}
            disabled={list.length === 0}
          >
            <ListChecks size={13} />
            {batchMode ? tr("退出批量", "Done") : tr("批量管理", "Batch Edit")}
          </button>

          {/* 更多管理操作下拉（重命名、导出/导入 M3U、删除） */}
          <div className="relative">
            <button
              className={`btn-secondary !w-8 !h-8 !p-0 flex items-center justify-center ${
                moreOpen ? "!text-[var(--accent)] !border-[var(--accent)]" : ""
              }`}
              onClick={() => setMoreOpen((v) => !v)}
              title={tr("更多歌单操作", "More playlist actions")}
            >
              <MoreHorizontal size={15} />
            </button>
            {moreOpen && (
              <>
                <div
                  className="fixed inset-0 z-40"
                  onClick={() => setMoreOpen(false)}
                />
                <div className="absolute left-0 top-full mt-1.5 z-50 min-w-[170px] w-max glass-strong rounded-xl p-1.5 shadow-2xl anim-menu border border-[var(--line)]">
                  <button
                    className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left whitespace-nowrap"
                    onClick={() => {
                      setMoreOpen(false);
                      setRenameOpen(true);
                    }}
                  >
                    <Pencil size={13} />
                    {tr("重命名", "Rename")}
                  </button>
                  <button
                    className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left disabled:opacity-40 whitespace-nowrap"
                    disabled={!pl?.entries.length}
                    onClick={() => {
                      setMoreOpen(false);
                      doExportM3u();
                    }}
                  >
                    <FileDown size={13} />
                    {tr("导出 M3U", "Export M3U")}
                  </button>
                  <button
                    className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left whitespace-nowrap"
                    onClick={() => {
                      setMoreOpen(false);
                      doImportM3u();
                    }}
                  >
                    <FileUp size={13} />
                    {tr("导入 M3U", "Import M3U")}
                  </button>
                  <div className="my-1 border-t border-[var(--line)]" />
                  <button
                    className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2 text-[12.5px] !text-[#e8564a] hover:!bg-[rgba(232,86,74,0.12)] text-left whitespace-nowrap"
                    onClick={() => {
                      setMoreOpen(false);
                      setConfirmDel(true);
                    }}
                  >
                    <Trash2 size={13} />
                    {tr("删除歌单", "Delete Playlist")}
                  </button>
                </div>
              </>
            )}
          </div>

          {/* 本页搜索框：靠右对齐，与左侧主操作保持同一行 */}
          <div className="relative ml-auto w-[240px] max-w-full">
            <Search
              size={13}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ink-3)]"
            />
            <input
              ref={plSearchRef}
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder={tr("在歌单内搜索… (Ctrl+F)", "Search in playlist… (Ctrl+F)")}
              className={
                isLiquid
                  ? "w-full h-8 rounded-xl liquid-input pl-8 pr-7 text-[12px] text-[var(--ink)] placeholder:text-[var(--ink-3)] outline-none focus:border-[var(--accent)]"
                  : "w-full h-8 rounded-lg bg-[var(--shade)] border border-[var(--line)] pl-8 pr-7 text-[12px] text-[var(--ink)] placeholder:text-[var(--ink-3)] outline-none focus:border-[var(--accent)]"
              }
            />
            {search && (
              <button
                className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--ink-3)] hover:text-[var(--ink)]"
                onClick={() => setSearch("")}
                title={tr("清除筛选", "Clear filter")}
              >
                <X size={12} />
              </button>
            )}
          </div>
        </div>
      </div>
    </>
  );

  const batchBanner = batchMode ? (
    <div className="mb-2 px-4 py-2 glass rounded-2xl flex items-center justify-between gap-3 text-[12.5px] anim-fade-in shrink-0">
      <div className="flex items-center gap-2.5">
        <span className="text-[var(--ink-2)] font-medium">
          {tr("已选 ", "Selected ")}
          <span className="text-[var(--accent-strong)] font-semibold">{sel.size}</span> / {list.length}{" "}
          {tr("首", list.length === 1 ? "track" : "tracks")}
        </span>
        <button
          className="btn-secondary !py-1 !px-2.5 !text-[12px]"
          onClick={() => setSel(new Set(list.map((r) => r.entry.rowid)))}
          title={tr("全选当前歌单歌曲（Ctrl+A）", "Select all tracks in playlist (Ctrl+A)")}
        >
          {tr("全选", "Select All")}
        </button>
        <button
          className="btn-secondary !py-1 !px-2.5 !text-[12px]"
          onClick={() => setSel(new Set())}
          disabled={sel.size === 0}
        >
          {tr("清除", "Clear")}
        </button>
        <button
          className="btn-danger !py-1 !px-2.5 !text-[12px]"
          onClick={() => setConfirmBatchRemove(true)}
          disabled={sel.size === 0}
          title={tr("从当前歌单移除勾选歌曲", "Remove selected tracks from playlist")}
        >
          {tr("移除", "Remove")}
        </button>
      </div>
      <button
        className="btn-ghost w-7 h-7 rounded-lg text-[var(--ink-3)]"
        onClick={() => {
          setBatchMode(false);
          setSel(new Set());
        }}
        title={tr("退出批量管理（ESC）", "Exit batch edit (ESC)")}
      >
        <X size={14} />
      </button>
    </div>
  ) : null;

  return (
    <>
      <ViewShell
        uiTheme={uiTheme}
        header={headerContent}
        cardHeader={batchBanner}
        headerClass="pt-5 pb-4 px-5 flex gap-5 items-center shrink-0"
        liquidHeaderClass="pt-5 pb-4 px-5 flex gap-5 items-center shrink-0 border-b border-[var(--line)]"
        bottomPadding={isSkeuo ? "pb-[96px]" : "pb-[86px]"}
      >
        {list.length === 0 ? (
            <div className="flex-1 flex items-center justify-center text-[13px] text-[var(--ink-2)]">
              {search
                ? tr("没有匹配筛选的歌曲（点右侧 ✕ 清除筛选）", "No matching tracks (click ✕ on the right to clear)")
                : tr("列表里还没有歌曲（可在在线曲库右键添加）", "No tracks yet (right-click any online track to add)")}
            </div>
          ) : (
            <>
              {/* 表头：清晰区分序号、歌曲、专辑、时长与操作 */}
              <div className="grid grid-cols-[52px_minmax(200px,3fr)_minmax(130px,2fr)_104px_116px] items-center gap-4 h-9 px-6 border-b border-[var(--line)] text-[11px] font-semibold text-[var(--ink-3)] tracking-wider select-none shrink-0">
                <span className="text-center">#</span>
                <span className="pl-1">{tr("歌曲", "TITLE")}</span>
                <span>{tr("专辑", "ALBUM")}</span>
                <span className="text-right">{tr("时长", "DURATION")}</span>
                <span className="text-right pr-2">{tr("操作", "ACTIONS")}</span>
              </div>
              <div
                ref={win.containerRef}
                onScroll={win.onScroll}
                className="flex-1 min-h-0 overflow-y-auto px-2 pt-1.5 pb-4"
              >
                {/* 虚拟窗口占位（data-drag-skip：不参与长按拖拽的行枚举） */}
                <div
                  style={{ height: (windowed ? win.start : 0) * ROW_H }}
                  aria-hidden
                  data-drag-skip
                />
                {(windowed ? list.slice(win.start, win.end) : list).map((r, k) => {
                  const i = windowed ? win.start + k : k;
                  const active = isCurrentRow(r);
                  const isSelected = batchMode && sel.has(r.entry.rowid);
                  // 播放失败（无版权/下架等）：整行置灰 + 无版权标记
                  const dead =
                    r.kind !== "track" &&
                    unavailable[`${r.kind}:${r.id}`] != null;
                  return (
                    <div
                      key={r.entry.rowid}
                      {...(windowed || batchMode ? {} : rowProps(i))}
                      style={{ ["--row-idx" as string]: Math.min(i, 12) }}
                      className={
                        isSkeuo
                          ? `${!windowed || i < 24 ? "anim-row" : ""} skeuo-row group grid grid-cols-[52px_minmax(200px,3fr)_minmax(130px,2fr)_104px_116px] items-center gap-4 h-[60px] px-4 rounded-[12px] cursor-default ${
                              isSelected ? "checked" : active ? "active" : ""
                            } ${dead ? "opacity-45" : ""}`
                          : isLiquid
                            ? `${!windowed || i < 24 ? "anim-row" : ""} group grid grid-cols-[52px_minmax(200px,3fr)_minmax(130px,2fr)_104px_116px] items-center gap-4 h-[60px] px-4 rounded-2xl transition-all duration-200 cursor-default ${
                                isSelected
                                  ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] border border-[var(--accent-soft)] shadow-sm"
                                  : active
                                    ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] border border-[var(--accent-soft)] shadow-sm font-medium"
                                    : "hover:bg-[var(--shade-hover)] border border-transparent"
                              } ${dead ? "opacity-45" : ""}`
                            : `${!windowed || i < 24 ? "anim-row" : ""} group grid grid-cols-[52px_minmax(200px,3fr)_minmax(130px,2fr)_104px_116px] items-center gap-4 h-[60px] px-4 rounded-[13px] transition-colors cursor-default ${
                                isSelected
                                  ? "bg-[var(--accent-weak)] text-[var(--accent-strong)]"
                                  : active
                                    ? "bg-[var(--accent-weak)]"
                                    : "hover:bg-[var(--shade-hover)]"
                              } ${dead ? "opacity-45" : ""}`
                      }
                    title={dead ? tr(`无法播放：${unavailable[`${r.kind}:${r.id}`]}`, `Unavailable: ${unavailable[`${r.kind}:${r.id}`]}`) : undefined}
                    onClick={() => {
                      if (batchMode) toggleSel(r.entry.rowid);
                    }}
                    onDoubleClick={() => {
                      if (!batchMode) playEntries(list.map((x) => x.entry), i);
                    }}
                    onContextMenu={(ev) => {
                      if (batchMode) return;
                      ev.preventDefault();
                      setMenu({ x: ev.clientX, y: ev.clientY, e: r.entry });
                    }}
                  >
                    <div className="relative h-11 flex items-center justify-center">
                      {batchMode ? (
                        <button
                          type="button"
                          className={`w-[18px] h-[18px] rounded-[5px] border flex items-center justify-center shrink-0 transition-colors ${
                            isSelected
                              ? "bg-[var(--accent)] border-[var(--accent)] text-[var(--accent-on)]"
                              : "border-[var(--line)] hover:border-[var(--accent)]"
                          }`}
                          onClick={(e) => {
                            e.stopPropagation();
                            toggleSel(r.entry.rowid);
                          }}
                        >
                          {isSelected && <Check size={12} strokeWidth={3} />}
                        </button>
                      ) : (
                        <>
                          <span
                            className={`text-[12.5px] tabular-nums transition-opacity group-hover:opacity-0 ${
                              active ? "text-[var(--accent)] font-bold" : "text-[var(--ink-3)]"
                            }`}
                          >
                            {String(i + 1).padStart(2, "0")}
                          </span>
                          <button
                            className={`absolute inset-0 m-auto w-9 h-9 rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-all hover:scale-105 ${
                              active
                                ? "bg-[var(--accent)] text-[var(--accent-on)] shadow-[0_2px_10px_rgba(0,0,0,0.2)]"
                                : "bg-[var(--accent)] text-[var(--accent-on)] shadow-[0_2px_10px_rgba(0,0,0,0.18)]"
                            }`}
                            onClick={(ev) => {
                              ev.stopPropagation();
                              if (active) {
                                useStore.getState().togglePlay();
                              } else {
                                playEntries(list.map((x) => x.entry), i);
                              }
                            }}
                            title={active ? (playing ? tr("暂停", "Pause") : tr("播放", "Play")) : tr("播放", "Play")}
                          >
                            {active && playing ? (
                              <Pause size={14} className="fill-current" />
                            ) : (
                              <Play size={14} className="fill-current ml-px" />
                            )}
                          </button>
                        </>
                      )}
                    </div>

                    <div className="flex items-center gap-4 min-w-0">
                      <CoverImg
                        src={r.cover}
                        seed={r.name}
                        small
                        className="hover-lift w-11 h-11 rounded-xl shadow-[var(--cover-shadow-sm)] shrink-0"
                        iconSize={16}
                      />
                        <div className="min-w-0">
                        <div
                          className={`text-[13.5px] truncate flex items-center gap-2 ${
                            active
                              ? "text-[var(--accent-strong)] font-semibold"
                              : "text-[var(--ink)]"
                          }`}
                        >
                          <span className="truncate">{r.name}</span>
                          {r.kind !== "track" && (
                            <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-[var(--shade-strong)] text-[var(--ink-3)] font-medium shrink-0 flex items-center gap-1">
                              <Cloud size={9} />
                              {r.kind === "netease"
                                ? tr("网易云", "NetEase")
                                : r.kind === "kugou"
                                  ? tr("酷狗", "Kugou")
                                  : r.kind === "bilibili"
                                    ? tr("B站", "Bilibili")
                                    : r.kind === "navidrome"
                                      ? "Navidrome"
                                      : tr("QQ音乐", "QQ Music")}
                            </span>
                          )}
                          {r.kind !== "track" && r.entry.vip && !dead && (
                            <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-[var(--accent-weak)] text-[var(--accent-strong)] font-bold shrink-0">
                              VIP
                            </span>
                          )}
                          {dead && (
                            <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-[var(--shade-strong)] text-[var(--ink-3)] font-medium shrink-0 flex items-center gap-1" title={unavailable[`${r.kind}:${r.id}`]}>
                              <Ban size={9} />
                              {tr("无版权", "Unavailable")}
                            </span>
                          )}
                        </div>
                        <button
                          className="block text-left text-[12px] text-[var(--ink-3)] truncate mt-1 max-w-full hover:text-[var(--accent-strong)] transition-colors"
                          onClick={(ev) => {
                            if (batchMode) {
                              ev.stopPropagation();
                              toggleSel(r.entry.rowid);
                              return;
                            }
                            ev.stopPropagation();
                            openDetailPage("artist", r.artist || tr("未知艺术家", "Unknown Artist"), r.kind === "netease" || r.kind === "qq" || r.kind === "kugou" ? r.kind : undefined);
                          }}
                          title={
                            batchMode
                              ? undefined
                              : tr(
                                  `查看歌手：${r.artist || "未知艺术家"}`,
                                  `View artist: ${r.artist || "Unknown Artist"}`,
                                )
                          }
                        >
                          {r.artist || tr("未知艺术家", "Unknown Artist")}
                        </button>
                      </div>
                    </div>

                    <button
                      className="block text-left text-[12.5px] text-[var(--ink-3)] truncate max-w-full hover:text-[var(--accent-strong)] transition-colors"
                      onClick={(ev) => {
                        if (batchMode) {
                          ev.stopPropagation();
                          toggleSel(r.entry.rowid);
                          return;
                        }
                        ev.stopPropagation();
                        openDetailPage("album", r.album || tr("未知专辑", "Unknown Album"), r.kind === "netease" || r.kind === "qq" || r.kind === "kugou" ? r.kind : undefined);
                      }}
                      title={
                        batchMode
                          ? undefined
                          : tr(
                              `查看专辑：${r.album || "未知专辑"}`,
                              `View album: ${r.album || "Unknown Album"}`,
                            )
                      }
                    >
                      {r.album || tr("未知专辑", "Unknown Album")}
                    </button>

                    <div className="text-right text-[12.5px] text-[var(--ink-2)] tabular-nums">
                      {fmtTime(r.durationMs)}
                    </div>

                    <div className={`flex items-center justify-end gap-1 pr-1 ${batchMode ? "opacity-0 pointer-events-none" : ""}`}>
                      <button
                        className="btn-ghost w-8 h-8"
                        onClick={(e) => {
                          e.stopPropagation();
                          if (r.kind === "track") toggleLike(r.id);
                          else
                            toggleLikeOnline({
                              kind: r.kind,
                              id: r.id,
                              name: r.name,
                              artist: r.artist,
                              album: r.album,
                              cover: r.cover,
                              durationMs: r.durationMs,
                              mediaMid: r.entry.mediaMid,
                              vip: r.entry.vip,
                            });
                        }}
                        title={
                          r.kind === "track"
                            ? r.liked
                              ? tr("取消喜欢", "Remove Favorite")
                              : tr("喜欢", "Favorite")
                            : r.liked
                              ? tr("取消喜欢", "Remove Favorite")
                              : tr("收藏到“我喜欢”", "Add to Favorites")
                        }
                      >
                        <Heart
                          size={15}
                          className={
                            r.liked
                              ? "fill-[#e0533f] text-[#e0533f]"
                              : isSkeuo
                                ? "text-[var(--ink-3)] group-hover:text-[var(--ink-2)] transition-colors"
                                : "opacity-0 group-hover:opacity-100"
                          }
                        />
                      </button>
                      <button
                        className="btn-ghost w-8 h-8"
                        onClick={(e) => {
                          e.stopPropagation();
                          const item = entryToQueueItem(r.entry);
                          if (item) playNext(item);
                        }}
                        title={tr("下一首播放", "Play Next")}
                      >
                        <Play
                          size={14}
                          className={
                            isSkeuo
                              ? "text-[var(--ink-3)] group-hover:text-[var(--ink-2)] transition-colors"
                              : "opacity-0 group-hover:opacity-100"
                          }
                        />
                      </button>
                      <button
                        className="btn-ghost w-8 h-8"
                        onClick={(ev) => {
                          const rect = (
                            ev.currentTarget as HTMLElement
                          ).getBoundingClientRect();
                          setMenu({
                            x: rect.right - 215,
                            y: rect.bottom + 4,
                            e: r.entry,
                          });
                        }}
                        title={isSkeuo ? tr("更多选项", "More Options") : undefined}
                      >
                        <MoreHorizontal
                          size={16}
                          className={
                            isSkeuo
                              ? "text-[var(--ink-3)] group-hover:text-[var(--ink-2)] transition-colors"
                              : "opacity-0 group-hover:opacity-100"
                          }
                        />
                      </button>
                    </div>
                  </div>
                );
              })}
              <div
                style={{
                  height: Math.max(0, (windowed ? list.length - win.end : 0)) * ROW_H,
                }}
                aria-hidden
                data-drag-skip
              />
            </div>
            </>
          )}
        <LocateCurrentPill
          show={pill.show}
          onClick={pill.locate}
          className="absolute bottom-[72px] left-1/2 -translate-x-1/2"
        />
      </ViewShell>

      <ConfirmModal
        open={confirmDel}
        title={tr("删除播放列表", "Delete Playlist")}
        danger
        confirmText={tr("删除", "Delete")}
        onClose={() => setConfirmDel(false)}
        onConfirm={() => deletePlaylist(pl.id)}
      >
        {tr(
          `确定删除播放列表「${pl.name}」？列表中的曲目不会被删除。`,
          `Delete playlist "${pl.name}"? Tracks inside will not be deleted.`,
        )}
      </ConfirmModal>

      <ConfirmModal
        open={confirmBatchRemove}
        title={tr("批量从歌单移除", "Remove Tracks from Playlist")}
        danger
        confirmText={tr(`确认移除 (${sel.size} 首)`, `Remove (${sel.size})`)}
        onClose={() => setConfirmBatchRemove(false)}
        onConfirm={handleBatchRemove}
      >
        <p className="text-[13px] text-[var(--ink)] font-medium mb-1.5">
          {tr(
            `确定从歌单「${pl.name}」中移除选中的 ${sel.size} 首歌曲？`,
            `Remove ${sel.size} selected ${sel.size === 1 ? "track" : "tracks"} from "${pl.name}"?`,
          )}
        </p>
        <p className="text-[12px] text-[var(--ink-3)]">
          {tr(
            "注意：仅从当前歌单中移除该歌曲记录，不会删除硬盘上的本地音乐文件。",
            "Note: This only removes entries from the playlist; local files on disk will not be deleted.",
          )}
        </p>
      </ConfirmModal>

      <InputModal
        open={renameOpen}
        title={tr("重命名播放列表", "Rename Playlist")}
        initialValue={pl.name}
        confirmText={tr("重命名", "Rename")}
        onClose={() => setRenameOpen(false)}
        onConfirm={(name) => void renamePlaylist(pl.id, name)}
      />

      {/* 条目菜单（Portal 到 body：脱离 .glass 卡片，fixed 才相对视口） */}
      {menu &&
        createPortal(
        <div
          className={`fixed z-[75] w-[215px] glass-strong ${isLiquid ? "!rounded-2xl" : "rounded-xl"} p-1.5 shadow-2xl anim-menu`}
          style={(() => {
            const p = clampMenuPos(menu.x, menu.y, 215, 240);
            return { left: p.x, top: p.y };
          })()}
          onMouseDown={(ev) => ev.stopPropagation()}
          onMouseLeave={() => setMenu(null)}
        >
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              const idx = list.findIndex((r) => r.entry.rowid === menu.e.rowid);
              playEntries(
                list.map((r) => r.entry),
                Math.max(0, idx)
              );
              setMenu(null);
            }}
          >
            <Play size={13} /> {tr("播放", "Play")}
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              const r = list.find((x) => x.entry.rowid === menu.e.rowid);
              if (r) {
                const item = entryToQueueItem(r.entry);
                if (item) playNext(item);
              }
              setMenu(null);
            }}
          >
            <Play size={13} /> {tr("下一首播放", "Play Next")}
          </button>
          {menu.e.kind !== "local" && (
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                const r = list.find((x) => x.entry.rowid === menu.e.rowid);
                if (r)
                  downloadOnline({
                    kind: r.kind,
                    id: r.id,
                    name: r.name,
                    artist: r.artist,
                    album: r.album,
                    cover: r.cover,
                    durationMs: r.durationMs,
                    mediaMid: r.entry.mediaMid,
                  });
                setMenu(null);
              }}
            >
              <Play size={13} /> {tr("下载到本地", "Download to Library")}
            </button>
          )}
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              // rowid 已唯一定位该条目；不再按 trackId 二次删除
              //（同曲目在列表出现多次时会误删另一条目）
              removePlaylistEntryRow(menu.e.rowid);
              setMenu(null);
            }}
          >
            <Trash2 size={13} /> {tr("从列表移除", "Remove from Playlist")}
          </button>
        </div>,
        document.body
      )}
    </>
  );
}

function matchSearchEntry(
  e: PlaylistEntryMeta,
  q: string
): boolean {
  if (!q) return true;
  const s = q.toLowerCase();
  return (
    e.title.toLowerCase().includes(s) ||
    e.artist.toLowerCase().includes(s) ||
    e.album.toLowerCase().includes(s)
  );
}
