//! 三平台扫码登录弹窗（轮询二维码状态，成功后写入登录态）。
import { useEffect, useRef, useState } from "react";
import { Loader2 } from "lucide-react";
import { useStore } from "../../store";
import { api } from "../../api";
import Modal from "../../components/Modal";
import type { Source } from "./shared";

export function QrLoginModal({
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
  const kugouSetLogin = useStore((s) => s.kugouSetLogin);
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
            : src === "qq"
              ? await api.qqQrCheck(identifier)
              : await api.kugouQrCheck(identifier);
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
          } else if (src === "qq") {
            qqSetLogin(true, c.nickname ?? "");
          } else {
            kugouSetLogin(true, c.nickname ?? "");
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
      } else if (source === "qq") {
        const r = await api.qqQrCreate();
        setQr(r.qr);
        setStatus("waiting");
        startPolling("qq", r.qrsig);
      } else {
        const r = await api.kugouQrCreate();
        if (!r.qr) throw new Error("二维码图片获取失败，请重试");
        setQr(r.qr);
        setStatus("waiting");
        startPolling("kugou", r.key);
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

  const sourceName =
    source === "qq" ? "QQ 音乐" : source === "kugou" ? "酷狗音乐" : "网易云";
  const appName =
    source === "qq" ? "QQ" : source === "kugou" ? "酷狗" : "网易云";

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
              ? `打开${appName} App 扫一扫`
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
