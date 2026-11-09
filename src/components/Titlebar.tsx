import { useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Minus, Square, Copy, X } from "lucide-react";
import { useStore } from "../store";

const win = getCurrentWebviewWindow();

export default function Titlebar() {
  const [maximized, setMaximized] = useState(false);
  const uiTheme = useStore((s) => s.uiTheme);
  const isLiquid = uiTheme === "liquid";
  const isSkeuo = uiTheme === "skeuo";

  useEffect(() => {
    let disposed = false;
    win.isMaximized().then((m) => {
      if (!disposed) setMaximized(m);
    });
    let un: (() => void) | undefined;
    const p = win.onResized(async () => setMaximized(await win.isMaximized()));
    p.then((u) => {
      // 组件可能已在 promise resolve 前卸载（StrictMode 双挂载），避免泄漏监听器
      if (disposed) u();
      else un = u;
    });
    return () => {
      disposed = true;
      if (un) un();
    };
  }, []);

  if (isLiquid) {
    return (
      <>
        {/* 顶部拖拽热区：24px 高，恰好止于头部首行（面板顶 8 + 内边距 16），不侵入内容可点击区 */}
        <div
          data-tauri-drag-region
          className="absolute top-0 left-0 right-0 h-6 z-[54] pointer-events-auto"
        />

        {/* 窗口控制按钮：无边框容器的幽灵按钮组（与全局 hover 语言一致），顶/右缘与内容卡片对齐（8px 外框） */}
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
            className="titlebar-btn w-8 h-7 rounded-lg flex items-center justify-center text-[var(--ink-2)] hover:bg-[#e04a3f] hover:text-white active:scale-90 transition-all"
            onClick={() => win.close()}
            title="关闭"
          >
            <X size={15} />
          </button>
        </div>
      </>
    );
  }

  if (isSkeuo) {
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

  return (
    <>
      {/* 顶部拖拽热区：扩大到 36px 高，向下压在内容区顶部的留白（各视图 pt-7）上 */}
      <div
        data-tauri-drag-region
        className="absolute top-0 left-0 right-0 h-9 z-[54] pointer-events-auto"
      />

      {/* 窗口控制按钮（右上角悬浮，压在热区之上、内容区右上角留白处） */}
      <div className="absolute top-0 right-0 flex items-center h-9 z-[56]">
        <button
          className="titlebar-btn w-11 h-9 flex items-center justify-center text-[var(--ink-2)] hover:bg-[var(--shade)] hover:text-[var(--ink)] transition-colors"
          onClick={() => win.minimize()}
        >
          <Minus size={14} />
        </button>
        <button
          className="titlebar-btn w-11 h-9 flex items-center justify-center text-[var(--ink-2)] hover:bg-[var(--shade)] hover:text-[var(--ink)] transition-colors"
          onClick={() => win.toggleMaximize()}
        >
          {maximized ? <Copy size={12} className="-scale-x-100" /> : <Square size={11} />}
        </button>
        <button
          className="titlebar-btn w-11 h-9 flex items-center justify-center text-[var(--ink-2)] hover:bg-[#c73e2e] hover:text-[var(--ink)] transition-colors"
          onClick={() => win.close()}
        >
          <X size={15} />
        </button>
      </div>
    </>
  );
}
