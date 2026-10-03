import { useCallback, useLayoutEffect, useRef, useState } from "react";

/**
 * 固定行高列表的窗口化（只渲染可视区 ± overscan 行）。
 *
 * 大曲库下整表全量渲染会让 DOM / 布局对象 / 封面解码缓存随曲库线性增长
 * （渲染进程内存的主要来源），这里把渲染量压成常数。行高必须恒定：
 * - TrackList 行 h-[64px]、PlaylistDetail/NeteaseView 行 h-[60px]、QueuePanel 行 h-12(48px)
 * - 上下内边距（pt-2.5 等）造成的整体偏移 ≤ 一行，由 overscan 吸收
 *
 * 用法：containerRef + onScroll 挂到现有的 overflow-y-auto 容器上，
 * 渲染 spacer(start*rowHeight) + slice(start,end) + spacer((count-end)*rowHeight)。
 */
export function useVirtualWindow(count: number, rowHeight: number, overscan = 8) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [range, setRange] = useState({ start: 0, end: 0 });
  const rangeRef = useRef(range);
  const rafRef = useRef(0);

  const measure = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    if (count === 0) {
      // 清空列表必须归零：保留旧 range 会留下 start×rowHeight 的空白
      // spacer，把空态提示顶出可视区（count=0 后不再有滚动事件可自愈）
      if (rangeRef.current.start !== 0 || rangeRef.current.end !== 0) {
        rangeRef.current = { start: 0, end: 0 };
        setRange({ start: 0, end: 0 });
      }
      return;
    }
    const start = Math.max(0, Math.floor(el.scrollTop / rowHeight) - overscan);
    const end = Math.min(
      count,
      Math.ceil((el.scrollTop + el.clientHeight) / rowHeight) + overscan
    );
    const cur = rangeRef.current;
    if (cur.start !== start || cur.end !== end) {
      const next = { start, end };
      rangeRef.current = next;
      setRange(next);
    }
  }, [count, rowHeight, overscan]);

  // 滚动重算 rAF 节流：一帧至多一次 setState
  const onScroll = useCallback(() => {
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(measure);
  }, [measure]);

  // 定位到第 i 行（垂直居中）：打开播放队列/歌单/歌曲列表时跳到当前播放曲。
  // 数学定位对窗口化与全量渲染都成立（行高恒定约定见文件头）；
  // 容器内边距造成的偏差 ≤ 一个 padding，居中视觉下可忽略
  const scrollToIndex = useCallback(
    (i: number) => {
      const el = containerRef.current;
      if (!el || i < 0) return;
      el.scrollTop = Math.max(
        0,
        i * rowHeight - el.clientHeight / 2 + rowHeight / 2
      );
      measure();
    },
    [rowHeight, measure]
  );

  useLayoutEffect(() => {
    measure();
    const el = containerRef.current;
    if (!el) return;
    // 容器尺寸变化（窗口缩放/列表显隐）时重算可视范围
    const ro = new ResizeObserver(() => measure());
    ro.observe(el);
    return () => {
      ro.disconnect();
      cancelAnimationFrame(rafRef.current);
    };
  }, [measure]);

  return { containerRef, start: range.start, end: range.end, onScroll, scrollToIndex };
}
