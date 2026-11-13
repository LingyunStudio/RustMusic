import { useEffect, useMemo, useState } from "react";
import {
  Folder,
  FolderOpen,
  FolderPlus,
  Headphones,
  Loader2,
  RefreshCw,
  Settings as SettingsIcon,
  Trash2,
} from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import { Select } from "../components/Select";
import { PLATFORM } from "../platform";
import { getMenubarLyricsColor, setMenubarLyricsColor } from "../store/menubar";
import {
  getTaskbarLyricsColor,
  getTaskbarLyricsDisplay,
  setTaskbarLyricsColor,
  setTaskbarLyricsDisplay,
} from "../store/taskbar";
import {
  ACCENTS,
  UI_THEMES,
  accentLabel,
  loadCustomAccentHex,
  saveCustomAccentHex,
  uiThemeLabel,
} from "../theme";
import { useI18n } from "../i18n";
import { showUpdateDialog } from "../components/UpdateDialog";

const EQ_FREQS = ["31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];

const PRESETS: { key: string; zh: string; en: string; gains: number[] }[] = [
  { key: "flat", zh: "平直", en: "Flat", gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  { key: "pop", zh: "流行", en: "Pop", gains: [-1, 1, 3, 4, 3, 0, -1, -1, -1, -2] },
  { key: "rock", zh: "摇滚", en: "Rock", gains: [4, 3, 2, 0, -1, 0, 2, 4, 4, 3] },
  { key: "classical", zh: "古典", en: "Classical", gains: [3, 2, 0, 0, 0, 0, -2, -2, 0, 3] },
  { key: "jazz", zh: "爵士", en: "Jazz", gains: [2, 1, 0, 2, -1, -1, 0, 1, 2, 3] },
  { key: "electronic", zh: "电子", en: "Electronic", gains: [4, 3, 1, 0, -2, 1, 1, 2, 3, 4] },
  { key: "vocal", zh: "人声", en: "Vocal", gains: [-2, -1, 0, 2, 4, 4, 3, 1, 0, -1] },
];

const CACHE_LIMITS: { bytes: number; label?: string }[] = [
  { bytes: 512 * 1024 * 1024, label: "512 MB" },
  { bytes: 1024 * 1024 * 1024, label: "1 GB" },
  { bytes: 2 * 1024 * 1024 * 1024, label: "2 GB" },
  { bytes: 5 * 1024 * 1024 * 1024, label: "5 GB" },
  { bytes: 0 },
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
  const { tr } = useI18n();
  const uiTheme = useStore((s) => s.uiTheme);
  const isSkeuo = uiTheme === "skeuo";
  const isLiquid = uiTheme === "liquid";

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
  const qualities = useStore((s) => s.qualities);
  const setSourceQuality = useStore((s) => s.setSourceQuality);
  const language = useStore((s) => s.language);
  const setLanguage = useStore((s) => s.setLanguage);
  const closeAction = useStore((s) => s.closeAction);
  const setCloseAction = useStore((s) => s.setCloseAction);
  const autoUpdate = useStore((s) => s.autoUpdate);
  const wasapiExclusive = useStore((s) => s.wasapiExclusive);
  const setWasapiExclusive = useStore((s) => s.setWasapiExclusive);
  const setAutoUpdate = useStore((s) => s.setAutoUpdate);
  const theme = useStore((s) => s.theme);
  const setTheme = useStore((s) => s.setTheme);
  const setUiTheme = useStore((s) => s.setUiTheme);
  const accent = useStore((s) => s.accent);
  const setAccent = useStore((s) => s.setAccent);
  const desktopLyricsOn = useStore((s) => s.desktopLyricsOn);
  const desktopLyricsLock = useStore((s) => s.desktopLyricsLock);
  const openDesktopLyrics = useStore((s) => s.openDesktopLyrics);
  const closeDesktopLyrics = useStore((s) => s.closeDesktopLyrics);
  const unlockDesktopLyrics = useStore((s) => s.unlockDesktopLyrics);
  const dlyricsColors = useStore((s) => s.dlyricsColors);
  const setDlyricsColors = useStore((s) => s.setDlyricsColors);
  const lyricOffsetMs = useStore((s) => s.lyricOffsetMs);
  const setLyricOffsetMs = useStore((s) => s.setLyricOffsetMs);
  const menubarLyricsOn = useStore((s) => s.menubarLyricsOn);
  const setMenubarLyrics = useStore((s) => s.setMenubarLyrics);
  const taskbarLyricsOn = useStore((s) => s.taskbarLyricsOn);
  const setTaskbarLyrics = useStore((s) => s.setTaskbarLyrics);
  const [menubarColor, setMenubarColorState] = useState(() => getMenubarLyricsColor());
  const [installingExt, setInstallingExt] = useState(false);
  const [taskbarColor, setTaskbarColorState] = useState(() => getTaskbarLyricsColor());
  const [taskbarDisplay, setTaskbarDisplayState] = useState(() => getTaskbarLyricsDisplay());
  const [taskbarDisplays, setTaskbarDisplays] = useState<
    { id: string; name: string; primary: boolean; has_taskbar: boolean }[]
  >([]);
  const clearCache = useStore((s) => s.clearCache);
  const cacheLimit = useStore((s) => s.cacheLimit);
  const setCacheLimit = useStore((s) => s.setCacheLimit);
  const cacheBytes = useStore((s) => s.cacheBytes);
  const [saveDir, setSaveDir] = useState("");
  const [saveDirDefault, setSaveDirDefault] = useState("");
  // 挂载时按持久化的增益识别已选预设
  const [preset, setPreset] = useState(() => {
    const hit = PRESETS.find(
      (p) =>
        p.gains.length === eqGains.length &&
        p.gains.every((v, i) => Math.abs(v - eqGains[i]) < 0.01)
    );
    return hit ? hit.key : "custom";
  });
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
  const [platform, setPlatform] = useState<string>(() => PLATFORM);
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
    setDeviceSwitching(true);
    try {
      await api.setOutputDevice(value === "" ? null : value);
      await refreshDevices();
      useStore.getState().toast(
        value === ""
          ? tr("已跟随系统默认输出设备", "Following system default output device")
          : tr(`输出已切换到「${value}」`, `Output switched to "${value}"`),
        "success"
      );
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    } finally {
      setDeviceSwitching(false);
    }
  };

  useEffect(() => {
    api
      .saveDirGet()
      .then((d) => {
        setSaveDir(d.dir);
        setSaveDirDefault(d.default);
      })
      .catch(() => {});
    refreshDevices();
    api
      .taskbarLyricsListDisplays()
      .then(setTaskbarDisplays)
      .catch(() => {});
    api
      .getAppInfo()
      .then((i) => {
        setAppVersion(i.version);
        if (i.platform) setPlatform(i.platform);
      })
      .catch(() => {});
    useStore.getState().refreshCacheBytes();
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
        title: tr("选择下载保存目录", "Select Download Folder"),
      });
      if (!selected || typeof selected !== "string") return;
      await api.saveDirSet(selected);
      setSaveDir(selected);
      useStore.getState().toast(tr("下载目录已更新", "Download folder updated"), "success");
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    }
  };

  const setBand = (i: number, v: number) => {
    const g = [...eqGains];
    g[i] = v;
    setEq(g, eqEnabled);
    setPreset("custom");
  };

  const checkForUpdate = async () => {
    setCheckingUpdate(true);
    try {
      const info = await api.checkUpdate();
      if (info) {
        showUpdateDialog(info);
      } else {
        useStore.getState().toast(
          tr(`当前已是最新版本（v${appVersion}）`, `Already up to date (v${appVersion})`),
          "success"
        );
      }
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    } finally {
      setCheckingUpdate(false);
    }
  };

  const sectionClass = `anim-row ${isLiquid ? "liquid-panel !rounded-2xl" : "glass rounded-2xl"} p-5`;

  return (
    <div
      className={`flex-1 min-h-0 overflow-y-auto ${
        isLiquid ? "pl-3 pr-2 pt-2 pb-40" : "px-5 pt-5 pb-[92px]"
      }`}
    >
      <h1
        className={
          isLiquid
            ? "text-[24px] font-bold flex items-center gap-3 mb-6 text-[var(--ink)] tracking-tight"
            : "text-[22px] font-bold flex items-center gap-2.5 mb-5"
        }
      >
        <SettingsIcon size={isLiquid ? 22 : 20} className="text-[var(--accent)]" />
        {tr("设置", "Settings")}
      </h1>

      <div className={`flex flex-col gap-4 ${isLiquid ? "max-w-[820px]" : "max-w-[760px]"}`}>
        {/* 通用 */}
        <section style={{ ["--row-idx" as string]: 0 }} className={sectionClass}>
          <h2 className="text-[14.5px] font-semibold mb-4">{tr("通用", "General")}</h2>
          <div className="flex items-center gap-4 mb-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("界面语言", "Language")}
            </span>
            <div className={isSkeuo ? "skeuo-segmented" : "flex items-center gap-1.5 flex-wrap"}>
              {(
                [
                  { key: "auto", label: tr("自动（跟随系统）", "Auto (System)") },
                  { key: "zh", label: "简体中文" },
                  { key: "en", label: "English" },
                ] as const
              ).map((o) => (
                <button
                  key={o.key}
                  onClick={() => setLanguage(o.key)}
                  className={
                    isSkeuo
                      ? `skeuo-segmented-btn ${language === o.key ? "active" : ""}`
                      : `px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                          language === o.key
                            ? "text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                  style={
                    !isSkeuo && language === o.key
                      ? { background: "var(--accent-weak)" }
                      : undefined
                  }
                >
                  {o.label}
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("关闭窗口时", "On Close")}
            </span>
            <div className={isSkeuo ? "skeuo-segmented" : "flex items-center gap-1.5"}>
              {(
                [
                  { key: "tray", label: tr("最小化到托盘", "Minimize to Tray") },
                  { key: "exit", label: tr("直接退出应用", "Quit App") },
                ] as const
              ).map((o) => (
                <button
                  key={o.key}
                  onClick={() => setCloseAction(o.key)}
                  className={
                    isSkeuo
                      ? `skeuo-segmented-btn ${closeAction === o.key ? "active" : ""}`
                      : `px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                          closeAction === o.key
                            ? "text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                  style={
                    !isSkeuo && closeAction === o.key
                      ? { background: "var(--accent-weak)" }
                      : undefined
                  }
                >
                  {o.label}
                </button>
              ))}
            </div>
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            {tr(
              "最小化到托盘后音乐继续播放，从任务栏托盘图标可重新打开主界面或完全退出。",
              "Audio continues playing in the tray; use the tray icon to reopen the window or quit."
            )}
          </p>
        </section>

        {/* 外观 */}
        <section style={{ ["--row-idx" as string]: 1 }} className={sectionClass}>
          <h2 className="text-[14.5px] font-semibold mb-4">{tr("外观", "Appearance")}</h2>
          <div className="flex items-center gap-4 mb-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("界面风格", "UI Style")}
            </span>
            <div className={isSkeuo ? "skeuo-segmented" : "flex items-center gap-1.5"}>
              {UI_THEMES.map((t) => (
                <button
                  key={t.key}
                  onClick={() => setUiTheme(t.key)}
                  className={
                    isSkeuo
                      ? `skeuo-segmented-btn ${uiTheme === t.key ? "active" : ""}`
                      : `px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                          uiTheme === t.key
                            ? "text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                  style={!isSkeuo && uiTheme === t.key ? { background: "var(--accent-weak)" } : undefined}
                >
                  {uiThemeLabel(t)}
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-center gap-4 mb-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("界面模式", "Color Mode")}
            </span>
            <div className={isSkeuo ? "skeuo-segmented" : "flex items-center gap-1.5"}>
              {(
                [
                  { key: "light", label: tr("浅色", "Light") },
                  { key: "dark", label: tr("深色", "Dark") },
                ] as const
              ).map((t) => (
                <button
                  key={t.key}
                  onClick={() => setTheme(t.key)}
                  className={
                    isSkeuo
                      ? `skeuo-segmented-btn ${theme === t.key ? "active" : ""}`
                      : `px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                          theme === t.key
                            ? "text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                  style={
                    !isSkeuo && theme === t.key
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
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("强调色", "Accent Color")}
            </span>
            <div className="flex items-center gap-2 flex-wrap">
              {ACCENTS.map((a) => (
                <button
                  key={a.key}
                  onClick={() => setAccent(a.key)}
                  title={accentLabel(a)}
                  className={
                    isSkeuo
                      ? `w-7 h-7 rounded-full transition-all hover:scale-110 active:scale-95 ${
                          accent === a.key ? "ring-2 ring-offset-2 scale-105" : ""
                        }`
                      : `w-7 h-7 rounded-full transition-transform hover:scale-110 ${
                          accent === a.key ? "ring-2 ring-offset-2" : ""
                        }`
                  }
                  style={{
                    background: `linear-gradient(135deg, ${a.base}, ${a.strong})`,
                    ...(isSkeuo
                      ? {
                          border: "1px solid rgba(0, 0, 0, 0.35)",
                          boxShadow:
                            "0 2px 5px rgba(0,0,0,0.35), inset 0 1px 1px rgba(255,255,255,0.6), inset 0 -1px 2px rgba(0,0,0,0.35)",
                        }
                      : {}),
                    ["--tw-ring-color" as string]: a.base,
                    ["--tw-ring-offset-color" as string]: "var(--bg)",
                  }}
                />
              ))}

              {/* 自定义取色盘 */}
              <label
                title={tr("自定义颜色", "Custom color")}
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
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("桌面歌词", "Desktop Lyrics")}
            </span>
            <button
              className={
                isSkeuo
                  ? `skeuo-pill ${desktopLyricsOn ? "active" : ""}`
                  : `px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                      desktopLyricsOn
                        ? "text-[var(--accent-strong)] font-medium"
                        : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                    }`
              }
              style={!isSkeuo && desktopLyricsOn ? { background: "var(--accent-weak)" } : undefined}
              onClick={() =>
                desktopLyricsOn
                  ? desktopLyricsLock
                    ? unlockDesktopLyrics()
                    : closeDesktopLyrics()
                  : openDesktopLyrics()
              }
            >
              {desktopLyricsOn
                ? desktopLyricsLock
                  ? tr("已锁定（点按解锁）", "Locked (Click to Unlock)")
                  : tr("已开启", "On")
                : tr("已关闭", "Off")}
            </button>
            <span className="text-[11.5px] text-[var(--ink-3)]">
              {tr("快捷键 L：开启 / 关闭 / 解锁", "Shortcut L: Toggle / Unlock")}
            </span>
          </div>

          {/* 桌面歌词配色 */}
          <div className="flex items-center gap-4 mt-3 pl-0">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("桌面歌词配色", "Lyrics Colors")}
            </span>
            <LyricColorPickers
              values={dlyricsColors}
              onChange={(k, v) => setDlyricsColors({ ...dlyricsColors, [k]: v })}
            />
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            {tr(
              "已唱 = 当前行卡拉OK 染色；未唱 = 当前行文字；下一句 = 即将演唱的一行。播放页歌词颜色由封面自动适配（取封面主色保证对比可读），不在此设置。",
              "Sung = active line fill; Unsung = current line text; Next = upcoming line. Now Playing lyric colors adapt automatically from album art."
            )}
          </p>

          {/* 歌词时间偏移微调（蓝牙延迟等场景可手动延后/提前） */}
          <div className="flex items-center gap-4 mt-4 pt-4 border-t border-[var(--line)]">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("歌词偏移", "Lyrics Offset")}
            </span>
            <div className="flex items-center gap-1.5">
              <button
                className="btn-secondary !py-1 !px-2.5 !text-[11.5px]"
                onClick={() => setLyricOffsetMs(lyricOffsetMs - 50)}
                title={tr("延后 50ms（歌词偏快时点此）", "Delay 50ms (if lyrics are early)")}
              >
                -50ms
              </button>
              <span className="text-[12px] text-[var(--ink)] tabular-nums min-w-[84px] text-center">
                {lyricOffsetMs === 0
                  ? "0 ms"
                  : lyricOffsetMs < 0
                    ? tr(`延后 ${-lyricOffsetMs}ms`, `Delay ${-lyricOffsetMs}ms`)
                    : tr(`提前 ${lyricOffsetMs}ms`, `Ahead ${lyricOffsetMs}ms`)}
              </span>
              <button
                className="btn-secondary !py-1 !px-2.5 !text-[11.5px]"
                onClick={() => setLyricOffsetMs(lyricOffsetMs + 50)}
                title={tr("提前 50ms（歌词偏慢时点此）", "Advance 50ms (if lyrics are late)")}
              >
                +50ms
              </button>
              {lyricOffsetMs !== 0 && (
                <button
                  className="btn-secondary !py-1 !px-2.5 !text-[11px] ml-1"
                  onClick={() => setLyricOffsetMs(0)}
                >
                  {tr("归零", "Reset")}
                </button>
              )}
            </div>
            <span className="text-[11.5px] text-[var(--ink-3)]">
              {tr(
                "已自动补偿声卡缓冲；使用蓝牙耳机等高延迟设备时可继续调负延后",
                "Audio buffer delay is auto-compensated; lower further for high-latency Bluetooth headphones"
              )}
            </span>
          </div>

          {/* 菜单栏歌词（Linux/GNOME 顶栏） */}
          {platform === "linux" && (
            <>
              <div className="flex items-center gap-4 mt-4 pt-4 border-t border-[var(--line)]">
                <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
                  {tr("菜单栏歌词", "Top Bar Lyrics")}
                </span>
                <button
                  className={
                    isSkeuo
                      ? `skeuo-pill ${menubarLyricsOn ? "active" : ""}`
                      : `px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                          menubarLyricsOn
                            ? "text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                  style={!isSkeuo && menubarLyricsOn ? { background: "var(--accent-weak)" } : undefined}
                  onClick={() => setMenubarLyrics(!menubarLyricsOn)}
                >
                  {menubarLyricsOn ? tr("已开启", "On") : tr("已关闭", "Off")}
                </button>
                <button
                  className="btn-secondary !py-1 !px-2.5 !text-[11px]"
                  disabled={installingExt}
                  onClick={async () => {
                    setInstallingExt(true);
                    try {
                      useStore.getState().toast(await api.menubarExtensionInstall(), "info");
                    } catch (e) {
                      useStore.getState().toast(String(e), "error");
                    } finally {
                      setInstallingExt(false);
                    }
                  }}
                >
                  {installingExt
                    ? tr("安装中…", "Installing…")
                    : tr("安装 GNOME 扩展", "Install GNOME Extension")}
                </button>
                <label
                  className="relative w-7 h-7 shrink-0 overflow-hidden rounded-full cursor-pointer transition-transform hover:scale-110"
                  style={{
                    background: menubarColor || "var(--shade-strong)",
                    boxShadow: "0 0 0 1px var(--line), inset 0 0 0 1px rgba(0,0,0,0.06)",
                  }}
                  title={tr("顶栏歌词颜色", "Top bar lyrics color")}
                >
                  {!menubarColor && (
                    <span className="absolute inset-0 flex items-center justify-center text-[9px] text-[var(--ink-3)]">
                      {tr("默认", "Auto")}
                    </span>
                  )}
                  <input
                    type="color"
                    value={menubarColor || "#ffffff"}
                    onChange={(e) => {
                      setMenubarColorState(e.target.value);
                      setMenubarLyricsColor(e.target.value);
                    }}
                    className="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
                  />
                </label>
                {menubarColor && (
                  <button
                    className="btn-secondary !py-1 !px-2.5 !text-[11px]"
                    onClick={() => {
                      setMenubarColorState("");
                      setMenubarLyricsColor("");
                    }}
                  >
                    {tr("默认", "Default")}
                  </button>
                )}
              </div>
              <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
                {tr(
                  "在系统顶栏显示当前歌词行，与悬浮歌词相互独立；色块可自定义文字颜色（默认跟随顶栏主题）。首次使用先点「安装 GNOME 扩展」；Wayland 下安装后需注销并重新登录一次扩展才会出现。",
                  'Shows the current lyric line in the system top bar independently from desktop lyrics. Color follows your top bar theme by default. Click "Install GNOME Extension" first (on Wayland, log out and back in for it to appear).'
                )}
              </p>
            </>
          )}

          {/* 任务栏歌词（Windows） */}
          {platform === "windows" && (
            <>
              <div className="flex items-center gap-4 mt-4 pt-4 border-t border-[var(--line)]">
                <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
                  {tr("任务栏歌词", "Taskbar Lyrics")}
                </span>
                <button
                  className={
                    isSkeuo
                      ? `skeuo-pill ${taskbarLyricsOn ? "active" : ""}`
                      : `px-4 py-1.5 rounded-full text-[12px] transition-colors ${
                          taskbarLyricsOn
                            ? "text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                  style={!isSkeuo && taskbarLyricsOn ? { background: "var(--accent-weak)" } : undefined}
                  onClick={() => setTaskbarLyrics(!taskbarLyricsOn)}
                >
                  {taskbarLyricsOn ? tr("已开启", "On") : tr("已关闭", "Off")}
                </button>
                <Select
                  value={taskbarDisplay}
                  className="!h-7 !text-[11.5px] min-w-[175px]"
                  options={[
                    { value: "all", label: tr("全部显示器", "All Displays") },
                    ...taskbarDisplays.map((d) => ({
                      value: d.id,
                      label: `${d.primary ? tr("主显示器", "Primary") : tr("副屏", "Secondary")} · ${d.name}${d.has_taskbar ? "" : tr("（无任务栏）", " (No taskbar)")}`,
                    })),
                  ]}
                  onChange={(v) => {
                    setTaskbarDisplayState(v);
                    setTaskbarLyricsDisplay(v);
                  }}
                />
                <label
                  className="relative w-7 h-7 shrink-0 overflow-hidden rounded-full cursor-pointer transition-transform hover:scale-110"
                  style={{
                    background: taskbarColor || "var(--shade-strong)",
                    boxShadow: "0 0 0 1px var(--line), inset 0 0 0 1px rgba(0,0,0,0.06)",
                  }}
                  title={tr("任务栏歌词颜色", "Taskbar lyrics color")}
                >
                  {!taskbarColor && (
                    <span className="absolute inset-0 flex items-center justify-center text-[9px] text-[var(--ink-3)]">
                      {tr("默认", "Auto")}
                    </span>
                  )}
                  <input
                    type="color"
                    value={taskbarColor || "#ffffff"}
                    onChange={(e) => {
                      setTaskbarColorState(e.target.value);
                      setTaskbarLyricsColor(e.target.value);
                    }}
                    className="absolute inset-0 w-full h-full opacity-0 cursor-pointer"
                  />
                </label>
                {taskbarColor && (
                  <button
                    className="btn-secondary !py-1 !px-2.5 !text-[11px]"
                    onClick={() => {
                      setTaskbarColorState("");
                      setTaskbarLyricsColor("");
                    }}
                  >
                    {tr("默认", "Default")}
                  </button>
                )}
              </div>
              <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
                {tr(
                  "在任务栏右侧托盘图标旁显示当前歌词行，随托盘图标增减自动移动，与悬浮歌词相互独立；色块可自定义文字颜色（默认随任务栏深浅取黑/白）。副屏任务栏无托盘区，歌词紧跟应用图标右侧。",
                  "Displays current lyrics beside the system tray independently from desktop lyrics. Color adapts to taskbar contrast by default; secondary displays show lyrics next to taskbar app icons."
                )}
              </p>
            </>
          )}
        </section>

        {/* 音乐文件夹 */}
        <section style={{ ["--row-idx" as string]: 2 }} className={sectionClass}>
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-[14.5px] font-semibold">{tr("音乐文件夹", "Music Folders")}</h2>
            <div className="flex items-center gap-2">
              {scan.active && (
                <span className="text-[12px] text-[var(--accent)] flex items-center gap-1.5">
                  <Loader2 size={12} className="animate-spin" />
                  {scan.total ? `${scan.done}/${scan.total}` : tr("扫描中…", "Scanning…")}
                </span>
              )}
              <button className="btn-secondary !py-1.5 !px-3" onClick={rescan}>
                <RefreshCw size={12.5} />
                {tr("重新扫描", "Rescan")}
              </button>
              <button className="btn-primary !py-1.5 !px-3" onClick={addFolderByDialog}>
                <FolderPlus size={13} />
                {tr("添加文件夹", "Add Folder")}
              </button>
            </div>
          </div>
          {folders.length ? (
            <div className="flex flex-col gap-1.5">
              {folders.map((f) => (
                <div
                  key={f.id}
                  className={`flex items-center gap-2.5 h-11 px-3 rounded-lg transition-colors ${
                    isLiquid
                      ? "bg-[var(--shade)] hover:bg-[var(--shade-hover)]"
                      : "bg-white/[0.03] hover:bg-[var(--shade)]"
                  }`}
                >
                  <Folder size={14} className="text-[var(--ink-2)] shrink-0" />
                  <span
                    className="text-[12.5px] text-[var(--ink)] truncate flex-1 min-w-0"
                    title={f.path}
                  >
                    {f.path}
                  </span>
                  <div className="flex items-center gap-1.5 shrink-0">
                    <button
                      className="btn-secondary !py-1 !px-2.5 text-[12px] flex items-center gap-1.5"
                      onClick={() =>
                        api.openFolder(f.path).catch((e) => useStore.getState().toast(String(e), "error"))
                      }
                      title={tr("在文件管理器中打开", "Open in File Manager")}
                    >
                      <FolderOpen size={13} />
                      {tr("在文件管理器中打开", "Show in Folder")}
                    </button>
                    <button
                      className="btn-secondary !py-1 !px-2.5 text-[12px] flex items-center gap-1.5 hover:!text-rose-400"
                      onClick={() => removeFolder(f.id)}
                      title={tr("移除（不会删除文件）", "Remove folder (files stay on disk)")}
                    >
                      <Trash2 size={13} />
                      {tr("移除", "Remove")}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-[12.5px] text-[var(--ink-2)] py-2">
              {tr(
                "还没有添加文件夹。也可以直接把文件 / 文件夹拖进窗口。",
                "No folders added yet. You can also drag and drop files or folders here."
              )}
            </div>
          )}
        </section>

        {/* 均衡器 */}
        <section style={{ ["--row-idx" as string]: 3 }} className={sectionClass}>
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-[14.5px] font-semibold">
              {tr("均衡器", "Equalizer")}
              <span className="ml-2 text-[11.5px] font-normal text-[var(--ink-2)]">
                {tr("10 段", "10-Band")}
              </span>
            </h2>
            <div className="flex items-center gap-1.5 flex-wrap">
              {PRESETS.map((p) => (
                <button
                  key={p.key}
                  onClick={() => {
                    setEq(p.gains, eqEnabled);
                    setPreset(p.key);
                  }}
                  className={
                    isSkeuo
                      ? `skeuo-pill !py-1 !px-2.5 !text-[11.5px] ${preset === p.key ? "active" : ""}`
                      : `px-2.5 py-1 rounded-full text-[11.5px] transition-colors ${
                          preset === p.key
                            ? "bg-[var(--shade-strong)] text-[var(--ink)]"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                >
                  {tr(p.zh, p.en)}
                </button>
              ))}
              <button
                onClick={() => {}}
                className={
                  isSkeuo
                    ? `skeuo-pill !py-1 !px-2.5 !text-[11.5px] ${preset === "custom" ? "active" : ""}`
                    : `px-2.5 py-1 rounded-full text-[11.5px] transition-colors ${
                        preset === "custom"
                          ? "bg-[var(--shade-strong)] text-[var(--ink)]"
                          : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                      }`
                }
              >
                {tr("自定义", "Custom")}
              </button>
              {isSkeuo ? (
                <button
                  className={`skeuo-switch ml-2 ${eqEnabled ? "active" : ""}`}
                  onClick={() => setEq(eqGains, !eqEnabled)}
                  title={eqEnabled ? tr("关闭均衡器", "Disable EQ") : tr("启用均衡器", "Enable EQ")}
                >
                  <span className="skeuo-switch-thumb" />
                </button>
              ) : (
                <button
                  className={`ml-2 relative w-10 h-[22px] rounded-full transition-colors ${
                    eqEnabled ? "bg-[var(--accent)]" : "bg-[var(--shade-strong)]"
                  }`}
                  onClick={() => setEq(eqGains, !eqEnabled)}
                  title={eqEnabled ? tr("关闭均衡器", "Disable EQ") : tr("启用均衡器", "Enable EQ")}
                >
                  <span
                    className={`absolute top-[3px] w-4 h-4 rounded-full bg-white shadow transition-all ${
                      eqEnabled ? "left-[21px]" : "left-[3px]"
                    }`}
                  />
                </button>
              )}
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
                  isSkeuo={isSkeuo}
                  onChange={(v) => setBand(i, Math.round(v))}
                />
                <span className="text-[10.5px] text-[var(--ink-3)]">{label}</span>
              </div>
            ))}
          </div>
        </section>

        {/* 播放 */}
        <section style={{ ["--row-idx" as string]: 4 }} className={sectionClass}>
          <h2 className="text-[14.5px] font-semibold mb-4">{tr("播放", "Playback")}</h2>
          <div className="flex items-center gap-4 mb-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("网易云音质", "NetEase Quality")}
            </span>
            <div className="flex items-center gap-1.5 flex-wrap">
              {(
                [
                  { value: "standard", label: tr("标准", "Standard"), desc: "128k" },
                  { value: "higher", label: tr("较高", "Higher"), desc: "192k" },
                  { value: "high", label: tr("极高", "Very High"), desc: "320k · VIP" },
                  { value: "lossless", label: tr("无损", "Lossless"), desc: "FLAC · VIP" },
                  { value: "hires", label: "Hi-Res", desc: "24bit · VIP" },
                  { value: "jyeffect", label: tr("高清环绕声", "Surround"), desc: "VIP" },
                  { value: "sky", label: tr("沉浸环绕声", "Spatial"), desc: "SVIP" },
                  { value: "jymaster", label: tr("超清母带", "Master"), desc: "SVIP" },
                ] as const
              ).map((o) => (
                <button
                  key={o.value}
                  onClick={() => setSourceQuality("netease", o.value)}
                  className={
                    isSkeuo
                      ? `skeuo-pill ${qualities.netease === o.value ? "active" : ""}`
                      : `px-3 py-1.5 rounded-full text-[12px] transition-colors ${
                          qualities.netease === o.value
                            ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                >
                  {o.label}
                  <span className="ml-1 text-[10px] opacity-70">{o.desc}</span>
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-center gap-4 mb-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("QQ 音质", "QQ Quality")}
            </span>
            <div className="flex items-center gap-1.5 flex-wrap">
              {(
                [
                  { value: "standard", label: tr("标准", "Standard"), desc: "128k" },
                  { value: "high", label: tr("HQ 高品质", "HQ High"), desc: "320k · VIP" },
                  { value: "lossless", label: tr("SQ 无损", "SQ Lossless"), desc: "FLAC · VIP" },
                  { value: "hires", label: "Hi-Res", desc: "24bit · VIP" },
                  { value: "atmos_2", label: tr("臻品音质 2.0", "Supreme 2.0"), desc: "SVIP" },
                  { value: "atmos", label: tr("臻品全景声", "Atmos"), desc: "SVIP" },
                  { value: "master", label: tr("臻品母带", "Master"), desc: "192kHz · SVIP" },
                ] as const
              ).map((o) => (
                <button
                  key={o.value}
                  onClick={() => setSourceQuality("qq", o.value)}
                  className={
                    isSkeuo
                      ? `skeuo-pill ${qualities.qq === o.value ? "active" : ""}`
                      : `px-3 py-1.5 rounded-full text-[12px] transition-colors ${
                          qualities.qq === o.value
                            ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                >
                  {o.label}
                  <span className="ml-1 text-[10px] opacity-70">{o.desc}</span>
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-center gap-4 mb-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("酷狗音质", "Kugou Quality")}
            </span>
            <div className="flex items-center gap-1.5 flex-wrap">
              {(
                [
                  { value: "standard", label: tr("标准", "Standard"), desc: "128k" },
                  { value: "high", label: tr("HQ 高品", "HQ High"), desc: "320k · VIP" },
                  { value: "lossless", label: tr("SQ 无损", "SQ Lossless"), desc: "FLAC · VIP" },
                  { value: "hires", label: "Hi-Res", desc: "24bit · VIP" },
                  { value: "viper_clear", label: tr("蝰蛇超清", "Viper Clear"), desc: "SVIP" },
                  { value: "viper_atmos", label: tr("蝰蛇全景声", "Viper Atmos"), desc: "SVIP" },
                  { value: "viper_tape", label: tr("蝰蛇母带", "Viper Master"), desc: "SVIP" },
                ] as const
              ).map((o) => (
                <button
                  key={o.value}
                  onClick={() => setSourceQuality("kugou", o.value)}
                  className={
                    isSkeuo
                      ? `skeuo-pill ${qualities.kugou === o.value ? "active" : ""}`
                      : `px-3 py-1.5 rounded-full text-[12px] transition-colors ${
                          qualities.kugou === o.value
                            ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                >
                  {o.label}
                  <span className="ml-1 text-[10px] opacity-70">{o.desc}</span>
                </button>
              ))}
            </div>
          </div>
          <span className="text-[11px] text-[var(--ink-3)] block mb-3">
            {tr(
              "各音源独立设置（播放、缓存与下载同步生效）。标有 VIP / SVIP 的音质需账号具备对应会员权益；当曲目无该档音源或账号权益不足时，将沿音质阶梯自动向下一档无损/高品回退。",
              "Per-source preference applies to playback, caching, and downloads. VIP/SVIP tiers require matching account privileges; unavailable tiers automatically fall back down the quality ladder."
            )}
          </span>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("播放速度", "Playback Speed")}
            </span>
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
            {tr(
              "变速通过重采样实现，音调会随之变化；均衡器实时作用于所有播放。",
              "Speed adjustment uses resampling and shifts pitch; the equalizer applies live to all playback."
            )}
          </p>
        </section>

        {/* 输出设备 */}
        <section style={{ ["--row-idx" as string]: 5 }} className={sectionClass}>
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-[14.5px] font-semibold">{tr("输出设备", "Output Device")}</h2>
            <button
              className="btn-ghost w-7 h-7"
              onClick={refreshDevices}
              title={tr("刷新设备列表", "Refresh device list")}
            >
              <RefreshCw size={13} />
            </button>
          </div>
          <div className="flex items-center gap-3">
            <Headphones size={14} className="text-[var(--ink-2)] shrink-0" />
            <Select
              className="flex-1"
              disabled={deviceSwitching}
              value={devicePref ?? ""}
              onChange={(v) => pickDevice(v)}
              options={[
                {
                  value: "",
                  label: tr(
                    `跟随系统默认${deviceCurrent && devicePref == null ? `（${deviceCurrent}）` : ""}`,
                    `Follow System Default${deviceCurrent && devicePref == null ? ` (${deviceCurrent})` : ""}`
                  ),
                },
                ...devices.map((d) => ({
                  value: d.name,
                  label: `${d.name}${d.isDefault ? tr("（系统默认）", " (System Default)") : ""}`,
                })),
              ]}
            />
            {deviceSwitching && (
              <Loader2 size={14} className="animate-spin text-[var(--accent)] shrink-0" />
            )}
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            {tr(
              "跟随系统默认时，插入耳机等设备热插拔会自动切换并从当前进度续播；固定设备则始终使用所选设备。",
              "When following system default, plugging/unplugging devices switches automatically and resumes playback. Selecting a specific device keeps output pinned to it."
            )}
          </p>
          {platform === "windows" && (
            <>
              <div className="flex items-center gap-4 mt-4">
                <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
                  {tr("独占模式", "Exclusive Mode")}
                </span>
                {isSkeuo ? (
                  <button
                    className={`skeuo-switch ${wasapiExclusive ? "active" : ""}`}
                    onClick={() => setWasapiExclusive(!wasapiExclusive)}
                    title={
                      wasapiExclusive
                        ? tr("关闭 WASAPI 独占模式", "Disable WASAPI Exclusive")
                        : tr("开启 WASAPI 独占模式", "Enable WASAPI Exclusive")
                    }
                  >
                    <span className="skeuo-switch-thumb" />
                  </button>
                ) : (
                  <button
                    className={`relative w-10 h-[22px] rounded-full transition-colors ${
                      wasapiExclusive ? "bg-[var(--accent)]" : "bg-[var(--shade-strong)]"
                    }`}
                    onClick={() => setWasapiExclusive(!wasapiExclusive)}
                    title={
                      wasapiExclusive
                        ? tr("关闭 WASAPI 独占模式", "Disable WASAPI Exclusive")
                        : tr("开启 WASAPI 独占模式", "Enable WASAPI Exclusive")
                    }
                  >
                    <span
                      className={`absolute top-[3px] w-4 h-4 rounded-full bg-white shadow transition-all ${
                        wasapiExclusive ? "left-[21px]" : "left-[3px]"
                      }`}
                    />
                  </button>
                )}
                <span className="text-[11.5px] text-[var(--ink-3)]">
                  {tr(
                    "绕过系统混音器直连声卡（WASAPI Exclusive）",
                    "Bypass Windows mixer directly to audio device (WASAPI Exclusive)"
                  )}
                </span>
              </div>
              <p className="text-[11.5px] text-[var(--ink-3)] mt-2 leading-relaxed">
                {tr(
                  "WASAPI 独占模式下，应用将独占声卡、绕过 Windows 音量混音器：采样率按源文件直通输出，不经系统重采样与音效处理，尽可能保留原始音质。注意：独占播放期间其它应用将暂时无法发声；独占需要声卡驱动支持，蓝牙/网络音箱等设备驱动不支持独占，若当前设备不支持或系统禁用了独占授权，该曲目会自动回退普通模式播放（以提示说明原因）；均衡器与音量在独占模式下仍有效；开启后下一首生效，关闭则立即切回普通模式（从当前进度继续播放，其它应用恢复出声）。",
                  "In WASAPI Exclusive mode, RustMusic takes exclusive control of the audio device and bypasses the Windows mixer so source sample rates pass through bit-perfectly without OS resampling or DSP. Other apps will be muted while active. Requires driver support (Bluetooth/network devices are unsupported and fall back to shared mode automatically with a toast). EQ and volume still apply; enabling takes effect on the next track, disabling switches back immediately."
                )}
              </p>
            </>
          )}
        </section>

        {/* 下载目录 */}
        <section style={{ ["--row-idx" as string]: 6 }} className={sectionClass}>
          <h2 className="text-[14.5px] font-semibold mb-3">{tr("下载保存目录", "Download Folder")}</h2>
          <div className="flex items-center gap-3">
            <Folder size={14} className="text-[var(--ink-2)] shrink-0" />
            <span
              className="text-[12.5px] text-[var(--ink)] truncate flex-1 min-w-0"
              title={saveDir || saveDirDefault}
            >
              {saveDir || saveDirDefault}
            </span>
            <div className="flex items-center gap-2 shrink-0">
              <button
                className="btn-secondary !py-1.5 !px-3 flex items-center gap-1.5"
                onClick={() => {
                  const target = saveDir || saveDirDefault;
                  if (target) {
                    api.openFolder(target).catch((e) => useStore.getState().toast(String(e), "error"));
                  }
                }}
                title={tr("在资源管理器中显示", "Open in File Manager")}
              >
                <FolderOpen size={13} />
                {tr("在资源管理器中显示", "Show in Folder")}
              </button>
              <button className="btn-secondary !py-1.5 !px-3" onClick={pickSaveDir}>
                {tr("更改目录", "Change Folder")}
              </button>
            </div>
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            {tr(
              "在线歌曲“下载到本地”将保存到此目录，并自动加入资料库（含标签与歌词）。",
              'Online tracks saved via "Download to Local" are stored here and added to your library with tags and lyrics.'
            )}
          </p>
        </section>

        {/* 缓存 */}
        <section style={{ ["--row-idx" as string]: 7 }} className={sectionClass}>
          <h2 className="text-[14.5px] font-semibold mb-2">{tr("缓存", "Cache")}</h2>
          <div className="flex items-center gap-4 mb-3">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("当前占用", "Cache Size")}
            </span>
            <span className="text-[12.5px] text-[var(--ink)] tabular-nums">
              {cacheBytes == null ? tr("查询中…", "Calculating…") : fmtCache(cacheBytes)}
            </span>
            <button
              className="btn-ghost !py-1 !px-2 ml-1"
              onClick={() => useStore.getState().refreshCacheBytes()}
              title={tr("刷新占用", "Refresh size")}
            >
              <RefreshCw size={12} />
            </button>
          </div>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("自动清理", "Size Limit")}
            </span>
            <div className="flex items-center gap-1.5 flex-wrap">
              {CACHE_LIMITS.map((o) => (
                <button
                  key={o.bytes}
                  onClick={() => setCacheLimit(o.bytes)}
                  className={
                    isSkeuo
                      ? `skeuo-pill ${cacheLimit === o.bytes ? "active" : ""}`
                      : `px-3 py-1.5 rounded-full text-[12px] transition-colors ${
                          cacheLimit === o.bytes
                            ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                            : "text-[var(--ink-2)] hover:text-[var(--ink)] hover:bg-[var(--shade-hover)]"
                        }`
                  }
                >
                  {o.label ?? tr("不限制", "Unlimited")}
                </button>
              ))}
            </div>
          </div>
          <button className="btn-secondary !text-rose-300/80 hover:!bg-rose-500/15 mt-3" onClick={clearCache}>
            <Trash2 size={13} />
            {tr("立即清理全部缓存", "Clear All Cache")}
          </button>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            {tr(
              "在线歌曲播放时会自动缓存到应用数据目录（同一首歌同一音质只缓存一份，重复播放不再下载）。超过上限后从最旧缓存开始自动清理，正在播放的文件不受影响；「不限制」则永久保留。",
              "Online streams are cached automatically per track and quality tier. When exceeding the limit, oldest entries are pruned automatically while the playing track is preserved."
            )}
          </p>
        </section>

        {/* 关于与更新 */}
        <section style={{ ["--row-idx" as string]: 8 }} className={sectionClass}>
          <h2 className="text-[14.5px] font-semibold mb-3">{tr("关于与更新", "About & Updates")}</h2>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("当前版本", "Version")}
            </span>
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
              {tr("检查更新", "Check for Updates")}
            </button>
          </div>
          <div className="flex items-center gap-4 mt-3">
            <span className="text-[12.5px] text-[var(--ink-2)] min-w-[116px] shrink-0">
              {tr("自动检查", "Auto Check")}
            </span>
            {isSkeuo ? (
              <button
                className={`skeuo-switch ${autoUpdate ? "active" : ""}`}
                onClick={() => setAutoUpdate(!autoUpdate)}
                title={
                  autoUpdate
                    ? tr("关闭自动检查更新", "Disable auto update check")
                    : tr("开启自动检查更新", "Enable auto update check")
                }
              >
                <span className="skeuo-switch-thumb" />
              </button>
            ) : (
              <button
                className={`relative w-10 h-[22px] rounded-full transition-colors ${
                  autoUpdate ? "bg-[var(--accent)]" : "bg-[var(--shade-strong)]"
                }`}
                onClick={() => setAutoUpdate(!autoUpdate)}
                title={
                  autoUpdate
                    ? tr("关闭自动检查更新", "Disable auto update check")
                    : tr("开启自动检查更新", "Enable auto update check")
                }
              >
                <span
                  className={`absolute top-[3px] w-4 h-4 rounded-full bg-white shadow transition-all ${
                    autoUpdate ? "left-[21px]" : "left-[3px]"
                  }`}
                />
              </button>
            )}
            <span className="text-[11.5px] text-[var(--ink-3)]">
              {tr(
                "启动时自动检测 GitHub 上的新版本",
                "Automatically check GitHub releases on startup"
              )}
            </span>
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            {tr(
              "发现新版本时会展示更新说明，确认后自动下载并原地安装（不改变安装位置），完成后自动重启应用。",
              "Shows release notes when a new version is found, downloads and installs in-place upon confirmation, then restarts automatically."
            )}
          </p>
        </section>

        <div className="text-[11.5px] text-[var(--ink-3)] px-1 pb-2">
          RustMusic {appVersion ? `v${appVersion}` : ""} · Rust + Tauri 2 + React ·{" "}
          {tr(
            "引擎 rodio / symphonia · 界面支持 Windows / Linux（WASAPI 独占输出仅 Windows）",
            "Engine: rodio / symphonia · Windows / Linux (WASAPI Exclusive on Windows only)"
          )}
        </div>
      </div>
    </div>
  );
}

/** 垂直滑块（均衡器用：普通圆滑轨 / 拟物调音台金属推子） */
function VSlider({
  value,
  min,
  max,
  isSkeuo,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  isSkeuo?: boolean;
  onChange: (v: number) => void;
}) {
  const H = 120;
  const zero = ((0 - min) / (max - min)) * H;
  const y = ((value - min) / (max - min)) * H;

  if (isSkeuo) {
    return (
      <div
        className="relative w-6 rounded-full cursor-pointer touch-none flex justify-center"
        style={{
          height: H,
          background: "var(--sk-groove-bg)",
          border: "1px solid var(--sk-groove-border)",
          boxShadow: "var(--sk-groove-shadow)",
        }}
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
        {/* 零点刻度参考线 */}
        <div
          className="absolute w-4 h-px bg-white/20 pointer-events-none"
          style={{ top: zero }}
        />
        {/* 增益/衰减发光柱 */}
        <div
          className="absolute w-2 rounded-full"
          style={{
            top: Math.min(zero, y),
            height: Math.max(2, Math.abs(zero - y)),
            background:
              value >= 0
                ? "linear-gradient(180deg, var(--accent-strong), var(--accent))"
                : "linear-gradient(180deg, #d97706, #b45309)",
            boxShadow: "0 0 6px var(--accent-soft)",
          }}
        />
        {/* 拟物车削金属调音台推子钮 */}
        <div
          className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 w-6 h-3 rounded-[3px] flex items-center justify-center transition-transform"
          style={{
            top: y,
            background: "var(--sk-knob-metal)",
            border: "1px solid var(--sk-edge)",
            boxShadow: "var(--sk-knob-shadow)",
          }}
        >
          <span className="w-3.5 h-[1.5px] bg-red-600 shadow-[0_0_2px_red]" />
        </div>
      </div>
    );
  }

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

/** 三色取色器组 */
function LyricColorPickers({
  values,
  defaults,
  onChange,
}: {
  values: { sung: string; unsung: string; next: string };
  defaults?: { sung: string; unsung: string; next: string };
  onChange: (key: "sung" | "unsung" | "next", v: string) => void;
}) {
  const { tr } = useI18n();
  return (
    <div className="flex items-center gap-5">
      {(
        [
          { key: "sung", label: tr("已唱", "Sung") },
          { key: "unsung", label: tr("未唱", "Unsung") },
          { key: "next", label: tr("下一句", "Next") },
        ] as const
      ).map((c) => {
        const raw = values[c.key] || defaults?.[c.key] || "#ffffff";
        const m = raw.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
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
            title={tr(`${c.label}颜色（点击选择）`, `${c.label} color (click to pick)`)}
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
              onChange={(e) => onChange(c.key, e.target.value)}
              className="w-0 h-0 opacity-0"
            />
          </label>
        );
      })}
    </div>
  );
}
