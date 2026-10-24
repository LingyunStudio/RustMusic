import {
  Check,
  Clock3,
  Cloud,
  Disc3,
  Heart,
  Library,
  ListChecks,
  ListMusic,
  Loader2,
  Music2,
  Palette,
  Pencil,
  Plus,
  Radio,
  Settings,
  DiscAlbum,
  Trash2,
  X,
} from "lucide-react";
import { useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { useStore } from "../store";
import Logo from "./Logo";
import SkinPicker from "./SkinPicker";
import { ConfirmModal, InputModal } from "./Dialogs";
import CoverImg from "./CoverImg";
import { clampMenuPos } from "../utils";
import { useDragList } from "../hooks/useDragList";
import type { ViewName } from "../types";

const NAV: { key: ViewName; label: string; icon: typeof Library }[] = [
  { key: "library", label: "资料库", icon: Library },
  { key: "liked", label: "我喜欢", icon: Heart },
  { key: "recent", label: "最近播放", icon: Clock3 },
  { key: "netease", label: "网易云", icon: Cloud },
  { key: "qq", label: "QQ音乐", icon: DiscAlbum },
  { key: "kugou", label: "酷狗", icon: Music2 },
  { key: "sources", label: "在线音源", icon: Radio },
];

export default function Sidebar() {
  const view = useStore((s) => s.view);
  const viewParam = useStore((s) => s.viewParam);
  const setView = useStore((s) => s.setView);
  const playlists = useStore((s) => s.playlists);
  const scan = useStore((s) => s.scan);
  const tracks = useStore((s) => s.tracks);
  const likedOnline = useStore((s) => s.likedOnline);
  const reorderPlaylists = useStore((s) => s.reorderPlaylists);
  const renamePlaylist = useStore((s) => s.renamePlaylist);
  const deletePlaylist = useStore((s) => s.deletePlaylist);
  const deletePlaylists = useStore((s) => s.deletePlaylists);
  const [plModalOpen, setPlModalOpen] = useState(false);
  // 歌单批量选择模式与状态
  const [batchMode, setBatchMode] = useState(false);
  const [batchSel, setBatchSel] = useState<Set<number>>(new Set());
  const [confirmBatchDel, setConfirmBatchDel] = useState(false);
  // 播放列表右键菜单 / 重命名弹窗
  const [menu, setMenu] = useState<{ x: number; y: number; id: number } | null>(
    null
  );
  const [renaming, setRenaming] = useState<{ id: number; name: string } | null>(
    null
  );
  // 右键删除二次确认：第一次点"删除"只置位，再点"确认删除"才真删
  const [confirmDel, setConfirmDel] = useState<number | null>(null);
  const [skinOpen, setSkinOpen] = useState(false);
  // 长按拖动排序：行序列 = 本地 state 的播放列表顺序
  const { rowProps } = useDragList((from, to) => {
    const ids = playlists.map((p) => p.id);
    const [moved] = ids.splice(from, 1);
    ids.splice(to, 0, moved);
    void reorderPlaylists(ids);
  });

  // 计数 = 本地喜欢 + 在线喜欢（与“我喜欢”视图显示的内容一致）。
  // useMemo：此前每次渲染都全库扫两遍，扫描期间每条进度更新都触发重算
  const likedCount = useMemo(
    () => tracks.filter((t) => t.liked).length + likedOnline.length,
    [tracks, likedOnline]
  );
  const totalDuration = useMemo(
    () => tracks.reduce((acc, t) => acc + t.duration, 0),
    [tracks]
  );
  const hours = Math.floor(totalDuration / 3600);
  const mins = Math.floor((totalDuration % 3600) / 60);

  return (
    <aside className="w-[236px] shrink-0 flex flex-col px-4 pt-3 relative z-20">
      {/* 品牌区（与桌面图标同构的黑胶 logo） */}
      <div className="flex items-center gap-3.5 px-3 pt-2 pb-6">
        <Logo size={44} />
        <div className="min-w-0">
          <div className="text-[15px] font-bold tracking-wide leading-tight text-[var(--ink)]">
            RustMusic
          </div>
          <div className="text-[10px] text-[var(--ink-3)] tracking-[0.22em] mt-0.5">
            HI-FI PLAYER
          </div>
        </div>
      </div>

      {/* 导航 + 播放列表（可滚动区） */}
      <div className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden pb-3">
        <nav className="flex flex-col gap-1.5">
          {NAV.map(({ key, label, icon: Icon }) => {
            const active = view === key;
            return (
              <button
                key={key}
                onClick={() => setView(key)}
                className={`nav-item relative h-11 pl-4 pr-3 rounded-xl flex items-center gap-3.5 text-[13.5px] transition-all duration-200 ${
                  active
                    ? "text-[var(--ink)] font-semibold"
                    : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                }`}
                style={
                  active
                    ? {
                        background: "var(--accent-weak)",
                        // 与背景同色的半透明边框会在 1px 内叠加出更亮的描边，
                        // 这里保持透明边框（占位对齐）只用背景表达选中
                        border: "1px solid transparent",
                      }
                    : { border: "1px solid transparent" }
                }
              >
                {active && (
                  <span
                    className="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-full"
                    style={{ background: "var(--accent)" }}
                  />
                )}
                <Icon
                  size={17}
                  strokeWidth={1.9}
                  className={active ? "text-[var(--accent)]" : ""}
                />
                {label}
                {key === "liked" && likedCount > 0 && (
                  <span className="ml-auto text-[11.5px] text-[var(--ink-3)] tabular-nums">
                    {likedCount}
                  </span>
                )}
              </button>
            );
          })}
        </nav>

        {/* 播放列表 */}
        <div className="mt-7 mb-2 px-3 flex items-center justify-between">
          <span className="text-[10.5px] font-semibold text-[var(--ink-3)] tracking-[0.18em]">
            {batchMode ? `已选 ${batchSel.size} 个` : "播放列表"}
          </span>
          <div className="flex items-center gap-1">
            {batchMode ? (
              <>
                <button
                  className="btn-ghost text-[10.5px] px-1.5 h-6 rounded"
                  onClick={() => {
                    if (batchSel.size === playlists.length) {
                      setBatchSel(new Set());
                    } else {
                      setBatchSel(new Set(playlists.map((p) => p.id)));
                    }
                  }}
                  title={batchSel.size === playlists.length ? "取消全选" : "全选"}
                >
                  {batchSel.size === playlists.length ? "取消" : "全选"}
                </button>
                <button
                  className="btn-ghost w-6 h-6 rounded text-[#e0533f] hover:bg-[#e0533f]/10 disabled:opacity-30"
                  title="删除所选歌单"
                  disabled={batchSel.size === 0}
                  onClick={() => setConfirmBatchDel(true)}
                >
                  <Trash2 size={13} />
                </button>
                <button
                  className="btn-ghost w-6 h-6 rounded text-[var(--ink-3)]"
                  title="退出批量模式"
                  onClick={() => {
                    setBatchMode(false);
                    setBatchSel(new Set());
                  }}
                >
                  <X size={13} />
                </button>
              </>
            ) : (
              <>
                <button
                  className="btn-ghost w-7 h-7 rounded-lg text-[var(--ink-3)] hover:text-[var(--ink)] disabled:opacity-40"
                  title="批量删除歌单"
                  disabled={playlists.length === 0}
                  onClick={() => {
                    setBatchMode(true);
                    setBatchSel(new Set());
                  }}
                >
                  <ListChecks size={14} />
                </button>
                <button
                  className="btn-ghost w-7 h-7 rounded-lg"
                  title="新建播放列表"
                  onClick={() => setPlModalOpen(true)}
                >
                  <Plus size={15} />
                </button>
              </>
            )}
          </div>
        </div>

        <div className="flex flex-col gap-1">
          {playlists.map((p, i) => {
            const active = view === "playlist" && viewParam === p.id;
            const checked = batchMode && batchSel.has(p.id);
            const originTip =
              p.originName && p.originName !== p.name
                ? `原名：${p.originName}（长按可拖动排序）`
                : "长按可拖动排序";
            return (
              <div key={p.id} {...(batchMode ? {} : rowProps(i))}>
                {/* 整行原是 button：会被 useDragList 的“行内控件不触发拖拽”
                    保护拦掉，故行容器用 div，点击行为由内层承接 */}
                <div
                  role="button"
                  tabIndex={-1}
                  title={batchMode ? undefined : originTip}
                  onClick={() => {
                    if (batchMode) {
                      setBatchSel((prev) => {
                        const next = new Set(prev);
                        if (next.has(p.id)) next.delete(p.id);
                        else next.add(p.id);
                        return next;
                      });
                    } else {
                      setView("playlist", p.id);
                    }
                  }}
                  onContextMenu={(ev) => {
                    if (batchMode) return;
                    ev.preventDefault();
                    setMenu({ x: ev.clientX, y: ev.clientY, id: p.id });
                  }}
                  className={`nav-item h-10 pl-3 pr-3 rounded-xl flex items-center gap-2.5 text-[13px] transition-all cursor-pointer ${
                    checked
                      ? "bg-[var(--accent-weak)] text-[var(--accent-strong)]"
                      : active
                        ? "bg-[var(--shade-strong)] text-[var(--ink)]"
                        : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                  }`}
                >
                  {batchMode ? (
                    <span
                      className={`w-[17px] h-[17px] rounded-[5px] border flex items-center justify-center shrink-0 transition-colors ${
                        checked
                          ? "bg-[var(--accent)] border-[var(--accent)] text-[var(--accent-on)]"
                          : "border-[var(--line)]"
                      }`}
                    >
                      {checked && <Check size={12} strokeWidth={3} />}
                    </span>
                  ) : (
                    <ListMusic
                      size={15}
                      className={active ? "text-[var(--accent)]" : "text-[var(--ink-3)]"}
                    />
                  )}
                  <span className="truncate">{p.name}</span>
                  <span className="ml-auto text-[11.5px] text-[var(--ink-3)] tabular-nums">
                    {p.entries.length}
                  </span>
                </div>
              </div>
            );
          })}
          {!playlists.length && (
            <button
              onClick={() => setPlModalOpen(true)}
              className="mx-1 h-10 rounded-xl border border-dashed border-[rgba(243,233,216,0.14)] text-[12px] text-[var(--ink-3)] hover:text-[var(--ink-2)] hover:border-[rgba(243,233,216,0.26)] transition-colors"
            >
              + 创建播放列表
            </button>
          )}
        </div>
      </div>

      {/* 底部固定区：设置按钮中心与播放条垂直中心对齐（播放条中心距底 36px） */}
      <div className="shrink-0 flex flex-col justify-end pb-[10px] gap-2">
        {scan.active && (
          <div className="flex items-center gap-2 px-4 text-[12px] text-[var(--accent)]">
            <Loader2 size={13} className="animate-spin" />
            <span className="truncate">
              扫描中 {scan.total ? `${scan.done}/${scan.total}` : "…"}
            </span>
          </div>
        )}

        {/* 设置 + 皮肤：同一行，皮肤在设置右侧 */}
        <div className="flex gap-2">
          <button
            onClick={() => setView("settings")}
            className={`nav-item flex-1 min-w-0 h-11 px-4 rounded-xl flex items-center gap-3.5 text-[13.5px] transition-all ${
              view === "settings"
                ? "bg-[var(--shade-strong)] text-[var(--ink)]"
                : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
            }`}
          >
            <Settings size={17} strokeWidth={1.9} />
            设置
          </button>
          <button
            onClick={() => setSkinOpen(true)}
            title="皮肤"
            className={`nav-item h-11 px-3.5 rounded-xl flex items-center gap-2 text-[13.5px] shrink-0 transition-all ${
              skinOpen
                ? "bg-[var(--shade-strong)] text-[var(--ink)]"
                : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
            }`}
          >
            <Palette size={17} strokeWidth={1.9} />
            皮肤
          </button>
        </div>
      </div>

      <SkinPicker open={skinOpen} onClose={() => setSkinOpen(false)} />

      <InputModal
        open={plModalOpen}
        title="新建播放列表"
        placeholder="播放列表名称"
        confirmText="创建"
        onClose={() => setPlModalOpen(false)}
        onConfirm={(name) => useStore.getState().createPlaylist(name)}
      />

      <InputModal
        open={renaming != null}
        title="重命名播放列表"
        initialValue={renaming?.name}
        confirmText="重命名"
        onClose={() => setRenaming(null)}
        onConfirm={(name) => {
          if (renaming) void renamePlaylist(renaming.id, name);
        }}
      />

      {/* 播放列表右键菜单（Portal 到 body：fixed 才相对视口） */}
      {menu &&
        createPortal(
          <div
            className="fixed z-[75] w-[150px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
            style={(() => {
              const p = clampMenuPos(menu.x, menu.y, 150, 110);
              return { left: p.x, top: p.y };
            })()}
            onMouseDown={(ev) => ev.stopPropagation()}
            onMouseLeave={() => {
              setMenu(null);
              setConfirmDel(null);
            }}
          >
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                const pl = playlists.find((p) => p.id === menu.id);
                if (pl) setRenaming({ id: pl.id, name: pl.name });
                setMenu(null);
              }}
            >
              <Pencil size={13} className="text-[var(--ink-3)]" />
              重命名
            </button>
            {confirmDel === menu.id ? (
              <button
                className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[#e0533f] hover:bg-[var(--shade-strong)] text-left"
                onClick={() => {
                  void deletePlaylist(menu.id);
                  setMenu(null);
                  setConfirmDel(null);
                }}
                title="再次确认删除（歌单内条目一并移除）"
              >
                <Trash2 size={13} />
                确认删除？
              </button>
            ) : (
              <button
                className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
                onClick={() => setConfirmDel(menu.id)}
              >
                <Trash2 size={13} className="text-[var(--ink-3)]" />
                删除
              </button>
            )}
          </div>,
          document.body
        )}

      <ConfirmModal
        open={confirmBatchDel}
        title="批量删除播放列表"
        danger
        confirmText={`确认删除 (${batchSel.size} 个)`}
        onClose={() => setConfirmBatchDel(false)}
        onConfirm={async () => {
          const ids = Array.from(batchSel);
          if (ids.length) {
            await deletePlaylists(ids);
            if (
              view === "playlist" &&
              viewParam != null &&
              batchSel.has(viewParam)
            ) {
              setView("library");
            }
            setBatchSel(new Set());
            setBatchMode(false);
          }
          setConfirmBatchDel(false);
        }}
      >
        <p className="text-[13px] text-[var(--ink)] font-medium mb-1.5">
          确定删除选中的 {batchSel.size} 个播放列表？
        </p>
        <p className="text-[12px] text-[var(--ink-3)]">
          注意：列表中的条目引用将被移除，但本地歌曲源文件不会受任何影响。
        </p>
      </ConfirmModal>
    </aside>
  );
}
