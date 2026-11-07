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
      {/* 顶部拖拽热区 */}
      <div
        data-tauri-drag-region
        className="absolute top-0 left-0 right-0 h-9 z-[54] pointer-events-auto"
      />

      {/* 拟物金属按键窗口控制区 */}
      <div className="absolute top-1.5 right-3 flex items-center gap-1.5 h-7 z-[56]">
        <button
          className="titlebar-btn"
          onClick={() => win.minimize()}
          title="最小化"
        >
          <Minus size={12} />
        </button>
        <button
          className="titlebar-btn"
          onClick={() => win.toggleMaximize()}
          title="最大化 / 还原"
        >
          {maximized ? <Copy size={11} className="-scale-x-100" /> : <Square size={10} />}
        </button>
        <button
          className="titlebar-btn close-btn"
          onClick={() => win.close()}
          title="关闭"
        >
          <X size={13} />
        </button>
      </div>
    </>
  );
}
