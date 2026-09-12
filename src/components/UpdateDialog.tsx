import { useEffect } from "react";
import { create } from "zustand";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { Download, Loader2 } from "lucide-react";
import Modal from "./Modal";
import { api, listenEvent } from "../api";
import type { UpdateInfo } from "../types";
import { useStore } from "../store";

type Phase = "available" | "downloading" | "installing";

interface UpdateStore {
  open: boolean;
  phase: Phase;
  info: UpdateInfo | null;
  currentVersion: string;
  received: number;
  total: number;
  error: string;
  show(info: UpdateInfo): void;
  close(): void;
  confirm(): Promise<void>;
  cancel(): Promise<void>;
  setProgress(received: number, total: number): void;
}

const useUpdateStore = create<UpdateStore>()((set, get) => ({
  open: false,
  phase: "available",
  info: null,
  currentVersion: "",
  received: 0,
  total: 0,
  error: "",

  show(info) {
    set({
      open: true,
      phase: "available",
      info,
      received: 0,
      total: info.assetSize,
      error: "",
    });
  },

  close() {
    const { phase } = get();
    // 安装已交接给系统安装程序，不可中断
    if (phase === "installing") return;
    if (phase === "downloading") {
      api.cancelUpdateDownload().catch(() => {});
    }
    set({ open: false });
  },

  async confirm() {
    const { info } = get();
    if (!info || get().phase === "downloading" || get().phase === "installing") return;
    set({ phase: "downloading", received: 0, total: info.assetSize, error: "" });
    try {
      const path = await api.downloadUpdate({
        url: info.assetUrl,
        name: info.assetName,
        size: info.assetSize,
      });
      // 确认后才退出应用：进入安装阶段，进程即将被安装程序接管
      set({ phase: "installing" });
      await api.installUpdate(path);
    } catch (e) {
      // 用户取消（弹窗已关）或临时目录被清理等：回到可重试状态
      if (!get().open) return;
      set({ phase: "available", error: String(e) });
    }
  },

  async cancel() {
    await api.cancelUpdateDownload().catch(() => {});
    set({ open: false });
  },

  setProgress(received, total) {
    set({ received, total });
  },
}));

/** 供设置页"检查更新"打开弹窗 */
export function showUpdateDialog(info: UpdateInfo) {
  useUpdateStore.getState().show(info);
}

function fmtMB(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "…";
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

export default function UpdateDialog() {
  const open = useUpdateStore((s) => s.open);
  const phase = useUpdateStore((s) => s.phase);
  const info = useUpdateStore((s) => s.info);
  const currentVersion = useUpdateStore((s) => s.currentVersion);
  const received = useUpdateStore((s) => s.received);
  const total = useUpdateStore((s) => s.total);
  const error = useUpdateStore((s) => s.error);

  useEffect(() => {
    // 启动自动检查：后端延 2 秒执行，避免抢占启动网络；调试构建后端会跳过
    api.autoCheckUpdate().catch(() => {});
    api
      .getAppInfo()
      .then((i) => useUpdateStore.setState({ currentVersion: i.version }))
      .catch(() => {});
  }, []);

  useEffect(() => {
    let disposed = false;
    const unbinds: (() => void)[] = [];
    (async () => {
      const u1 = await listenEvent<UpdateInfo>("update://available", (i) => {
        useUpdateStore.getState().show(i);
      });
      const u2 = await listenEvent<{ received: number; total: number }>(
        "update://progress",
        (p) => {
          const s = useUpdateStore.getState();
          if (s.open && s.phase === "downloading") s.setProgress(p.received, p.total);
        }
      );
      if (disposed) {
        u1();
        u2();
      } else {
        unbinds.push(u1, u2);
      }
    })();
    return () => {
      disposed = true;
      unbinds.forEach((u) => u());
    };
  }, []);

  if (!open || !info) return null;

  const notesHtml = info.notes
    ? DOMPurify.sanitize(
        marked.parse(info.notes, { async: false, gfm: true, breaks: true }) as string
      )
    : "";
  const pct = total > 0 ? Math.min(100, Math.round((received / total) * 100)) : 0;

  const onNotesClick = (e: React.MouseEvent) => {
    const a = (e.target as HTMLElement).closest("a[href]");
    if (!a) return;
    const href = (a as HTMLAnchorElement).href;
    e.preventDefault();
    if (/^https?:\/\//i.test(href)) {
      api.openUrl(href).catch((err) => useStore.getState().toast(String(err), "error"));
    }
  };

  return (
    <Modal open={open} onClose={() => useUpdateStore.getState().close()} width={560} title="发现新版本">
      {phase === "installing" ? (
        <div className="flex flex-col items-center gap-3 py-8">
          <Loader2 size={24} className="animate-spin text-[var(--accent)]" />
          <div className="text-[13px] text-[var(--ink)]">正在退出并安装更新…</div>
          <div className="text-[11.5px] text-[var(--ink-3)]">
            安装位置保持不变，完成后将自动重启应用
          </div>
        </div>
      ) : (
        <>
          {/* 版本走向 */}
          <div className="flex items-center gap-2 mb-3">
            <span className="text-[12.5px] text-[var(--ink-3)] tabular-nums">
              v{currentVersion || "…"}
            </span>
            <span className="text-[var(--ink-3)]">→</span>
            <span className="text-[13.5px] font-semibold text-[var(--accent-strong)] tabular-nums">
              v{info.version}
            </span>
            {info.publishedAt && (
              <span className="ml-auto text-[11.5px] text-[var(--ink-3)]">
                {info.publishedAt.slice(0, 10)} 发布
              </span>
            )}
          </div>

          {/* 更新说明（markdown 渲染） */}
          <div
            className="release-notes max-h-[46vh] min-h-[72px] overflow-y-auto rounded-xl bg-[var(--shade)] border border-[var(--line)] px-4 py-3"
            onClick={onNotesClick}
            dangerouslySetInnerHTML={{
              __html:
                notesHtml ||
                '<p class="rn-empty">本次更新没有提供更新说明。</p>',
            }}
          />

          {/* 下载进度 */}
          {phase === "downloading" && (
            <div className="mt-4">
              <div className="h-2 rounded-full bg-[var(--shade-strong)] overflow-hidden">
                <div
                  className="h-full rounded-full bg-[var(--accent)] transition-[width] duration-150"
                  style={{ width: `${pct}%` }}
                />
              </div>
              <div className="text-[11.5px] text-[var(--ink-2)] mt-2 tabular-nums">
                正在下载安装包… {fmtMB(received)} / {fmtMB(total)}（{pct}%）
              </div>
            </div>
          )}

          {error && (
            <p className="text-[12px] text-rose-400 mt-3 leading-snug">{error}</p>
          )}

          <div className="flex items-center justify-end gap-2 mt-4">
            {phase === "available" && (
              <>
                <button className="btn-secondary" onClick={() => useUpdateStore.getState().close()}>
                  以后再说
                </button>
                <button className="btn-primary" onClick={() => useUpdateStore.getState().confirm()}>
                  <Download size={14} />
                  立即更新
                </button>
              </>
            )}
            {phase === "downloading" && (
              <>
                <span className="mr-auto text-[11.5px] text-[var(--ink-3)]">
                  下载完成后将自动安装并重启
                </span>
                <button className="btn-secondary" onClick={() => useUpdateStore.getState().cancel()}>
                  取消下载
                </button>
              </>
            )}
          </div>
        </>
      )}
    </Modal>
  );
}
