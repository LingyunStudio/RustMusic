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
      // 组件可能已在 promise resolve 前卸载（StrictMode 双挂载），避免泄漏监听器
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
      {/* 顶部拖拽热区：扩大到 36px 高，向下压在内容区顶部的留白（各视图 pt-7）上。
          绝对定位不参与布局，因此内容组件位置完全不变；data-tauri-drag-region
          使空白处可拖动窗口。 */}
      <div
        data-tauri-drag-region
        className="absolute top-0 left-0 right-0 h-9 z-[54] pointer-events-auto"
      />

      {/* 窗口控制按钮（右上角悬浮液态玻璃胶囊） */}
      <div className="absolute top-2 right-3 flex items-center gap-0.5 h-8 z-[56] px-1.5 py-0.5 rounded-full liquid-dock shadow-md">
        <button
          className="titlebar-btn w-8 h-7 rounded-full flex items-center justify-center text-[var(--ink-2)] hover:bg-[var(--shade-strong)] hover:text-[var(--ink)] active:scale-90 transition-all"
          onClick={() => win.minimize()}
          title="最小化"
        >
          <Minus size={13} />
        </button>
        <button
          className="titlebar-btn w-8 h-7 rounded-full flex items-center justify-center text-[var(--ink-2)] hover:bg-[var(--shade-strong)] hover:text-[var(--ink)] active:scale-90 transition-all"
          onClick={() => win.toggleMaximize()}
          title={maximized ? "向下还原" : "最大化"}
        >
          {maximized ? <Copy size={11} className="-scale-x-100" /> : <Square size={10} />}
        </button>
        <button
          className="titlebar-btn w-8 h-7 rounded-full flex items-center justify-center text-[var(--ink-2)] hover:bg-[#e04a3f]/80 hover:text-white active:scale-90 transition-all"
          onClick={() => win.close()}
          title="关闭"
        >
          <X size={14} />
        </button>
      </div>
    </>
  );
}
