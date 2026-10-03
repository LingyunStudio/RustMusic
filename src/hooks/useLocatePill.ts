import { useCallback, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";

/**
 * 「回到当前歌曲」药丸的可见性与定位动作（配合 useVirtualWindow 使用）。
 *
 * 虚拟窗口列表行高恒定：行 i 是否在可视区用数学判断即可，不依赖行 DOM
 * （滚出可视区的行在窗口化下本来就没挂载）。滚动监听 rAF 节流，
 * show 值有变化才 setState——滚动高频路径上零重渲染。
 */
export function useLocatePill(opts: {
  containerRef: RefObject<HTMLDivElement | null>;
  rowHeight: number;
  /** 当前播放行的下标；-1 = 列表无当前曲，药丸永不显示 */
  currentIndex: number;
}) {
  const { containerRef, rowHeight, currentIndex } = opts;
  const [show, setShow] = useState(false);
  const showRef = useRef(false);
  const rafRef = useRef(0);

  const update = useCallback(() => {
    const el = containerRef.current;
    if (!el || currentIndex < 0) {
      // 无当前行/容器：药丸隐藏
      if (showRef.current) {
        showRef.current = false;
        setShow(false);
      }
      return;
    }
    const rowTop = currentIndex * rowHeight;
    const rowBottom = rowTop + rowHeight;
    // 行与可视区是否相交（±半行容差吸收容器内边距的数学偏差）
    const visible =
      rowTop < el.scrollTop + el.clientHeight + rowHeight / 2 &&
      rowBottom > el.scrollTop - rowHeight / 2;
    // 药丸在行**不可见**时显示——此前漏了取反，可见性恰好写反
    const next = !visible;
    if (next !== showRef.current) {
      showRef.current = next;
      setShow(next);
    }
  }, [containerRef, rowHeight, currentIndex]);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    update();
    const onScroll = () => {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = requestAnimationFrame(update);
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    const ro = new ResizeObserver(onScroll);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro.disconnect();
      cancelAnimationFrame(rafRef.current);
    };
  }, [containerRef, update]);

  const locate = useCallback(() => {
    const el = containerRef.current;
    if (!el || currentIndex < 0) return;
    el.scrollTo({
      top: Math.max(
        0,
        currentIndex * rowHeight - el.clientHeight / 2 + rowHeight / 2
      ),
      behavior: "smooth",
    });
  }, [containerRef, rowHeight, currentIndex]);

  return { show, locate };
}
