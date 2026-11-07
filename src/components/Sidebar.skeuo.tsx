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

  // 计数 = 本地喜欢 + 在线喜欢（与“我喜欢”视图显示的内容一致）
  const likedCount = useMemo(
    () => tracks.filter((t) => t.liked).length + likedOnline.length,
    [tracks, likedOnline]
  );

  return (
    <aside className="w-[236px] shrink-0 flex flex-col px-3.5 pt-3 relative z-20 border-r border-[var(--sk-divider)] shadow-[inset_-1px_0_0_var(--sk-divider-hi)]">
      {/* 拟物金属铭牌品牌区 */}
      <div className="skeuo-nameplate mt-2 mb-4 px-3.5 py-2.5 rounded-xl flex items-center gap-3">
        <Logo size={38} />
        <div className="min-w-0">
          <div className="text-[14.5px] font-black tracking-wide leading-tight text-[var(--ink)]">
            RustMusic
          </div>
          <div className="text-[9.5px] font-bold text-[var(--accent-strong)] tracking-[0.24em] mt-0.5">
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
                className={`nav-item relative h-10 pl-3.5 pr-3 rounded-xl flex items-center gap-2.5 text-[13px] transition-all duration-150 ${
                  active
                    ? "text-[var(--ink)] font-semibold"
                    : "text-[var(--ink-2)] hover:text-[var(--ink)]"
                }`}
                style={
                  active
                    ? {
                        background: "var(--sk-pressed-bg)",
                        border: "1px solid var(--sk-pressed-border)",
                        boxShadow: "var(--sk-pressed-shadow)",
                      }
                    : { border: "1px solid transparent" }
                }
              >
                <Icon
                  size={16}
                  strokeWidth={2}
                  className={active ? "text-[var(--accent-strong)]" : ""}
                />
                <span className="truncate">{label}</span>
                {key === "liked" && likedCount > 0 && (
                  <span className="ml-auto text-[11px] px-1.5 py-0.5 rounded-full bg-[var(--sk-groove-bg)] border border-[var(--sk-groove-border)] text-[var(--ink-2)] tabular-nums shadow-[var(--sk-groove-shadow)]">
                    {likedCount}
                  </span>
                )}
              </button>
            );
          })}
        </nav>

        {/* 播放列表 */}
        <div className="mt-6 mb-2 px-2 flex items-center justify-between">
          <span className="text-[10px] font-bold text-[var(--ink-3)] tracking-[0.2em] uppercase">
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
                  className="btn-ghost w-6 h-6 rounded-lg text-[var(--ink-3)] hover:text-[var(--ink)] disabled:opacity-40"
                  title="批量删除歌单"
                  disabled={playlists.length === 0}
                  onClick={() => {
                    setBatchMode(true);
                    setBatchSel(new Set());
                  }}
                >
                  <ListChecks size={13} />
                </button>
                <button
                  className="btn-ghost w-6 h-6 rounded-lg"
                  title="新建播放列表"
                  onClick={() => setPlModalOpen(true)}
                >
                  <Plus size={14} />
                </button>
              </>
            )}
          </div>
        </div>

        {/* 拟物托盘式播放列表容器 */}
        <div className="skeuo-well p-1.5 rounded-xl flex flex-col gap-1 min-h-[70px]">
          {playlists.map((p, i) => {
            const active = view === "playlist" && viewParam === p.id;
            const checked = batchMode && batchSel.has(p.id);
            const originTip =
              p.originName && p.originName !== p.name
                ? `原名：${p.originName}（长按可拖动排序）`
                : "长按可拖动排序";
            return (
              <div key={p.id} {...(batchMode ? {} : rowProps(i))}>
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
                  className={`nav-item h-9 pl-2.5 pr-2.5 rounded-lg flex items-center gap-2 text-[12.5px] transition-all cursor-pointer ${
                    checked
                      ? "bg-[var(--accent-weak)] text-[var(--accent-strong)]"
                      : active
                        ? "text-[var(--ink)] font-semibold"
                        : "text-[var(--ink-2)] hover:text-[var(--ink)]"
                  }`}
                  style={
                    !checked && active
                      ? {
                          background: "var(--sk-pressed-bg)",
                          border: "1px solid var(--sk-pressed-border)",
                          boxShadow: "var(--sk-pressed-shadow)",
                        }
                      : { border: "1px solid transparent" }
                  }
                >
                  {batchMode ? (
                    <span
                      className={`w-[15px] h-[15px] rounded border flex items-center justify-center shrink-0 transition-colors ${
                        checked
                          ? "bg-[var(--accent)] border-[var(--accent)] text-[var(--accent-on)]"
                          : "border-[var(--sk-groove-border)] bg-[var(--sk-groove-bg)]"
                      }`}
                    >
                      {checked && <Check size={11} strokeWidth={3} />}
                    </span>
                  ) : (
                    <ListMusic
                      size={14}
                      className={active ? "text-[var(--accent-strong)]" : "text-[var(--ink-3)]"}
                    />
                  )}
                  <span className="truncate">{p.name}</span>
                  <span className="ml-auto text-[11px] text-[var(--ink-3)] tabular-nums">
                    {p.entries.length}
                  </span>
                </div>
              </div>
            );
          })}
          {!playlists.length && (
            <button
              onClick={() => setPlModalOpen(true)}
              className="mx-1 h-9 rounded-lg border border-dashed border-[var(--sk-divider)] text-[11.5px] text-[var(--ink-3)] hover:text-[var(--ink-2)] hover:border-[var(--accent)] transition-colors flex items-center justify-center"
            >
              + 创建播放列表
            </button>
          )}
        </div>
      </div>

      {/* 底部固定区：拟物触感设置 & 皮肤按钮 */}
      <div className="shrink-0 flex flex-col justify-end pb-[10px] gap-2 pt-2 border-t border-[var(--sk-divider)] shadow-[0_-1px_0_var(--sk-divider-hi)]">
        {scan.active && (
          <div className="flex items-center gap-2 px-3 text-[11.5px] text-[var(--accent)]">
            <Loader2 size={12} className="animate-spin" />
            <span className="truncate">
              扫描中 {scan.total ? `${scan.done}/${scan.total}` : "…"}
            </span>
          </div>
        )}

        <div className="flex gap-2">
          <button
            onClick={() => setView("settings")}
            className={`btn-ghost flex-1 min-w-0 h-10 px-3 rounded-xl flex items-center justify-center gap-2.5 text-[13px] ${
              view === "settings" ? "!text-[var(--accent-strong)] font-semibold" : ""
            }`}
            style={
              view === "settings"
                ? {
                    background: "var(--sk-pressed-bg)",
                    borderColor: "var(--sk-pressed-border)",
                    boxShadow: "var(--sk-pressed-shadow)",
                  }
                : undefined
            }
          >
            <Settings size={16} strokeWidth={2} />
            设置
          </button>
          <button
            onClick={() => setSkinOpen(true)}
            title="皮肤"
            className={`btn-ghost h-10 px-3 rounded-xl flex items-center justify-center gap-1.5 text-[13px] shrink-0 ${
              skinOpen ? "!text-[var(--accent-strong)] font-semibold" : ""
            }`}
            style={
              skinOpen
                ? {
                    background: "var(--sk-pressed-bg)",
                    borderColor: "var(--sk-pressed-border)",
                    boxShadow: "var(--sk-pressed-shadow)",
                  }
                : undefined
            }
          >
            <Palette size={16} strokeWidth={2} />
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
