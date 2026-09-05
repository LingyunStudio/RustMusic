import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X, Copy } from "lucide-react";
import { useEffect, useState } from "react";
import { useStore } from "../store";

export default function Titlebar() {
  const [maximized, setMaximized] = useState(false);
  const search = useStore((s) => s.search);
  const setSearch = useStore((s) => s.setSearch);
  const win = getCurrentWindow();

  useEffect(() => {
    const p = win.onResized(async () => setMaximized(await win.isMaximized()));
    let un: (() => void) | undefined;
    p.then((u) => (un = u));
    return () => un?.();
  }, []);

  return (
    <div
      data-tauri-drag-region
      className="h-11 flex items-center justify-between pl-4 pr-1 relative z-30 shrink-0"
    >
      <div data-tauri-drag-region className="flex items-center gap-2.5">
        <div
          className="w-6 h-6 rounded-lg flex items-center justify-center shadow-lg"
          style={{ background: "linear-gradient(135deg, #6366f1, #22d3ee)" }}
        >
          <div
            className="ml-[1px]"
            style={{
              width: 0,
              height: 0,
              borderTop: "5px solid transparent",
              borderBottom: "5px solid transparent",
              borderLeft: "8px solid white",
            }}
          />
        </div>
        <span data-tauri-drag-region className="font-semibold text-[13px] tracking-wide text-zinc-200">
          RustMusic
        </span>
      </div>

      <div className="absolute left-1/2 -translate-x-1/2 w-[340px]">
        <div className="relative">
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索歌曲、艺术家、专辑…"
            className="w-full h-8 rounded-full bg-white/[0.06] border border-white/[0.08] pl-8 pr-8 text-[12.5px] text-zinc-200 placeholder:text-zinc-500 focus:bg-white/[0.09] focus:border-white/20 transition-colors"
          />
          <svg
            className="absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-500"
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.2"
          >
            <circle cx="11" cy="11" r="7" />
            <path d="m20 20-3.5-3.5" />
          </svg>
          {search && (
            <button
              className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-zinc-200"
              onClick={() => setSearch("")}
            >
              <X size={13} />
            </button>
          )}
        </div>
      </div>

      <div className="flex items-center h-8">
        <button
          className="w-11 h-8 flex items-center justify-center text-zinc-400 hover:bg-white/[0.08] hover:text-white transition-colors rounded-md"
          onClick={() => win.minimize()}
        >
          <Minus size={14} />
        </button>
        <button
          className="w-11 h-8 flex items-center justify-center text-zinc-400 hover:bg-white/[0.08] hover:text-white transition-colors rounded-md"
          onClick={() => win.toggleMaximize()}
        >
          {maximized ? <Copy size={12} className="-scale-x-100" /> : <Square size={11.5} />}
        </button>
        <button
          className="w-11 h-8 flex items-center justify-center text-zinc-400 hover:bg-[#e81123] hover:text-white transition-colors rounded-md"
          onClick={() => win.close()}
        >
          <X size={15} />
        </button>
      </div>
    </div>
  );
}
