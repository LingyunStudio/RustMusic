import { useCallback, useEffect, useRef, useState } from "react";
import type { RefObject } from "react";

/**
 * 「回到当前歌曲」药丸——元素定位版：非虚拟窗口、行高不恒定（卡片网格）
 * 的列表用。当前播放行以 data-now-playing="1" 标记（selector 可配），
 * 滚动时检查该元素是否与容器可视区相交，滚出即显示药丸；点击平滑滚回。
 * 列表须全量渲染（元素常驻 DOM）；虚拟窗口列表请用 useLocatePill（数学版）。
 */
export function useLocatePillEl(opts: {
  containerRef: RefObject<HTMLElement | null>;
  /** 当前行元素 selector；null/空 = 列表无当前行，药丸不显示 */
  activeSelector: string | null;
}) {
  const { containerRef, activeSelector } = opts;
  const [show, setShow] = useState(false);
  const showRef = useRef(false);
  const rafRef = useRef(0);

  const update = useCallback(() => {
    const el = containerRef.current;
    let next = false;
    if (el && activeSelector) {
      const card = el.querySelector(activeSelector);
      if (card) {
        const r = card.getBoundingClientRect();
        const c = el.getBoundingClientRect();
        // 元素与容器可视区相交即视为可见（上下各 ±1/4 容器高的容差）
        next = !(
          r.bottom < c.top - c.height * 0.25 || r.top > c.bottom + c.height * 0.25
        );
      }
    }
    if (next !== showRef.current) {
      showRef.current = next;
      setShow(next);
    }
  }, [containerRef, activeSelector]);

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
    if (!el || !activeSelector) return;
    el.querySelector(activeSelector)?.scrollIntoView({
      block: "center",
      behavior: "smooth",
    });
  }, [containerRef, activeSelector]);

  return { show, locate };
}
