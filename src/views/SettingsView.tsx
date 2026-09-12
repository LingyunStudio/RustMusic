import { useEffect, useState } from "react";
import {
  Folder,
  FolderOpen,
  FolderPlus,
  Headphones,
  Loader2,
  RefreshCw,
  Settings as SettingsIcon,
  Trash2,
  X,
} from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import { ACCENTS, loadCustomAccentHex, parseCustomAccent, saveCustomAccentHex } from "../theme";
import { showUpdateDialog } from "../components/UpdateDialog";

const EQ_FREQS = ["31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];

const QUALITIES: { key: string; label: string; desc: string }[] = [
  { key: "standard", label: "标准", desc: "128k" },
  { key: "high", label: "较高", desc: "320k" },
  { key: "lossless", label: "无损", desc: "FLAC" },
];

const PRESETS: Record<string, number[]> = {
  平直: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  流行: [-1, 1, 3, 4, 3, 0, -1, -1, -1, -2],
  摇滚: [4, 3, 2, 0, -1, 0, 2, 4, 4, 3],
  古典: [3, 2, 0, 0, 0, 0, -2, -2, 0, 3],
  爵士: [2, 1, 0, 2, -1, -1, 0, 1, 2, 3],
  电子: [4, 3, 1, 0, -2, 1, 1, 2, 3, 4],
  人声: [-2, -1, 0, 2, 4, 4, 3, 1, 0, -1],
};

const CACHE_LIMITS: { bytes: number; label: string }[] = [
  { bytes: 512 * 1024 * 1024, label: "512 MB" },
  { bytes: 1024 * 1024 * 1024, label: "1 GB" },
  { bytes: 2 * 1024 * 1024 * 1024, label: "2 GB" },
  { bytes: 5 * 1024 * 1024 * 1024, label: "5 GB" },
  { bytes: 0, label: "不限制" },
];

/** 缓存占用展示（GB 感知） */
function fmtCache(bytes: number): string {
  if (bytes <= 0) return "0 MB";
  const gb = bytes / 1024 / 1024 / 1024;
  if (gb >= 1) return `${gb.toFixed(2)} GB`;
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024).toFixed(0)} KB`;
}

export default function SettingsView() {
  const folders = useStore((s) => s.folders);
  const scan = useStore((s) => s.scan);
  const addFolderByDialog = useStore((s) => s.addFolderByDialog);
  const removeFolder = useStore((s) => s.removeFolder);
  const rescan = useStore((s) => s.rescan);
  const eqGains = useStore((s) => s.eqGains);
  const eqEnabled = useStore((s) => s.eqEnabled);
  const setEq = useStore((s) => s.setEq);
  const speed = useStore((s) => s.speed);
  const setSpeed = useStore((s) => s.setSpeed);
  const quality = useStore((s) => s.quality);
  const setQuality = useStore((s) => s.setQuality);
  const closeAction = useStore((s) => s.closeAction);
  const setCloseAction = useStore((s) => s.setCloseAction);
  const autoUpdate = useStore((s) => s.autoUpdate);
  const setAutoUpdate = useStore((s) => s.setAutoUpdate);
  const theme = useStore((s) => s.theme);
  const setTheme = useStore((s) => s.setTheme);
  const accent = useStore((s) => s.accent);
  const setAccent = useStore((s) => s.setAccent);
  const desktopLyricsOn = useStore((s) => s.desktopLyricsOn);
  const desktopLyricsLock = useStore((s) => s.desktopLyricsLock);
  const openDesktopLyrics = useStore((s) => s.openDesktopLyrics);
  const closeDesktopLyrics = useStore((s) => s.closeDesktopLyrics);
  const unlockDesktopLyrics = useStore((s) => s.unlockDesktopLyrics);
  const dlyricsColors = useStore((s) => s.dlyricsColors);
  const setDlyricsColors = useStore((s) => s.setDlyricsColors);
  const clearCache = useStore((s) => s.clearCache);
  const cacheLimit = useStore((s) => s.cacheLimit);
  const setCacheLimit = useStore((s) => s.setCacheLimit);
  const cacheBytes = useStore((s) => s.cacheBytes);
  const [saveDir, setSaveDir] = useState("");
  const [saveDirDefault, setSaveDirDefault] = useState("");
  const [preset, setPreset] = useState("平直");
  const [devices, setDevices] = useState<
    { name: string; isDefault: boolean }[]
  >([]);
  const [deviceCurrent, setDeviceCurrent] = useState("");
  const [devicePref, setDevicePref] = useState<string | null>(null);
  const [deviceSwitching, setDeviceSwitching] = useState(false);
  // 自定义强调色（取色器当前值 / 回显上次选择）
  const [customHex, setCustomHex] = useState(loadCustomAccentHex);
  // 关于与更新
  const [appVersion, setAppVersion] = useState("");
  const [checkingUpdate, setCheckingUpdate] = useState(false);

  const refreshDevices = async () => {
    try {
      const d = await api.listOutputDevices();
      setDevices(d.devices);
      setDeviceCurrent(d.current);
      setDevicePref(d.preference);
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    }
  };

  const pickDevice = async (value: string) => {
    // "" = 跟随系统默认
    setDeviceSwitching(true);
    try {
      await api.setOutputDevice(value === "" ? null : value);
      await refreshDevices();
      useStore.getState().toast(
        value === "" ? "已跟随系统默认输出设备" : `输出已切换到「${value}」`,
        "success"
      );
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    } finally {
      setDeviceSwitching(false);
    }
  };

  useEffect(() => {
    api.saveDirGet().then((d) => {
      setSaveDir(d.dir);
      setSaveDirDefault(d.default);
    });
    refreshDevices();
    api.getAppInfo().then((i) => setAppVersion(i.version)).catch(() => {});
    useStore.getState().refreshCacheBytes();
    // 设备热插拔（插入耳机等）后端自动切换时同步 UI
    let unbind: (() => void) | undefined;
    let disposed = false;
    import("../api").then(({ listenEvent }) =>
      listenEvent<{ current: string }>("device://changed", () => {
        if (!disposed) refreshDevices();
      }).then((u) => {
        if (disposed) u();
        else unbind = u;
      })
    );
    return () => {
      disposed = true;
      unbind?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pickSaveDir = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择下载保存目录",
      });
      if (!selected || typeof selected !== "string") return;
      await api.saveDirSet(selected);
      setSaveDir(selected);
      useStore.getState().toast("下载目录已更新", "success");
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    }
  };

  const setBand = (i: number, v: number) => {
    const g = [...eqGains];
    g[i] = v;
    setEq(g, eqEnabled);
    setPreset("自定义");
  };

  const checkForUpdate = async () => {
    setCheckingUpdate(true);
    try {
      const info = await api.checkUpdate();
      if (info) {
        showUpdateDialog(info);
      } else {
        useStore.getState().toast(`当前已是最新版本（v${appVersion}）`, "success");
      }
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    } finally {
      setCheckingUpdate(false);
    }
  };

  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-5 pt-5 pb-[92px]">
      <h1 className="text-[22px] font-bold flex items-center gap-2.5 mb-5">
        <SettingsIcon size={20} className="text-[var(--accent)]" />
        设置
      </h1>

      <div className="flex flex-col gap-4 max-w-[760px]">
        {/* 通用 */}
        <section style={{ ["--row-idx" as string]: 0 }} className="anim-row glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-4">通用</h2>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">关闭窗口时</span>
            <div className="flex items-center gap-1.5">
              {(
                [
                  { key: "tray", label: "最小化到托盘" },
                  { key: "exit", label: "直接退出应用" },
                ] as const
              ).map((o) => (
                <button
                  key={o.key}
                  onClick={() => setCloseAction(o.key)}
                  className={`px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                    closeAction === o.key
                      ? "text-[var(--accent-strong)] font-medium"
                      : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                  }`}
                  style={
                    closeAction === o.key ? { background: "var(--accent-weak)" } : undefined
                  }
                >
                  {o.label}
                </button>
              ))}
            </div>
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            最小化到托盘后音乐继续播放，从任务栏托盘图标可重新打开主界面或完全退出。
          </p>
        </section>

        {/* 外观 */}
        <section style={{ ["--row-idx" as string]: 1 }} className="anim-row glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-4">外观</h2>
          <div className="flex items-center gap-4 mb-4">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">界面模式</span>
            <div className="flex items-center gap-1.5">
              {(
                [
                  { key: "light", label: "浅色" },
                  { key: "dark", label: "深色" },
                ] as const
              ).map((t) => (
                <button
                  key={t.key}
                  onClick={() => setTheme(t.key)}
                  className={`px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                    theme === t.key
                      ? "text-[var(--accent-strong)] font-medium"
                      : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                  }`}
                  style={
                    theme === t.key
                      ? { background: "var(--accent-weak)" }
                      : undefined
                  }
                >
                  {t.label}
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">强调色</span>
            <div className="flex items-center gap-2 flex-wrap">
              {ACCENTS.map((a) => (
                <button
                  key={a.key}
                  onClick={() => setAccent(a.key)}
                  title={a.label}
                  className={`w-7 h-7 rounded-full transition-transform hover:scale-110 ${
                    accent === a.key ? "ring-2 ring-offset-2" : ""
                  }`}
                  style={{
                    background: `linear-gradient(135deg, ${a.base}, ${a.strong})`,
                    ["--tw-ring-color" as string]: a.base,
                    ["--tw-ring-offset-color" as string]: "var(--bg)",
                  }}
                />
              ))}

              {/* 自定义取色盘：点击打开系统取色器，任意颜色即席生效 */}
              <label
                title="自定义颜色"
                className={`relative w-7 h-7 rounded-full cursor-pointer transition-transform hover:scale-110 ${
                  accent.startsWith("custom:") ? "ring-2 ring-offset-2" : ""
                }`}
                style={{
                  background: `conic-gradient(#f71, #ee4, #4d5, #4cd, #55f, #a4e, #f71)`,
                  boxShadow: "inset 0 0 0 3.5px var(--bg)",
                  ["--tw-ring-color" as string]: "var(--accent)",
                  ["--tw-ring-offset-color" as string]: "var(--bg)",
                }}
              >
                <input
                  type="color"
                  value={customHex}
                  onChange={(e) => {
                    const hex = e.target.value;
                    setCustomHex(hex);
                    saveCustomAccentHex(hex);
                    setAccent(`custom:${hex}`);
                  }}
                  className="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
                />
              </label>
            </div>
          </div>

          {/* 桌面歌词开关 */}
          <div className="flex items-center gap-4 mt-4 pt-4 border-t border-[var(--line)]">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">桌面歌词</span>
            <button
              className={`px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                desktopLyricsOn
                  ? "text-[var(--accent-strong)] font-medium"
                  : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
              }`}
              style={desktopLyricsOn ? { background: "var(--accent-weak)" } : undefined}
              onClick={() =>
                desktopLyricsOn
                  ? desktopLyricsLock
                    ? unlockDesktopLyrics()
                    : closeDesktopLyrics()
                  : openDesktopLyrics()
              }
            >
              {desktopLyricsOn ? (desktopLyricsLock ? "已锁定（点按解锁）" : "已开启") : "已关闭"}
            </button>
            <span className="text-[11.5px] text-[var(--ink-3)]">
              快捷键 L：开启 / 关闭 / 解锁
            </span>
          </div>

          {/* 桌面歌词配色：已唱 / 未唱 / 下一句 */}
          <div className="flex items-center gap-4 mt-3 pl-0">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">歌词配色</span>
            <div className="flex items-center gap-5">
              {(
                [
                  { key: "sung", label: "已唱" },
                  { key: "unsung", label: "未唱" },
                  { key: "next", label: "下一句" },
                ] as const
              ).map((c) => {
                // rgba() 字符串对原生取色器不可解析：取 rgb 值拼 #hex 回显
                const raw = dlyricsColors[c.key];
                const m = raw.match(
                  /rgba?\((\d+),\s*(\d+),\s*(\d+)/
                );
                const hex = m
                  ? `#${[m[1], m[2], m[3]]
                      .map((v) => (+v).toString(16).padStart(2, "0"))
                      .join("")}`
                  : /^#[0-9a-fA-F]{6}$/.test(raw)
                    ? raw
                    : "#ffffff";
                return (
                  <label
                    key={c.key}
                    className="flex items-center gap-1.5 cursor-pointer"
                    title={`${c.label}颜色（点击选择）`}
                  >
                    <span
                      className="w-5 h-5 rounded-full inline-block"
                      style={{
                        background: hex,
                        boxShadow: "0 0 0 1px var(--line), inset 0 0 0 1px rgba(0,0,0,0.06)",
                      }}
                    />
                    <span className="text-[11.5px] text-[var(--ink-2)]">{c.label}</span>
                    <input
                      type="color"
                      value={hex}
                      onChange={(e) =>
                        setDlyricsColors({ ...dlyricsColors, [c.key]: e.target.value })
                      }
                      className="w-0 h-0 opacity-0"
                    />
                  </label>
                );
              })}
            </div>
          </div>
        </section>

        {/* 音乐文件夹 */}
        <section style={{ ["--row-idx" as string]: 2 }} className="anim-row glass rounded-2xl p-5">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-[14.5px] font-semibold">音乐文件夹</h2>
            <div className="flex items-center gap-2">
              {scan.active && (
                <span className="text-[12px] text-[var(--accent)] flex items-center gap-1.5">
                  <Loader2 size={12} className="animate-spin" />
                  {scan.total ? `${scan.done}/${scan.total}` : "扫描中…"}
                </span>
              )}
              <button className="btn-secondary !py-1.5 !px-3" onClick={rescan}>
                <RefreshCw size={12.5} />
                重新扫描
              </button>
              <button className="btn-primary !py-1.5 !px-3" onClick={addFolderByDialog}>
                <FolderPlus size={13} />
                添加文件夹
              </button>
            </div>
          </div>
          {folders.length ? (
            <div className="flex flex-col gap-1.5">
              {folders.map((f) => (
                <div
                  key={f.id}
                  className="group flex items-center gap-2.5 h-10 px-3 rounded-lg bg-white/[0.03] hover:bg-[var(--shade)] transition-colors"
                >
                  <Folder size={14} className="text-[var(--ink-2)] shrink-0" />
                  <span className="text-[12.5px] text-[var(--ink)] truncate">{f.path}</span>
                  <button
                    className="btn-ghost w-7 h-7 ml-auto shrink-0 opacity-0 group-hover:opacity-100"
                    onClick={() =>
                      api.openFolder(f.path).catch((e) => useStore.getState().toast(String(e), "error"))
                    }
                    title="在资源管理器中打开"
                  >
                    <FolderOpen size={14} />
                  </button>
                  <button
                    className="btn-ghost w-7 h-7 shrink-0 opacity-0 group-hover:opacity-100 hover:!text-rose-400"
                    onClick={() => removeFolder(f.id)}
                    title="移除（不会删除文件）"
                  >
                    <X size={14} />
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-[12.5px] text-[var(--ink-2)] py-2">
              还没有添加文件夹。也可以直接把文件 / 文件夹拖进窗口。
            </div>
          )}
        </section>

        {/* 均衡器 */}
        <section style={{ ["--row-idx" as string]: 3 }} className="anim-row glass rounded-2xl p-5">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-[14.5px] font-semibold">
              均衡器
              <span className="ml-2 text-[11.5px] font-normal text-[var(--ink-2)]">10 段</span>
            </h2>
            <div className="flex items-center gap-1.5">
              {Object.keys(PRESETS).concat("自定义").map((name) => (
                <button
                  key={name}
                  onClick={() => {
                    if (name === "自定义") return;
                    setEq(PRESETS[name], eqEnabled);
                    setPreset(name);
                  }}
                  className={`px-2.5 py-1 rounded-full text-[11.5px] transition-colors ${
                    preset === name
                      ? "bg-[var(--shade-strong)] text-[var(--ink)]"
                      : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                  }`}
                >
                  {name}
                </button>
              ))}
              <button
                className={`ml-2 relative w-10 h-[22px] rounded-full transition-colors ${
                  eqEnabled ? "bg-[var(--accent)]" : "bg-[var(--shade-strong)]"
                }`}
                onClick={() => setEq(eqGains, !eqEnabled)}
                title={eqEnabled ? "关闭均衡器" : "启用均衡器"}
              >
                <span
                  className={`absolute top-[3px] w-4 h-4 rounded-full bg-white shadow transition-all ${
                    eqEnabled ? "left-[21px]" : "left-[3px]"
                  }`}
                />
              </button>
            </div>
          </div>
          <div className={`flex justify-between gap-2 ${eqEnabled ? "" : "opacity-40"}`}>
            {EQ_FREQS.map((label, i) => (
              <div key={label} className="flex flex-col items-center gap-1.5 flex-1">
                <span className="text-[10.5px] text-[var(--ink-2)] tabular-nums">
                  {eqGains[i] > 0 ? "+" : ""}
                  {eqGains[i].toFixed(0)}
                </span>
                <VSlider
                  value={eqGains[i]}
                  min={-12}
                  max={12}
                  onChange={(v) => setBand(i, Math.round(v))}
                />
                <span className="text-[10.5px] text-[var(--ink-3)]">{label}</span>
              </div>
            ))}
          </div>
        </section>

        {/* 播放 */}
        <section style={{ ["--row-idx" as string]: 4 }} className="anim-row glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-4">播放</h2>
          <div className="flex items-center gap-4 mb-4">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">在线音质</span>
            <div className="flex items-center gap-1.5">
              {QUALITIES.map((q) => (
                <button
                  key={q.key}
                  onClick={() => setQuality(q.key)}
                  className={`px-3 py-1.5 rounded-full text-[12px] transition-colors ${
                    quality === q.key
                      ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                      : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                  }`}
                >
                  {q.label}
                  <span className="ml-1 text-[10px] opacity-70">{q.desc}</span>
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">播放速度</span>
            <input
              type="range"
              min={0.5}
              max={2}
              step={0.05}
              value={speed}
              onChange={(e) => setSpeed(parseFloat(e.target.value))}
              className="flex-1"
              style={{ ["--fill" as string]: `${((speed - 0.5) / 1.5) * 100}%` }}
            />
            <span className="text-[12px] text-[var(--ink)] tabular-nums w-10 text-right">
              {speed.toFixed(2)}x
            </span>
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            变速通过重采样实现，音调会随之变化；均衡器实时作用于所有播放。
          </p>
        </section>

        {/* 输出设备 */}
        <section style={{ ["--row-idx" as string]: 5 }} className="anim-row glass rounded-2xl p-5">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-[14.5px] font-semibold">输出设备</h2>
            <button
              className="btn-ghost w-7 h-7"
              onClick={refreshDevices}
              title="刷新设备列表"
            >
              <RefreshCw size={13} />
            </button>
          </div>
          <div className="flex items-center gap-3">
            <Headphones size={14} className="text-[var(--ink-2)] shrink-0" />
            <select
              className="flex-1 h-9 rounded-lg bg-[var(--shade)] border border-[var(--line)] px-2.5 text-[12.5px] text-[var(--ink)] outline-none focus:border-[rgba(240,162,74,0.45)]"
              value={devicePref ?? ""}
              disabled={deviceSwitching}
              onChange={(e) => pickDevice(e.target.value)}
            >
              <option value="">跟随系统默认{deviceCurrent && devicePref == null ? `（${deviceCurrent}）` : ""}</option>
              {devices.map((d) => (
                <option key={d.name} value={d.name}>
                  {d.name}
                  {d.isDefault ? "（系统默认）" : ""}
                </option>
              ))}
            </select>
            {deviceSwitching && (
              <Loader2 size={14} className="animate-spin text-[var(--accent)] shrink-0" />
            )}
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            跟随系统默认时，插入耳机等设备热插拔会自动切换并从当前进度续播；固定设备则始终使用所选设备。
          </p>
        </section>

        {/* 下载目录 */}
        <section style={{ ["--row-idx" as string]: 6 }} className="anim-row glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-3">下载保存目录</h2>
          <div className="flex items-center gap-3">
            <Folder size={14} className="text-[var(--ink-2)] shrink-0" />
            <span className="text-[12.5px] text-[var(--ink)] truncate flex-1">
              {saveDir || saveDirDefault}
            </span>
            <button className="btn-secondary !py-1.5 !px-3" onClick={pickSaveDir}>
              更改目录
            </button>
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            在线歌曲“下载到本地”将保存到此目录，并自动加入资料库（含标签与歌词）。
          </p>
        </section>

        {/* 缓存 */}
        <section style={{ ["--row-idx" as string]: 7 }} className="anim-row glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-2">缓存</h2>
          <div className="flex items-center gap-4 mb-3">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">当前占用</span>
            <span className="text-[12.5px] text-[var(--ink)] tabular-nums">
              {cacheBytes == null ? "查询中…" : fmtCache(cacheBytes)}
            </span>
            <button
              className="btn-ghost !py-1 !px-2 ml-1"
              onClick={() => useStore.getState().refreshCacheBytes()}
              title="刷新占用"
            >
              <RefreshCw size={12} />
            </button>
          </div>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">自动清理</span>
            <div className="flex items-center gap-1.5">
              {CACHE_LIMITS.map((o) => (
                <button
                  key={o.bytes}
                  onClick={() => setCacheLimit(o.bytes)}
                  className={`px-3 py-1.5 rounded-full text-[12px] transition-colors ${
                    cacheLimit === o.bytes
                      ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                      : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                  }`}
                >
                  {o.label}
                </button>
              ))}
            </div>
          </div>
          <button className="btn-secondary !text-rose-300/80 hover:!bg-rose-500/15 mt-3" onClick={clearCache}>
            <Trash2 size={13} />
            立即清理全部缓存
          </button>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            在线歌曲播放时会自动缓存到应用数据目录（同一首歌同一音质只缓存一份，重复播放不再下载）。
            超过上限后从最旧缓存开始自动清理，正在播放的文件不受影响；「不限制」则永久保留。
          </p>
        </section>

        {/* 关于与更新 */}
        <section style={{ ["--row-idx" as string]: 8 }} className="anim-row glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-3">关于与更新</h2>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">当前版本</span>
            <span className="text-[12.5px] text-[var(--ink)] tabular-nums">
              {appVersion ? `v${appVersion}` : "…"}
            </span>
            <button
              className="btn-secondary !py-1.5 !px-3"
              onClick={checkForUpdate}
              disabled={checkingUpdate}
            >
              {checkingUpdate ? (
                <Loader2 size={12.5} className="animate-spin" />
              ) : (
                <RefreshCw size={12.5} />
              )}
              检查更新
            </button>
          </div>
          <div className="flex items-center gap-4 mt-3">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">自动检查</span>
            <button
              className={`relative w-10 h-[22px] rounded-full transition-colors ${
                autoUpdate ? "bg-[var(--accent)]" : "bg-[var(--shade-strong)]"
              }`}
              onClick={() => setAutoUpdate(!autoUpdate)}
              title={autoUpdate ? "关闭自动检查更新" : "开启自动检查更新"}
            >
              <span
                className={`absolute top-[3px] w-4 h-4 rounded-full bg-white shadow transition-all ${
                  autoUpdate ? "left-[21px]" : "left-[3px]"
                }`}
              />
            </button>
            <span className="text-[11.5px] text-[var(--ink-3)]">
              启动时自动检测 GitHub 上的新版本
            </span>
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            发现新版本时会展示更新说明，确认后自动下载并原地安装（不改变安装位置），完成后自动重启应用。
          </p>
        </section>

        <div className="text-[11.5px] text-[var(--ink-3)] px-1 pb-2">
          RustMusic {appVersion ? `v${appVersion}` : ""} · Rust + Tauri 2 + React ·
          引擎 rodio / symphonia · 界面仅支持 Windows（架构上保留跨平台能力）
        </div>
      </div>
    </div>
  );
}

/** 垂直滑块（均衡器用） */
function VSlider({
  value,
  min,
  max,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  onChange: (v: number) => void;
}) {
  const H = 120;
  const zero = ((0 - min) / (max - min)) * H;
  const y = ((value - min) / (max - min)) * H;
  return (
    <div
      className="relative w-7 rounded-full bg-[var(--shade)] cursor-pointer touch-none"
      style={{ height: H }}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        const rect = e.currentTarget.getBoundingClientRect();
        const ratio = 1 - (e.clientY - rect.top) / rect.height;
        onChange(Math.round(min + ratio * (max - min)));
      }}
      onPointerMove={(e) => {
        if (e.buttons !== 1) return;
        const rect = e.currentTarget.getBoundingClientRect();
        const ratio = Math.min(1, Math.max(0, 1 - (e.clientY - rect.top) / rect.height));
        onChange(Math.round(min + ratio * (max - min)));
      }}
    >
      <div
        className="absolute left-0 right-0 rounded-full"
        style={{
          top: Math.min(zero, y),
          height: Math.max(2, Math.abs(zero - y)),
          background:
            value >= 0
              ? "linear-gradient(180deg, var(--accent-strong), var(--accent))"
              : "linear-gradient(180deg, #d97706, #b45309)",
        }}
      />
      <div
        className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 w-5 h-2.5 rounded-full bg-white shadow"
        style={{ top: y }}
      />
    </div>
  );
}
