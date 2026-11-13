//! 三平台扫码登录弹窗（轮询二维码状态，成功后写入登录态）。
//! QQ 音乐额外支持微信扫码：走长轮询（服务端挂起约 15s），与秒级
//! interval 轮询分开处理。
import { useEffect, useRef, useState } from "react";
import { Loader2 } from "lucide-react";
import { useStore } from "../../store";
import { useI18n } from "../../i18n";
import { api } from "../../api";
import Modal from "../../components/Modal";
import type { Source } from "./shared";

type QrStatus = "loading" | "waiting" | "scanned" | "expired" | "error";
type QqMode = "qq" | "wx";

export function QrLoginModal({
  source,
  onClose,
}: {
  source: Source | null;
  onClose: () => void;
}) {
  const { tr } = useI18n();
  const open = source != null;
  const [mode, setMode] = useState<QqMode>("qq");
  const [qr, setQr] = useState("");
  const [status, setStatus] = useState<QrStatus>("loading");
  const [errMsg, setErrMsg] = useState("");
  const neteaseSetLogin = useStore((s) => s.neteaseSetLogin);
  const qqSetLogin = useStore((s) => s.qqSetLogin);
  const kugouSetLogin = useStore((s) => s.kugouSetLogin);
  const toast = useStore((s) => s.toast);
  const uiTheme = useStore((s) => s.uiTheme);
  const isLiquid = uiTheme === "liquid";
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // 微信长轮询循环的取消标志 + 自增序号（新一次 create 使旧循环自然退出）
  const wxLoopRef = useRef<{ cancel: boolean; seq: number }>({ cancel: false, seq: 0 });

  const stopPolling = () => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
    wxLoopRef.current.cancel = true;
    wxLoopRef.current.seq += 1;
  };

  const qqSetLoginAndClose = (nickname?: string) => {
    qqSetLogin(true, nickname ?? "");
    toast(
      nickname
        ? tr(`登录成功：${nickname}`, `Signed in: ${nickname}`)
        : tr("登录成功", "Signed in"),
      "success",
    );
    onClose();
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
            qqSetLoginAndClose(c.nickname);
            return;
          } else {
            kugouSetLogin(true, c.nickname ?? "");
          }
          toast(
            c.nickname
              ? tr(`登录成功：${c.nickname}`, `Signed in: ${c.nickname}`)
              : tr("登录成功", "Signed in"),
            "success",
          );
          onClose();
        }
      } catch (e) {
        stopPolling();
        setStatus("error");
        setErrMsg(String(e));
      }
    }, 1600);
  };

  /** 微信长轮询循环：每次后端调用挂起约 15s，出错退避重试（连续 3 次才报错） */
  const startWxPolling = (uuid: string) => {
    wxLoopRef.current.cancel = false;
    const seq = wxLoopRef.current.seq;
    (async () => {
      let failures = 0;
      while (!wxLoopRef.current.cancel && wxLoopRef.current.seq === seq) {
        try {
          const c = await api.qqWxQrCheck(uuid);
          if (wxLoopRef.current.cancel || wxLoopRef.current.seq !== seq) return;
          failures = 0;
          if (c.status === "waiting") continue;
          if (c.status === "scanned") {
            setStatus("scanned");
            continue;
          }
          if (c.status === "expired") {
            setStatus("expired");
            return;
          }
          if (c.status === "success") {
            qqSetLoginAndClose(c.nickname);
            return;
          }
        } catch (e) {
          if (wxLoopRef.current.cancel || wxLoopRef.current.seq !== seq) return;
          failures += 1;
          if (failures >= 3) {
            setStatus("error");
            setErrMsg(String(e));
            return;
          }
          await new Promise((r) => setTimeout(r, 2000));
        }
      }
    })();
  };

  const create = async () => {
    if (!source) return;
    stopPolling();
    wxLoopRef.current.cancel = false;
    setStatus("loading");
    setErrMsg("");
    try {
      if (source === "netease") {
        const r = await api.neteaseQrCreate();
        setQr(r.qr);
        setStatus("waiting");
        startPolling("netease", r.key);
      } else if (source === "qq" && mode === "wx") {
        const r = await api.qqWxQrCreate();
        setQr(r.qr);
        setStatus("waiting");
        startWxPolling(r.uuid);
      } else if (source === "qq") {
        const r = await api.qqQrCreate();
        setQr(r.qr);
        setStatus("waiting");
        startPolling("qq", r.qrsig);
      } else {
        const r = await api.kugouQrCreate();
        if (!r.qr) {
          throw new Error(
            tr("二维码图片获取失败，请重试", "Failed to load QR code image, please retry"),
          );
        }
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

  // 弹窗打开期间切换 QQ/微信模式：重建二维码
  const switchMode = (m: QqMode) => {
    if (m === mode) return;
    setMode(m);
    // create 依赖最新 mode：直接以内联逻辑重建
    (async () => {
      if (!source || source !== "qq") return;
      stopPolling();
      wxLoopRef.current.cancel = false;
      setStatus("loading");
      setErrMsg("");
      try {
        if (m === "wx") {
          const r = await api.qqWxQrCreate();
          setQr(r.qr);
          setStatus("waiting");
          startWxPolling(r.uuid);
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
    })();
  };

  const sourceName =
    source === "qq"
      ? tr("QQ 音乐", "QQ Music")
      : source === "kugou"
        ? tr("酷狗音乐", "KuGou Music")
        : tr("网易云", "NetEase Cloud");
  const appName =
    mode === "wx" && source === "qq"
      ? tr("微信", "WeChat")
      : source === "qq"
        ? "QQ"
        : source === "kugou"
          ? tr("酷狗", "KuGou")
          : tr("网易云", "NetEase Cloud Music");

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={tr(`扫码登录${sourceName}`, `Sign in to ${sourceName} via QR Code`)}
      width={360}
    >
      <div className="flex flex-col items-center gap-4 py-2">
        {source === "qq" && (
          <div className={isLiquid ? "flex gap-1 rounded-xl liquid-input p-1" : "flex gap-1 rounded-lg bg-[var(--shade)] p-1"}>
            {(["qq", "wx"] as QqMode[]).map((m) => (
              <button
                key={m}
                onClick={() => switchMode(m)}
                className={`rounded-md px-4 py-1 text-[12px] transition-colors ${
                  mode === m
                    ? "bg-[var(--accent)] text-white"
                    : "text-[var(--ink-2)] hover:text-[var(--ink)]"
                }`}
              >
                {m === "qq" ? tr("QQ 扫码", "QQ QR") : tr("微信扫码", "WeChat QR")}
              </button>
            ))}
          </div>
        )}
        <div className="flex h-[240px] w-[240px] items-center justify-center overflow-hidden rounded-2xl bg-white">
          {qr ? (
            <img
              src={qr}
              alt={tr("二维码", "QR code")}
              className="h-full w-full"
              draggable={false}
            />
          ) : status === "error" ? (
            <span className="px-4 text-center text-[12px] text-rose-500">
              {errMsg}
            </span>
          ) : (
            <Loader2 size={26} className="animate-spin text-[var(--ink-2)]" />
          )}
        </div>
        <div className="text-center text-[12.5px] leading-relaxed text-[var(--ink-2)]">
          {status === "loading"
            ? tr("正在生成二维码…", "Generating QR code…")
            : status === "waiting"
              ? tr(`打开${appName} App 扫一扫`, `Scan with the ${appName} app`)
              : status === "scanned"
                ? tr(
                    "已在手机上确认，请在手机上点击登录",
                    "Scanned — please tap Sign In on your phone",
                  )
                : status === "expired"
                  ? tr("二维码已过期", "QR code expired")
                  : tr(`出错了：${errMsg}`, `Error: ${errMsg}`)}
          {status === "expired" && (
            <button className="btn-secondary !py-1.5 !px-3 ml-2" onClick={create}>
              {tr("刷新", "Refresh")}
            </button>
          )}
        </div>
        <div className="max-w-[300px] text-center text-[10.5px] leading-relaxed text-[var(--ink-3)]">
          {mode === "wx" && source === "qq"
            ? tr(
                "登录凭证仅保存在本机设置中，用于按你的账号权益获取播放链接；QQ 与微信账号的音乐资产、付费特权不互通。",
                "Credentials are stored locally only to fetch streams for your account tier; QQ and WeChat music assets and paid entitlements are separate.",
              )
            : tr(
                "登录凭证仅保存在本机设置中，用于按你的账号权益获取播放链接；RustMusic 不提供任何绕过会员/版权限制的能力。",
                "Credentials are stored locally only to fetch streams for your account tier; RustMusic does not bypass VIP or copyright restrictions.",
              )}
        </div>
      </div>
      <div className="mt-3 flex justify-end">
        <button className="btn-secondary" onClick={onClose}>
          {tr("关闭", "Close")}
        </button>
      </div>
    </Modal>
  );
}
