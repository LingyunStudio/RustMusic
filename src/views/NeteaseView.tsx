import { useMemo, useEffect, useRef, useState } from "react";
import {
  Cloud,
  LogOut,
  MoreHorizontal,
  Play,
  Search,
  ShieldCheck,
  Loader2,
} from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import type { NeteaseTrack } from "../types";
import { fmtTime } from "../utils";
import Modal from "../components/Modal";

export default function NeteaseView() {
  const results = useStore((s) => s.neteaseResults);
  const total = useStore((s) => s.neteaseTotal);
  const searching = useStore((s) => s.neteaseSearching);
  const searched = useStore((s) => s.neteaseSearched);
  const loggedIn = useStore((s) => s.neteaseLoggedIn);
  const nickname = useStore((s) => s.neteaseNickname);
  const current = useStore((s) => s.current);
  const playNetease = useStore((s) => s.playNetease);
  const playNext = useStore((s) => s.playNext);
  const addToQueue = useStore((s) => s.addToQueue);
  const neteaseSearch = useStore((s) => s.neteaseSearch);
  const neteaseLogout = useStore((s) => s.neteaseLogout);

  const [kw, setKw] = useState("");
  const [menu, setMenu] = useState<{ x: number; y: number; t: NeteaseTrack } | null>(null);
  const [qrOpen, setQrOpen] = useState(false);

  const submit = () => neteaseSearch(kw);

  const rows = useMemo(() => results, [results]);

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      {/* 头部 */}
      <header className="px-8 pt-7 pb-5">
        <div className="flex items-end justify-between gap-6">
          <div className="min-w-0 anim-rise">
            <div className="flex items-center gap-2 text-[11px] text-[var(--ink-3)] tracking-[0.24em] mb-2">
              <Cloud size={13} />
              网易云音乐 · 在线曲库
            </div>
            <h1 className="text-[30px] font-extrabold leading-none tracking-tight text-[var(--ink)]">
              搜一下，就能听
            </h1>
            <div className="flex items-center gap-3 mt-3 text-[12.5px] text-[var(--ink-2)]">
              {searched && (
                <>
                  <span className="tabular-nums">{results.length} 条结果</span>
                  {total > 0 && (
                    <>
                      <span className="w-1 h-1 rounded-full bg-[var(--ink-3)]" />
                      <span className="tabular-nums">共 {total} 首</span>
                    </>
                  )}
                </>
              )}
            </div>
          </div>

          {/* 登录状态 */}
          <div className="shrink-0 flex items-center gap-2.5">
            {loggedIn ? (
              <>
                <span
                  className="chip text-[var(--accent-strong)]"
                  style={{ background: "rgba(240,162,74,0.12)" }}
                  title="已登录网易云账号"
                >
                  <ShieldCheck size={13} />
                  {nickname || "已登录"}
                </span>
                <button className="btn-secondary" onClick={neteaseLogout}>
                  <LogOut size={13} />
                  退出
                </button>
              </>
            ) : (
              <>
                <span className="text-[11.5px] text-[var(--ink-3)]">
                  登录后可播会员权益内歌曲
                </span>
                <button className="btn-secondary" onClick={() => setQrOpen(true)}>
                  扫码登录
                </button>
              </>
            )}
          </div>
        </div>

        {/* 搜索框 */}
        <div className="flex items-center gap-3 mt-5 max-w-[620px]">
          <div className="relative flex-1">
            <Search
              size={15}
              className="absolute left-3.5 top-1/2 -translate-y-1/2 text-[var(--ink-3)]"
            />
            <input
              type="text"
              value={kw}
              onChange={(e) => setKw(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && submit()}
              placeholder="输入歌名、歌手或专辑…"
              className="w-full h-11 rounded-xl bg-black/25 border border-[var(--line)] pl-10 pr-4 text-[13px] text-[var(--ink)] placeholder:text-[var(--ink-3)] focus:border-[rgba(240,162,74,0.45)] transition-colors"
            />
          </div>
          <button className="btn-primary h-11" onClick={submit} disabled={searching}>
            {searching ? <Loader2 size={14} className="animate-spin" /> : <Search size={14} />}
            搜索
          </button>
        </div>

        {!loggedIn && (
          <button
            onClick={() => setQrOpen(true)}
            className="mt-4 w-full max-w-[620px] flex items-center gap-3 rounded-xl px-4 py-3 text-left transition-colors hover:bg-[rgba(240,162,74,0.08)]"
            style={{
              background: "rgba(240,162,74,0.07)",
              border: "1px solid rgba(240,162,74,0.25)",
            }}
          >
            <ShieldCheck size={16} className="text-[var(--accent)] shrink-0" />
            <span className="text-[12.5px] text-[var(--ink-2)] leading-relaxed">
              当前未登录：可以搜索浏览，但获取播放链接需要登录。
              <span className="text-[var(--accent-strong)]">点击扫码登录</span>
              你的网易云账号（凭证只保存在本机），登录后按账号权益播放，含免费曲目的标准音质与会员曲目。
            </span>
          </button>
        )}
      </header>

      {/* 结果列表 */}
      <div className="flex-1 min-h-0 flex flex-col px-6 pb-4">
        <div className="glass rounded-3xl flex-1 min-h-0 flex flex-col overflow-hidden">
          {rows.length > 0 && (
            <div className="grid grid-cols-[64px_minmax(0,1fr)_minmax(0,0.5fr)_110px_104px] items-center gap-4 h-10 px-5 border-b border-[var(--line)] text-[10.5px] text-[var(--ink-3)] tracking-[0.18em]">
              <span className="text-center">序号</span>
              <span>歌曲</span>
              <span>专辑</span>
              <span className="text-right">时长</span>
              <span className="text-right">操作</span>
            </div>
          )}
          <div className="flex-1 min-h-0 overflow-y-auto px-2.5 py-2.5">
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
                      background: "radial-gradient(circle, var(--accent) 0%, transparent 65%)",
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
                  搜索网易云音乐曲库，双击即可播放
                </div>
                <div className="text-[11.5px] text-[var(--ink-3)] max-w-[420px] text-center leading-relaxed">
                  未登录可播免费曲目的标准音质；扫码登录自己的账号后，可按账号权益播放（含会员曲目）。
                  请支持正版。
                </div>
              </div>
            )}

            {rows.map((t, i) => {
              const active = current?.kind === "netease" && current.nid === t.id;
              const artists = t.ar.map((a) => a.name).join(" / ");
              return (
                <div
                  key={t.id}
                  className={`group grid grid-cols-[64px_minmax(0,1fr)_minmax(0,0.5fr)_110px_104px] items-center gap-4 h-[60px] px-4 rounded-2xl transition-colors cursor-default ${
                    active ? "bg-[rgba(240,162,74,0.1)]" : "hover:bg-white/[0.045]"
                  }`}
                  onDoubleClick={() => playNetease(rows, i)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setMenu({ x: e.clientX, y: e.clientY, t });
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
                        active ? "text-[var(--accent)]" : "bg-[var(--ink)] text-[#241505]"
                      }`}
                      onClick={() => playNetease(rows, i)}
                      title="播放"
                    >
                      <Play size={14} className="fill-current ml-px" />
                    </button>
                  </div>

                  <div className="flex items-center gap-4 min-w-0">
                    {t.al?.picUrl ? (
                      <img
                        src={t.al.picUrl}
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
                          active ? "text-[var(--accent-strong)] font-semibold" : "text-[var(--ink)]"
                        }`}
                      >
                        <span className="truncate">{t.name}</span>
                        {t.fee === 1 && (
                          <span className="text-[9.5px] px-1.5 py-0.5 rounded bg-[rgba(240,162,74,0.16)] text-[var(--accent-strong)] font-bold shrink-0">
                            VIP
                          </span>
                        )}
                      </div>
                      <div className="text-[12px] text-[var(--ink-3)] truncate mt-1">
                        {artists || "未知艺术家"}
                      </div>
                    </div>
                  </div>

                  <div className="text-[12.5px] text-[var(--ink-3)] truncate">
                    {t.al?.name || "未知专辑"}
                  </div>

                  <div className="text-right text-[12.5px] text-[var(--ink-2)] tabular-nums">
                    {fmtTime(t.dt)}
                  </div>

                  <div className="flex items-center justify-end gap-1 pr-1">
                    <button
                      className="btn-ghost w-8 h-8"
                      onClick={(e) => {
                        e.stopPropagation();
                        playNext({ kind: "netease", id: t.id });
                      }}
                      title="下一首播放"
                    >
                      <Play size={14} className="opacity-0 group-hover:opacity-100" />
                    </button>
                    <button
                      className="btn-ghost w-8 h-8"
                      onClick={(e) => {
                        const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                        setMenu({ x: r.left - 150, y: r.bottom + 6, t });
                      }}
                    >
                      <MoreHorizontal size={16} className="opacity-0 group-hover:opacity-100" />
                    </button>
                  </div>
                </div>
              );
            })}
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
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-zinc-200 hover:bg-white/[0.08] text-left"
            onClick={() => {
              const idx = results.findIndex((r) => r.id === menu.t.id);
              playNetease(results, Math.max(0, idx));
              setMenu(null);
            }}
          >
            <Play size={13} /> 播放
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-zinc-200 hover:bg-white/[0.08] text-left"
            onClick={() => {
              playNext({ kind: "netease", id: menu.t.id });
              setMenu(null);
            }}
          >
            <Play size={13} /> 下一首播放
          </button>
          <button
            className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-zinc-200 hover:bg-white/[0.08] text-left"
            onClick={() => {
              addToQueue({ kind: "netease", id: menu.t.id });
              setMenu(null);
            }}
          >
            <Play size={13} /> 加入队列
          </button>
        </div>
      )}

      <QrLoginModal open={qrOpen} onClose={() => setQrOpen(false)} />
    </div>
  );
}

function QrLoginModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [qr, setQr] = useState("");
  const [status, setStatus] = useState<"loading" | "waiting" | "scanned" | "expired" | "error">(
    "loading"
  );
  const [errMsg, setErrMsg] = useState("");
  const neteaseSetLogin = useStore((s) => s.neteaseSetLogin);
  const toast = useStore((s) => s.toast);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopPolling = () => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  };

  const create = async () => {
    stopPolling();
    setStatus("loading");
    setErrMsg("");
    try {
      const r = await api.neteaseQrCreate();
      setQr(r.qr);
      setStatus("waiting");
      timerRef.current = setInterval(async () => {
        try {
          const c = await api.neteaseQrCheck(r.key);
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
            neteaseSetLogin(true, c.nickname ?? "");
            toast(`网易云登录成功：${c.nickname ?? ""}`, "success");
            onClose();
          }
        } catch (e) {
          stopPolling();
          setStatus("error");
          setErrMsg(String(e));
        }
      }, 1600);
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

  const statusText =
    status === "loading"
      ? "正在生成二维码…"
      : status === "waiting"
        ? "打开网易云音乐 App 扫一扫"
        : status === "scanned"
          ? "已在手机上确认，请在手机上点击登录"
          : status === "expired"
            ? "二维码已过期"
            : `出错了：${errMsg}`;

  return (
    <Modal open={open} onClose={onClose} title="扫码登录网易云音乐" width={360}>
      <div className="flex flex-col items-center gap-4 py-2">
        <div className="w-[240px] h-[240px] rounded-2xl bg-white flex items-center justify-center overflow-hidden">
          {qr ? (
            <img src={qr} alt="二维码" className="w-full h-full" draggable={false} />
          ) : status === "error" ? (
            <span className="text-[12px] text-rose-500 px-4 text-center">{errMsg}</span>
          ) : (
            <Loader2 size={26} className="animate-spin text-zinc-400" />
          )}
        </div>
        <div className="text-[12.5px] text-[var(--ink-2)] text-center leading-relaxed">
          {statusText}
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
