import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X, Copy } from "lucide-react";
import { useEffect, useState } from "react";

export default function Titlebar() {
  const [maximized, setMaximized] = useState(false);
  const win = getCurrentWindow();

  useEffect(() => {
    let disposed = false;
    let un: (() => void) | undefined;
    const p = win.onResized(async () => setMaximized(await win.isMaximized()));
    p.then((u) => {
      if (disposed) u();
      else un = u;
    });
    return () => {
      disposed = true;
      un?.();
    };
  }, []);

  return (
    <>
      {/* 顶部拖拽热区：高度与机架倒角由各界面风格的 CSS 控制（.titlebar-drag） */}
      <div
        data-tauri-drag-region
        className="titlebar-drag absolute top-0 left-0 right-0 z-[54] pointer-events-auto"
      />

      {/* 窗口控制按钮：无边框容器的幽灵按钮组（与全局 hover 语言一致），
          顶/右缘与内容卡片对齐（8px 外框） */}
      <div className="absolute top-2 right-2 flex items-center gap-0.5 h-8 z-[56] px-1">
        <button
          className="titlebar-btn w-8 h-7 rounded-lg flex items-center justify-center text-[var(--ink-2)] hover:bg-[var(--shade-strong)] hover:text-[var(--ink)] active:scale-90 transition-all"
          onClick={() => win.minimize()}
          title="最小化"
        >
          <Minus size={14} />
        </button>
        <button
          className="titlebar-btn w-8 h-7 rounded-lg flex items-center justify-center text-[var(--ink-2)] hover:bg-[var(--shade-strong)] hover:text-[var(--ink)] active:scale-90 transition-all"
          onClick={() => win.toggleMaximize()}
          title={maximized ? "向下还原" : "最大化"}
        >
          {maximized ? <Copy size={12} className="-scale-x-100" /> : <Square size={11} />}
        </button>
        <button
          className="titlebar-btn close-btn w-8 h-7 rounded-lg flex items-center justify-center text-[var(--ink-2)] hover:bg-[#e04a3f] hover:text-white active:scale-90 transition-all"
          onClick={() => win.close()}
          title="关闭"
        >
          <X size={13} />
        </button>
      </div>
    </>
  );
}
