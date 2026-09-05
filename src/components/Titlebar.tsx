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
      className="h-12 flex items-center justify-between pl-6 pr-0 relative z-30 shrink-0"
    >
      <div data-tauri-drag-region className="w-[220px]" />

      <div data-tauri-drag-region className="w-[360px]">
        <div className="relative">
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索歌曲、艺术家、专辑…"
            className="w-full h-9 rounded-full bg-black/25 border border-[var(--line)] pl-10 pr-9 text-[12.5px] text-[var(--ink)] placeholder:text-[var(--ink-3)] focus:bg-black/35 focus:border-[rgba(243,233,216,0.22)] transition-colors"
          />
          <svg
            className="absolute left-3.5 top-1/2 -translate-y-1/2 text-[var(--ink-3)]"
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
              className="absolute right-3 top-1/2 -translate-y-1/2 text-[var(--ink-3)] hover:text-[var(--ink)]"
              onClick={() => setSearch("")}
            >
              <X size={13} />
            </button>
          )}
        </div>
      </div>

      <div className="flex items-center h-12">
        <button
          className="w-12 h-12 flex items-center justify-center text-[var(--ink-2)] hover:bg-white/[0.07] hover:text-[var(--ink)] transition-colors"
          onClick={() => win.minimize()}
        >
          <Minus size={15} />
        </button>
        <button
          className="w-12 h-12 flex items-center justify-center text-[var(--ink-2)] hover:bg-white/[0.07] hover:text-[var(--ink)] transition-colors"
          onClick={() => win.toggleMaximize()}
        >
          {maximized ? <Copy size={12.5} className="-scale-x-100" /> : <Square size={12} />}
        </button>
        <button
          className="w-12 h-12 flex items-center justify-center text-[var(--ink-2)] hover:bg-[#c73e2e] hover:text-white transition-colors"
          onClick={() => win.close()}
        >
          <X size={16} />
        </button>
      </div>
    </div>
  );
}
