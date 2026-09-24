import { useMemo, useState } from "react";
import { createPortal } from "react-dom";
import {
  Ban,
  Heart,
  ListMusic,
  MoreHorizontal,
  Pencil,
  Play,
  Shuffle,
  Trash2,
  Cloud,
} from "lucide-react";
import { useStore } from "../store";
import { useDragList } from "../hooks/useDragList";
import { useVirtualWindow } from "../hooks/useVirtualWindow";
import { ConfirmModal, InputModal } from "../components/Dialogs";
import { clampMenuPos, fmtTime, matchSearch } from "../utils";
import CoverImg from "../components/CoverImg";
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
    };

export default function PlaylistDetail({ id }: { id: number }) {
  const playlists = useStore((s) => s.playlists);
  const tracks = useStore((s) => s.tracks);
  const search = useStore((s) => s.search);
  const openDetailPage = useStore((s) => s.openDetailPage);
  const playing = useStore((s) => s.playing);
  const playEntries = useStore((s) => s.playEntries);
  const entryToQueueItem = useStore((s) => s.entryToQueueItem);
  const removePlaylistEntryRow = useStore((s) => s.removePlaylistEntryRow);
  const toggleLike = useStore((s) => s.toggleLike);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const downloadOnline = useStore((s) => s.downloadOnline);
  const playNext = useStore((s) => s.playNext);
  const addToQueue = useStore((s) => s.addToQueue);
  const deletePlaylist = useStore((s) => s.deletePlaylist);
  const current = useStore((s) => s.current);
  const savedOnline = useStore((s) => s.savedOnline);
  const [confirmDel, setConfirmDel] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; e: PlaylistEntryMeta } | null>(
    null
  );
  const reorderPlaylist = useStore((s) => s.reorderPlaylist);
  const renamePlaylist = useStore((s) => s.renamePlaylist);

  const pl = playlists.find((p) => p.id === id);
  const unavailable = useStore((s) => s.unavailable);
  const [renameOpen, setRenameOpen] = useState(false);

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
        return {
          ...base,
          kind: "qq",
          id: e.onlineId ?? "",
          liked: !!savedOnline[`qq-${e.onlineId}`],
        };
      });
  }, [pl, tracks, search, savedOnline]);

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
    if (!movedRow) return;
    const anchor = list[to + 1]?.entry ?? null; // 插入锚点：目标位置下一行
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

  if (!pl) {
    return (
      <div className="flex-1 flex items-center justify-center text-[var(--ink-2)]">
        播放列表不存在
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      <header className="pt-5 pb-4 px-5 flex gap-5 items-end">
        {pl.cover ? (
          <CoverImg
            src={pl.cover}
            seed={pl.name}
            className="w-[104px] h-[104px] rounded-2xl shadow-xl shrink-0"
            iconSize={34}
          />
        ) : (
          <div
            className="w-[104px] h-[104px] rounded-2xl shadow-xl flex items-center justify-center shrink-0"
            style={{
              background:
                "linear-gradient(135deg, var(--accent-soft), var(--shade-strong))",
            }}
          >
            <ListMusic size={34} className="text-[var(--accent)]" />
          </div>
        )}
        <div className="min-w-0 flex-1">
          <div className="text-[11px] text-[var(--ink-2)] tracking-wider mb-1">播放列表</div>
          <h1 className="text-[24px] font-bold truncate">{pl.name}</h1>
          {/* 改名后保留原始导入名，来源一目了然（重导入也按它兜底认出） */}
          {pl.originName && pl.originName !== pl.name && (
            <div className="text-[11.5px] text-[var(--ink-3)] mt-1 truncate">
              导入名「{pl.originName}」
              {pl.remoteKind && (
                <span className="ml-1">
                  · 来源{pl.remoteKind === "netease" ? "网易云" : "QQ 音乐"}
                </span>
              )}
            </div>
          )}
          <div className="text-[12.5px] text-[var(--ink-2)] mt-1.5">
            {list.length} 首曲目
            {list.length > 0 && !windowed && (
              <span className="text-[var(--ink-3)] ml-2">· 长按歌曲可拖动调序</span>
            )}
          </div>
          <div className="flex items-center gap-2 mt-3 flex-wrap">
            <button className="btn-primary" onClick={() => playEntries(list.map((r) => r.entry), 0)}>
              <Play size={13} className="fill-current" />
              播放全部
            </button>
            <button
              className="btn-secondary"
              onClick={() =>
                playEntries(
                  list.map((r) => r.entry),
                  Math.floor(Math.random() * Math.max(1, list.length))
                )
              }
            >
              <Shuffle size={13} />
              随机
            </button>
            <button
              className="btn-secondary"
              onClick={() => setRenameOpen(true)}
            >
              <Pencil size={13} />
              重命名
            </button>
            <button
              className="btn-secondary !text-[#e8564a] hover:!bg-[rgba(232,86,74,0.12)]"
              onClick={() => setConfirmDel(true)}
            >
              <Trash2 size={13} />
              删除列表
            </button>
          </div>
        </div>
      </header>

      <ConfirmModal
        open={confirmDel}
        title="删除播放列表"
        danger
        confirmText="删除"
        onClose={() => setConfirmDel(false)}
        onConfirm={() => deletePlaylist(pl.id)}
      >
        确定删除播放列表「{pl.name}」？列表中的曲目不会被删除。
      </ConfirmModal>

      <InputModal
        open={renameOpen}
        title="重命名播放列表"
        initialValue={pl.name}
        confirmText="重命名"
        onClose={() => setRenameOpen(false)}
        onConfirm={(name) => void renamePlaylist(pl.id, name)}
      />

      {/* 底边界抬到播放条上方，留 4px 空隙（播放条总占位 64+16+4=84px） */}
      <div className="flex-1 min-h-0 flex flex-col px-6 pb-[86px]">
        <div className="glass rounded-3xl flex-1 min-h-0 flex flex-col overflow-hidden">
          {list.length === 0 ? (
            <div className="flex-1 flex items-center justify-center text-[13px] text-[var(--ink-2)]">
              {search
                ? "没有匹配筛选的歌曲（点右侧 ✕ 清除筛选）"
                : "列表里还没有歌曲（可在在线曲库右键添加）"}
            </div>
          ) : (
            <div
              ref={win.containerRef}
              onScroll={win.onScroll}
              className="flex-1 min-h-0 overflow-y-auto px-2.5 pt-2.5 pb-[62px]"
            >
              {/* 虚拟窗口占位（data-drag-skip：不参与长按拖拽的行枚举） */}
              <div
                style={{ height: (windowed ? win.start : 0) * ROW_H }}
                aria-hidden
                data-drag-skip
              />
              {(windowed ? list.slice(win.start, win.end) : list).map((r, k) => {
                const i = windowed ? win.start + k : k;
                const active =
                  current != null &&
                  current.kind === r.kind &&
                  (r.kind === "qq"
                    ? current.qid === r.id
                    : r.kind === "netease"
                      ? current.nid === r.id
                      : current.id === r.id);
                // 播放失败（无版权/下架等）：整行置灰 + 无版权标记
                const dead =
                  r.kind !== "track" &&
                  unavailable[`${r.kind}:${r.id}`] != null;
                return (
                  <div
                    key={r.entry.rowid}
                    {...(windowed ? {} : rowProps(i))}
                    style={{ ["--row-idx" as string]: Math.min(i, 12) }}
                    className={`${!windowed || i < 24 ? "anim-row" : ""} group grid grid-cols-[56px_minmax(200px,460px)_minmax(140px,300px)_92px_136px] items-center gap-4 h-[60px] px-4 rounded-[13px] transition-colors cursor-default ${
                      active ? "bg-[var(--accent-weak)]" : "hover:bg-[var(--shade-hover)]"
                    } ${dead ? "opacity-45" : ""}`}
                    title={dead ? `无法播放：${unavailable[`${r.kind}:${r.id}`]}` : undefined}
                    onDoubleClick={() => playEntries(list.map((x) => x.entry), i)}
                    onContextMenu={(ev) => {
                      ev.preventDefault();
                      setMenu({ x: ev.clientX, y: ev.clientY, e: r.entry });
                    }}
                  >
                    <div className="relative h-11 flex items-center justify-center">
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
                        onClick={() => playEntries(list.map((x) => x.entry), i)}
                      >
                        <Play size={14} className="fill-current ml-px" />
                      </button>
                    </div>

                    <div className="flex items-center gap-4 min-w-0">
                      <CoverImg
                        src={r.cover}
                        seed={r.name}
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
                                ? "网易云"
                                : r.kind === "kugou"
                                  ? "酷狗"
                                  : "QQ音乐"}
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
                              无版权
                            </span>
                          )}
                        </div>
                        <button
                          className="block text-left text-[12px] text-[var(--ink-3)] truncate mt-1 max-w-full hover:text-[var(--accent-strong)] transition-colors"
                          onClick={(ev) => {
                            ev.stopPropagation();
                            openDetailPage("artist", r.artist || "未知艺术家");
                          }}
                          title={`查看歌手：${r.artist || "未知艺术家"}`}
                        >
                          {r.artist || "未知艺术家"}
                        </button>
                      </div>
                    </div>

                    <button
                      className="block text-left text-[12.5px] text-[var(--ink-3)] truncate max-w-full hover:text-[var(--accent-strong)] transition-colors"
                      onClick={(ev) => {
                        ev.stopPropagation();
                        openDetailPage("album", r.album || "未知专辑");
                      }}
                      title={`查看专辑：${r.album || "未知专辑"}`}
                    >
                      {r.album || "未知专辑"}
                    </button>

                    <div className="text-right text-[12.5px] text-[var(--ink-2)] tabular-nums">
                      {fmtTime(r.durationMs)}
                    </div>

                    <div className="flex items-center justify-end gap-1 pr-1">
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
                              ? "取消喜欢"
                              : "喜欢"
                            : r.liked ? "取消喜欢" : "收藏到“我喜欢”"
                        }
                      >
                        <Heart
                          size={15}
                          className={
                            r.liked
                              ? "fill-[#e0533f] text-[#e0533f]"
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
                        title="下一首播放"
                      >
                        <Play
                          size={14}
                          className="opacity-0 group-hover:opacity-100"
                        />
                      </button>
                      <button
                        className="btn-ghost w-8 h-8"
                        onClick={(ev) => {
                          const rect = (
                            ev.currentTarget as HTMLElement
                          ).getBoundingClientRect();
                          setMenu({
                            x: rect.right - 200,
                            y: rect.bottom + 4,
                            e: r.entry,
                          });
                        }}
                      >
                        <MoreHorizontal
                          size={16}
                          className="opacity-0 group-hover:opacity-100"
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
          )}
        </div>
      </div>

      {/* 条目菜单（Portal 到 body：脱离 .glass 卡片，fixed 才相对视口） */}
      {menu &&
        createPortal(
        <div
          className="fixed z-[75] w-[200px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
          style={(() => {
            const p = clampMenuPos(menu.x, menu.y, 200, 240);
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
            <Play size={13} /> 播放
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
            <Play size={13} /> 下一首播放
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
              <Play size={13} /> 下载到本地
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
            <Trash2 size={13} /> 从列表移除
          </button>
        </div>,
        document.body
      )}
    </div>
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
