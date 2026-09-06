import { useEffect, useState } from "react";
import {
  Folder,
  FolderPlus,
  Loader2,
  RefreshCw,
  Settings as SettingsIcon,
  Trash2,
  X,
} from "lucide-react";
import { useStore } from "../store";
import { api } from "../api";
import { ACCENTS } from "../theme";

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
  const theme = useStore((s) => s.theme);
  const setTheme = useStore((s) => s.setTheme);
  const accent = useStore((s) => s.accent);
  const setAccent = useStore((s) => s.setAccent);
  const clearCache = useStore((s) => s.clearCache);
  const [saveDir, setSaveDir] = useState("");
  const [saveDirDefault, setSaveDirDefault] = useState("");
  const [preset, setPreset] = useState("平直");

  useEffect(() => {
    api.saveDirGet().then((d) => {
      setSaveDir(d.dir);
      setSaveDirDefault(d.default);
    });
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

  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-5 pt-5 pb-[92px]">
      <h1 className="text-[22px] font-bold flex items-center gap-2.5 mb-5">
        <SettingsIcon size={20} className="text-[var(--accent)]" />
        设置
      </h1>

      <div className="flex flex-col gap-4 max-w-[760px]">
        {/* 外观 */}
        <section className="glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-4">外观</h2>
          <div className="flex items-center gap-4 mb-4">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[80px]">界面模式</span>
            <div className="flex items-center gap-1.5">
              {(
                [
                  { key: "dark", label: "深色" },
                  { key: "light", label: "浅色" },
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
            </div>
          </div>
        </section>

        {/* 音乐文件夹 */}
        <section className="glass rounded-2xl p-5">
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
                    className="btn-ghost w-7 h-7 ml-auto shrink-0 opacity-0 group-hover:opacity-100 hover:!text-rose-400"
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
        <section className="glass rounded-2xl p-5">
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
        <section className="glass rounded-2xl p-5">
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
                      ? "bg-[rgba(240,162,74,0.14)] text-[var(--accent-strong)] font-medium"
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
              className="flex-1 accent-amber-400"
            />
            <span className="text-[12px] text-[var(--ink)] tabular-nums w-10 text-right">
              {speed.toFixed(2)}x
            </span>
          </div>
          <p className="text-[11.5px] text-[var(--ink-3)] mt-2">
            变速通过重采样实现，音调会随之变化；均衡器实时作用于所有播放。
          </p>
        </section>

        {/* 下载目录 */}
        <section className="glass rounded-2xl p-5">
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
        <section className="glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-2">缓存</h2>
          <p className="text-[12px] text-[var(--ink-2)] mb-3">
            在线音源下载后的缓存文件存放在应用数据目录的 downloads 文件夹。
          </p>
          <button className="btn-secondary !text-rose-300/80 hover:!bg-rose-500/15" onClick={clearCache}>
            <Trash2 size={13} />
            清理音源缓存
          </button>
        </section>

        <div className="text-[11.5px] text-[var(--ink-3)] px-1 pb-2">
          RustMusic v0.1.0 · Rust + Tauri 2 + React · 引擎 rodio / symphonia ·
          界面仅支持 Windows（架构上保留跨平台能力）
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
