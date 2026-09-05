import { useState } from "react";
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

const EQ_FREQS = ["31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];

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
  const clearCache = useStore((s) => s.clearCache);
  const [preset, setPreset] = useState("平直");

  const setBand = (i: number, v: number) => {
    const g = [...eqGains];
    g[i] = v;
    setEq(g, eqEnabled);
    setPreset("自定义");
  };

  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-5 py-5 pb-8">
      <h1 className="text-[22px] font-bold flex items-center gap-2.5 mb-5">
        <SettingsIcon size={20} className="text-[var(--dyn)]" />
        设置
      </h1>

      <div className="flex flex-col gap-4 max-w-[760px]">
        {/* 音乐文件夹 */}
        <section className="glass rounded-2xl p-5">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-[14.5px] font-semibold">音乐文件夹</h2>
            <div className="flex items-center gap-2">
              {scan.active && (
                <span className="text-[12px] text-[var(--dyn)] flex items-center gap-1.5">
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
                  className="group flex items-center gap-2.5 h-10 px-3 rounded-lg bg-white/[0.03] hover:bg-white/[0.06] transition-colors"
                >
                  <Folder size={14} className="text-zinc-500 shrink-0" />
                  <span className="text-[12.5px] text-zinc-300 truncate">{f.path}</span>
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
            <div className="text-[12.5px] text-zinc-500 py-2">
              还没有添加文件夹。也可以直接把文件 / 文件夹拖进窗口。
            </div>
          )}
        </section>

        {/* 均衡器 */}
        <section className="glass rounded-2xl p-5">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-[14.5px] font-semibold">
              均衡器
              <span className="ml-2 text-[11.5px] font-normal text-zinc-500">10 段</span>
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
                      ? "bg-white/[0.12] text-white"
                      : "text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.05]"
                  }`}
                >
                  {name}
                </button>
              ))}
              <button
                className={`ml-2 relative w-10 h-[22px] rounded-full transition-colors ${
                  eqEnabled ? "bg-[var(--dyn)]" : "bg-white/[0.12]"
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
                <span className="text-[10.5px] text-zinc-500 tabular-nums">
                  {eqGains[i] > 0 ? "+" : ""}
                  {eqGains[i].toFixed(0)}
                </span>
                <VSlider
                  value={eqGains[i]}
                  min={-12}
                  max={12}
                  onChange={(v) => setBand(i, Math.round(v))}
                />
                <span className="text-[10.5px] text-zinc-600">{label}</span>
              </div>
            ))}
          </div>
        </section>

        {/* 播放 */}
        <section className="glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-4">播放</h2>
          <div className="flex items-center gap-4">
            <span className="text-[12.5px] text-zinc-400 w-[80px]">播放速度</span>
            <input
              type="range"
              min={0.5}
              max={2}
              step={0.05}
              value={speed}
              onChange={(e) => setSpeed(parseFloat(e.target.value))}
              className="flex-1 accent-indigo-400"
            />
            <span className="text-[12px] text-zinc-300 tabular-nums w-10 text-right">
              {speed.toFixed(2)}x
            </span>
          </div>
          <p className="text-[11.5px] text-zinc-600 mt-2">
            变速通过重采样实现，音调会随之变化；均衡器实时作用于所有播放。
          </p>
        </section>

        {/* 缓存 */}
        <section className="glass rounded-2xl p-5">
          <h2 className="text-[14.5px] font-semibold mb-2">缓存</h2>
          <p className="text-[12px] text-zinc-500 mb-3">
            在线音源下载后的缓存文件存放在应用数据目录的 downloads 文件夹。
          </p>
          <button className="btn-secondary !text-rose-300/80 hover:!bg-rose-500/15" onClick={clearCache}>
            <Trash2 size={13} />
            清理音源缓存
          </button>
        </section>

        <div className="text-[11.5px] text-zinc-600 px-1 pb-2">
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
      className="relative w-7 rounded-full bg-white/[0.07] cursor-pointer touch-none"
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
              ? "linear-gradient(180deg, var(--dyn), #a5b4fc)"
              : "linear-gradient(180deg, #67e8f9, #22d3ee)",
        }}
      />
      <div
        className="absolute left-1/2 -translate-x-1/2 -translate-y-1/2 w-5 h-2.5 rounded-full bg-white shadow"
        style={{ top: y }}
      />
    </div>
  );
}
