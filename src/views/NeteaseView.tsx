import { useEffect, useMemo, useRef, useState } from "react";
import {
  Cloud,
  Heart,
  LogOut,
  MoreHorizontal,
  Play,
  Search,
  ShieldCheck,
  Loader2,
} from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import type { NeteaseTrack, QqSong } from "../types";
import { fmtTime } from "../utils";
import Modal from "../components/Modal";

type Source = "netease" | "qq";

interface OnlineRow {
  kind: Source;
  id: number | string;
  name: string;
  artist: string;
  album: string;
  cover: string;
  durationMs: number;
  vip: boolean;
  mediaMid: string;
}

const qqCover = (albumMid: string) =>
  albumMid
    ? `https://y.gtimg.cn/music/photo_new/T002R300x300M000${albumMid}.jpg`
    : "";

export default function OnlineLibraryView({ source }: { source: Source }) {
  const neteaseResults = useStore((s) => s.neteaseResults);
  const neteaseSearching = useStore((s) => s.neteaseSearching);
  const neteaseSearched = useStore((s) => s.neteaseSearched);
  const neteaseTotal = useStore((s) => s.neteaseTotal);
  const qqResults = useStore((s) => s.qqResults);
  const qqSearching = useStore((s) => s.qqSearching);
  const qqSearched = useStore((s) => s.qqSearched);
  const current = useStore((s) => s.current);
  const playing = useStore((s) => s.playing);
  const savedOnline = useStore((s) => s.savedOnline);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const downloadOnline = useStore((s) => s.downloadOnline);
  const addOnlineToPlaylist = useStore((s) => s.addOnlineToPlaylist);
  const neteaseLiked = useStore((s) => s.neteaseLiked);
  const playNetease = useStore((s) => s.playNetease);
  const playQq = useStore((s) => s.playQq);
  const playNext = useStore((s) => s.playNext);
  const addToQueue = useStore((s) => s.addToQueue);
  const neteaseSearch = useStore((s) => s.neteaseSearch);
  const qqSearch = useStore((s) => s.qqSearch);
  const neteaseLoggedIn = useStore((s) => s.neteaseLoggedIn);
  const neteaseNickname = useStore((s) => s.neteaseNickname);
  const qqLoggedIn = useStore((s) => s.qqLoggedIn);
  const qqNickname = useStore((s) => s.qqNickname);
  const neteaseLogout = useStore((s) => s.neteaseLogout);
  const qqLogout = useStore((s) => s.qqLogout);
  const playlists = useStore((s) => s.playlists);
  const createPlaylist = useStore((s) => s.createPlaylist);
  const importNeteasePlaylist = useStore((s) => s.importNeteasePlaylist);
  const importQqPlaylist = useStore((s) => s.importQqPlaylist);
  const toast = useStore((s) => s.toast);

  const [kw, setKw] = useState("");
  const [menu, setMenu] = useState<{ x: number; y: number; row: OnlineRow } | null>(
    null
  );
  const [qrSource, setQrSource] = useState<Source | null>(null);
  const [pickerRow, setPickerRow] = useState<OnlineRow | null>(null);
  const [newPlName, setNewPlName] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [importList, setImportList] = useState<
    { id: number; name: string; trackCount: number }[] | null
  >(null);
  const [importing, setImporting] = useState(false);

  const searching = source === "netease" ? neteaseSearching : qqSearching;
  const searched = source === "netease" ? neteaseSearched : qqSearched;
  const loggedIn = source === "netease" ? neteaseLoggedIn : qqLoggedIn;
  const nickname = source === "netease" ? neteaseNickname : qqNickname;
  const sourceName = source === "netease" ? "网易云" : "QQ 音乐";

  const rows: OnlineRow[] = useMemo(() => {
    if (source === "netease") {
      return neteaseResults.map((t: NeteaseTrack) => ({
        kind: "netease" as const,
        id: t.id,
        name: t.name,
        artist: t.ar.map((a) => a.name).join(" / "),
        album: t.al?.name ?? "",
        cover: t.al?.picUrl ?? "",
        durationMs: t.dt,
        vip: t.fee === 1,
        mediaMid: "",
      }));
    }
    return qqResults.map((t: QqSong) => ({
      kind: "qq" as const,
      id: t.id,
      name: t.name,
      artist: t.singer,
      album: t.album,
      cover: qqCover(t.albumMid),
      durationMs: t.durationMs,
      vip: t.vip,
      mediaMid: t.mediaMid,
    }));
  }, [source, neteaseResults, qqResults]);

  const submit = () => (source === "netease" ? neteaseSearch(kw) : qqSearch(kw));

  const playRow = (i: number) => {
    if (source === "netease") playNetease(neteaseResults, i);
    else playQq(qqResults, i);
  };

  const menuAction = (row: OnlineRow, action: "play" | "next" | "queue") => {
    if (row.kind === "netease") {
      const idx = neteaseResults.findIndex((t) => t.id === row.id);
      if (action === "play") playNetease(neteaseResults, Math.max(0, idx));
      else if (action === "next")
        playNext({ kind: "netease", id: row.id as number });
      else addToQueue({ kind: "netease", id: row.id as number });
    } else {
      const idx = qqResults.findIndex((t) => t.id === row.id);
      if (action === "play") playQq(qqResults, Math.max(0, idx));
      else if (action === "next") playNext({ kind: "qq", id: row.id as string });
      else addToQueue({ kind: "qq", id: row.id as string });
    }
  };

  const hasMore =
    source === "netease"
      ? rows.length < neteaseTotal
      : rows.length > 0 && rows.length % 30 === 0;

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      {/* 头部：标题 | 搜索 | 登录（同一行区域） */}
      <header className="px-8 pt-7 pb-4">
        <div className="flex items-end justify-between gap-5">
          <div className="min-w-0 anim-rise shrink-0">
            <div className="flex items-center gap-2 text-[11px] text-[var(--ink-3)] tracking-[0.24em] mb-2">
              <Cloud size={13} />
              在线曲库
            </div>
            <h1 className="text-[30px] font-extrabold leading-none tracking-tight text-[var(--ink)]">
              搜一下，就能听
            </h1>
            <div className="flex items-center gap-3 mt-3 text-[12.5px] text-[var(--ink-2)]">
              <span className="tabular-nums">{rows.length} 条结果</span>
            </div>
          </div>

          {/* 搜索（与标题同行） */}
          <div className="flex items-center gap-2.5 flex-1 max-w-[520px] mb-1">
            <div className="relative flex-1">
              <Search
                size={14}
                className="absolute left-3.5 top-1/2 -translate-y-1/2 text-[var(--ink-3)]"
              />
              <input
                type="text"
                value={kw}
                onChange={(e) => setKw(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && submit()}
                placeholder={
                  source === "netease" ? "搜索网易云曲库…" : "搜索 QQ 音乐曲库…"
                }
                className="w-full h-10 rounded-xl bg-[var(--shade)] border border-[var(--line)] pl-9 pr-3 text-[12.5px] text-[var(--ink)] placeholder:text-[var(--ink-3)] focus:border-[rgba(240,162,74,0.45)] transition-colors"
              />
            </div>
            <button
              className="btn-primary h-10"
              onClick={submit}
              disabled={searching}
            >
              {searching ? (
                <Loader2 size={13} className="animate-spin" />
              ) : (
                <Search size={13} />
              )}
              搜索
            </button>
          </div>

          {/* 登录状态 */}
          <div className="shrink-0 flex flex-col items-end gap-2 mb-1">
            {loggedIn ? (
              <>
                <span
                  className="chip text-[var(--accent-strong)]"
                  style={{ background: "rgba(240,162,74,0.12)" }}
                  title={`已登录${sourceName}账号`}
                >
                  <ShieldCheck size={13} />
                  {nickname || "已登录"}
                </span>
                <div className="flex items-center gap-2">
                  <button
                    className="btn-secondary !py-1.5 !px-3"
                    onClick={async () => {
                      setImportList(null);
                      setImportOpen(true);
                      try {
                        const list =
                          source === "netease"
                            ? await api.neteaseUserPlaylists()
                            : await api.qqUserPlaylists();
                        setImportList(list);
                      } catch (e) {
                        setImportOpen(false);
                        toast(String(e), "error");
                      }
                    }}
                  >
                    导入歌单
                  </button>
                  <button
                    className="btn-secondary !py-1.5 !px-3"
                    onClick={() => (source === "netease" ? neteaseLogout() : qqLogout())}
                  >
                    退出
                  </button>
                </div>
              </>
            ) : (
              <button
                className="btn-secondary !py-1.5 !px-3"
                onClick={() => setQrSource(source)}
              >
                扫码登录
              </button>
            )}
          </div>
        </div>
      </header>

      {/* 结果列表 */}
      <div className="flex-1 min-h-0 flex flex-col px-6 pb-4">
        <div className="glass rounded-3xl flex-1 min-h-0 flex flex-col overflow-hidden">
          {rows.length > 0 && (
            <div className="grid grid-cols-[56px_minmax(200px,460px)_minmax(140px,300px)_92px_136px] items-center gap-4 h-10 px-5 border-b border-[var(--line)] text-[10.5px] text-[var(--ink-3)] tracking-[0.18em]">
              <span className="text-center">序号</span>
              <span>歌曲</span>
              <span>专辑</span>
              <span className="text-right">时长</span>
              <span className="text-right">操作</span>
            </div>
          )}
          <div className="flex-1 min-h-0 overflow-y-auto px-2.5 pt-2.5 pb-[84px]">
            {searching && (
              <div className="flex items-center justify-center gap-2.5 text-[var(--ink-3)] text-[13px] pt-16">
                <Loader2 size={15} className="animate-spin" />
                正在搜索…
              </div>
            )}
            {!searching && searched && rows.length === 0 && (
              <div className="text-center text-[var(--ink-3)] text-[13px] pt-16">
                没有找到相关歌曲
              </div>
            )}
            {!searched && !searching && (
              <div className="flex flex-col items-center justify-center gap-4 pt-24 anim-fade">
                <div className="relative">
                  <div
                    className="absolute -inset-8 rounded-full"
                    style={{
                      background:
                        "radial-gradient(circle, var(--accent) 0%, transparent 65%)",
                      opacity: 0.14,
                    }}
                  />
                  <div
                    className="relative w-[76px] h-[76px] rounded-3xl flex items-center justify-center"
                    style={{
                      background:
                        "linear-gradient(135deg, rgba(243,233,216,0.1), rgba(243,233,216,0.03))",
                      border: "1px solid rgba(243,233,216,0.12)",
                    }}
                  >
                    <Cloud size={30} className="text-[var(--ink-2)]" />
                  </div>
                </div>
                <div className="text-[13.5px] text-[var(--ink-2)]">
                  搜索在线曲库，双击即可播放
                </div>
                <div className="text-[11.5px] text-[var(--ink-3)] max-w-[440px] text-center leading-relaxed">
                  {sourceName}
                  免费曲目可直接播放；扫码登录自己的账号后按账号权益播放（含会员曲目）。请支持正版。
                </div>
              </div>
            )}

            {rows.map((t, i) => {
              const active =
                current?.kind === t.kind &&
                (t.kind === "qq" ? current.qid === t.id : current.id === t.id);
              const saved = !!savedOnline[`${t.kind}-${t.id}`];
              return (
                <div
                  key={`${t.kind}-${t.id}`}
                  className={`group grid grid-cols-[56px_minmax(200px,460px)_minmax(140px,300px)_92px_136px] items-center gap-4 h-[60px] px-4 rounded-2xl transition-colors cursor-default ${
                    active ? "bg-[rgba(240,162,74,0.1)]" : "hover:bg-[var(--shade-hover)]"
                  }`}
                  onDoubleClick={() => playRow(i)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setMenu({ x: e.clientX, y: e.clientY, row: t });
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
                      onClick={() => playRow(i)}
                      title="播放"
                    >
                      <Play size={14} className="fill-current ml-px" />
                    </button>
                  </div>

                  <div className="flex items-center gap-4 min-w-0">
                    {t.cover ? (
                      <img
                        src={t.cover}
                        alt=""
                        className="w-11 h-11 rounded-xl object-cover shadow-[0_4px_14px_rgba(0,0,0,0.45)] shrink-0"
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
                          <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-[rgba(240,162,74,0.16)] text-[var(--accent-strong)] font-bold shrink-0">
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
                      <div className="text-[12px] text-[var(--ink-3)] truncate mt-1">
                        {t.artist || "未知艺术家"}
                      </div>
                    </div>
                  </div>

                  <div className="text-[12.5px] text-[var(--ink-3)] truncate">
                    {t.album || "未知专辑"}
                  </div>

                  <div className="text-right text-[12.5px] text-[var(--ink-2)] tabular-nums">
                    {fmtTime(t.durationMs)}
                  </div>

                  <div className="flex items-center justify-end gap-1 pr-1">
                    <button
                      className="btn-ghost w-8 h-8"
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleLikeOnline({
                          kind: t.kind,
                          id: t.id,
                          name: t.name,
                          artist: t.artist,
                          album: t.album,
                          cover: t.cover,
                          durationMs: t.durationMs,
                          mediaMid: t.mediaMid,
                          vip: t.vip,
                        });
                      }}
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
                      onClick={(e) => {
                        e.stopPropagation();
                        menuAction(t, "next");
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
                      onClick={(e) => {
                        const r = (
                          e.currentTarget as HTMLElement
                        ).getBoundingClientRect();
                        setMenu({ x: r.left - 150, y: r.bottom + 6, row: t });
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

            {hasMore && (
              <div className="flex justify-center pt-1 pb-2">
                <button
                  className="btn-secondary !py-1.5 !px-4"
                  disabled={searching}
                  onClick={() =>
                    source === "netease" ? neteaseSearch(kw, true) : qqSearch(kw, true)
                  }
                >
                  {searching ? <Loader2 size={13} className="animate-spin" /> : null}
                  加载更多
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 右键菜单 */}
      {menu && (
        <div
          className="fixed z-[75] w-[200px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
          style={{ left: menu.x, top: menu.y }}
          onMouseDown={(e) => e.stopPropagation()}
          onMouseLeave={() => setMenu(null)}
        >
          {[
            { label: "播放", action: "play" as const },
            { label: "下一首播放", action: "next" as const },
            { label: "加入队列", action: "queue" as const },
          ].map((item) => (
            <button
              key={item.action}
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                menuAction(menu.row, item.action);
                setMenu(null);
              }}
            >
              <Play size={13} /> {item.label}
            </button>
          ))}
          <div className="my-1 mx-2 border-t border-[var(--line)]" />
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              setPickerRow(menu.row);
              setMenu(null);
            }}
          >
            <Heart size={13} /> 添加到播放列表…
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              toggleLikeOnline({
                kind: menu.row.kind,
                id: menu.row.id,
                name: menu.row.name,
                artist: menu.row.artist,
                album: menu.row.album,
                cover: menu.row.cover,
                durationMs: menu.row.durationMs,
                mediaMid: menu.row.mediaMid,
                vip: menu.row.vip,
              });
              setMenu(null);
            }}
          >
            <Heart size={13} /> 收藏到“我喜欢”
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
            onClick={() => {
              downloadOnline({
                kind: menu.row.kind,
                id: menu.row.id,
                name: menu.row.name,
                artist: menu.row.artist,
                album: menu.row.album,
                cover: menu.row.cover,
                durationMs: menu.row.durationMs,
                mediaMid: menu.row.mediaMid,
              });
              setMenu(null);
            }}
          >
            <Play size={13} /> 下载到本地
          </button>
        </div>
      )}

      {/* 添加到播放列表弹窗 */}
      <Modal
        open={!!pickerRow}
        onClose={() => setPickerRow(null)}
        title="添加到播放列表"
        width={380}
      >
        <div className="flex flex-col gap-1.5 max-h-[260px] overflow-y-auto">
          {playlists.map((p) => (
            <button
              key={p.id}
              className="h-10 px-3 rounded-lg text-left text-[13px] text-[var(--ink)] hover:bg-[var(--shade)] flex items-center justify-between transition-colors"
              onClick={async () => {
                if (pickerRow) await addOnlineToPlaylist(p.id, pickerRow);
                setPickerRow(null);
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
              if (!newPlName.trim() || !pickerRow) return;
              await createPlaylist(newPlName.trim());
              const pls = useStore.getState().playlists;
              const created = pls[pls.length - 1];
              if (created && pickerRow)
                await addOnlineToPlaylist(created.id, pickerRow);
              setNewPlName("");
              setPickerRow(null);
            }}
          >
            创建并添加
          </button>
        </div>
      </Modal>

      {/* 导入歌单弹窗（网易云 / QQ 音乐） */}
      <Modal
        open={importOpen}
        onClose={() => setImportOpen(false)}
        title={`导入${sourceName}歌单`}
        width={400}
      >
        {importList === null ? (
          <div className="flex items-center justify-center gap-2 text-[13px] text-[var(--ink-2)] py-6">
            <Loader2 size={15} className="animate-spin" /> 获取中…
          </div>
        ) : (
          <div className="flex flex-col gap-1.5 max-h-[320px] overflow-y-auto">
            {importList.map((p) => (
              <button
                key={p.id}
                disabled={importing}
                className="h-10 px-3 rounded-lg text-left text-[13px] text-[var(--ink)] hover:bg-[var(--shade)] flex items-center justify-between transition-colors"
                onClick={async () => {
                  setImporting(true);
                  if (source === "netease") await importNeteasePlaylist(p.id, p.name);
                  else await importQqPlaylist(p.id, p.name);
                  setImporting(false);
                  setImportOpen(false);
                }}
              >
                <span className="truncate">{p.name}</span>
                <span className="text-[11px] text-[var(--ink-2)] shrink-0 ml-3">
                  {p.trackCount} 首
                </span>
              </button>
            ))}
            {!importList.length && (
              <div className="text-[12.5px] text-[var(--ink-2)] py-2">账号下没有歌单</div>
            )}
          </div>
        )}
        <div className="text-[10.5px] text-[var(--ink-3)] mt-3 leading-relaxed">
          导入的歌单以在线条目保存：播放时按账号权益实时获取播放链接，不占用本地磁盘。
        </div>
      </Modal>

      {/* 扫码登录弹窗 */}
      <QrLoginModal source={qrSource} onClose={() => setQrSource(null)} />
    </div>
  );
}

function QrLoginModal({
  source,
  onClose,
}: {
  source: Source | null;
  onClose: () => void;
}) {
  const open = source != null;
  const [qr, setQr] = useState("");
  const [status, setStatus] = useState<
    "loading" | "waiting" | "scanned" | "expired" | "error"
  >("loading");
  const [errMsg, setErrMsg] = useState("");
  const neteaseSetLogin = useStore((s) => s.neteaseSetLogin);
  const qqSetLogin = useStore((s) => s.qqSetLogin);
  const toast = useStore((s) => s.toast);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopPolling = () => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  };

  const startPolling = (src: Source, identifier: string) => {
    timerRef.current = setInterval(async () => {
      try {
        const c =
          src === "netease"
            ? await api.neteaseQrCheck(identifier)
            : await api.qqQrCheck(identifier);
        if (c.status === "waiting") return;
        if (c.status === "scanned") {
          setStatus("scanned");
          return;
        }
        if (c.status === "expired") {
          stopPolling();
          setStatus("expired");
          return;
        }
        if (c.status === "success") {
          stopPolling();
          if (src === "netease") {
            neteaseSetLogin(true, c.nickname ?? "");
            useStore.getState().neteaseSyncLikes();
          } else {
            qqSetLogin(true, c.nickname ?? "");
          }
          toast(`登录成功：${c.nickname ?? ""}`, "success");
          onClose();
        }
      } catch (e) {
        stopPolling();
        setStatus("error");
        setErrMsg(String(e));
      }
    }, 1600);
  };

  const create = async () => {
    if (!source) return;
    stopPolling();
    setStatus("loading");
    setErrMsg("");
    try {
      if (source === "netease") {
        const r = await api.neteaseQrCreate();
        setQr(r.qr);
        setStatus("waiting");
        startPolling("netease", r.key);
      } else {
        const r = await api.qqQrCreate();
        setQr(r.qr);
        setStatus("waiting");
        startPolling("qq", r.qrsig);
      }
    } catch (e) {
      setStatus("error");
      setErrMsg(String(e));
    }
  };

  useEffect(() => {
    if (open) create();
    else {
      stopPolling();
      setQr("");
      setStatus("loading");
    }
    return stopPolling;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const sourceName = source === "qq" ? "QQ 音乐" : "网易云";

  return (
    <Modal open={open} onClose={onClose} title={`扫码登录${sourceName}`} width={360}>
      <div className="flex flex-col items-center gap-4 py-2">
        <div className="w-[240px] h-[240px] rounded-2xl bg-white flex items-center justify-center overflow-hidden">
          {qr ? (
            <img
              src={qr}
              alt="二维码"
              className="w-full h-full"
              draggable={false}
            />
          ) : status === "error" ? (
            <span className="text-[12px] text-rose-500 px-4 text-center">
              {errMsg}
            </span>
          ) : (
            <Loader2 size={26} className="animate-spin text-[var(--ink-2)]" />
          )}
        </div>
        <div className="text-[12.5px] text-[var(--ink-2)] text-center leading-relaxed">
          {status === "loading"
            ? "正在生成二维码…"
            : status === "waiting"
              ? `打开${sourceName === "QQ 音乐" ? "QQ" : "网易云"} App 扫一扫`
              : status === "scanned"
                ? "已在手机上确认，请在手机上点击登录"
                : status === "expired"
                  ? "二维码已过期"
                  : `出错了：${errMsg}`}
          {status === "expired" && (
            <button className="btn-secondary !py-1.5 !px-3 ml-2" onClick={create}>
              刷新
            </button>
          )}
        </div>
        <div className="text-[10.5px] text-[var(--ink-3)] text-center leading-relaxed max-w-[300px]">
          登录凭证仅保存在本机设置中，用于按你的账号权益获取播放链接；RustMusic
          不提供任何绕过会员/版权限制的能力。
        </div>
      </div>
      <div className="flex justify-end mt-3">
        <button className="btn-secondary" onClick={onClose}>
          关闭
        </button>
      </div>
    </Modal>
  );
}
