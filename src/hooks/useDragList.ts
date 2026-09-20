import { useCallback, useEffect, useRef } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";

/**
 * 列表行“鼠标左键长按拖动排序”。
 *
 * 交互：
 * - 按下不动 ≥ HOLD_MS 进入拖拽；期间移动超 SLOP_PX 视为普通点击/滚动，取消
 * - 拖拽中被拖行 1:1 跟随指针（滚动时贴住指针），其余行平移让位；
 *   指针靠近滚动容器上下边缘时自动滚动
 * - 松手：被拖行动画滑入目标槽位，动画结束的同一帧清样式并回调
 *   onSubmit(from, to)——要求回调触发的重渲染同步发生（store 乐观更新），
 *   新顺序的自然位置与拖拽视觉一致，无跳变
 * - ESC / 窗口失焦取消（不提交）；拖拽结束后一小段时间内的 click/dblclick
 *   吞掉（防止误触行内按钮与“双击播放”）
 *
 * 渲染完全不经过 React（全部直接写 DOM）。行必须是指针所在滚动容器的
 * 直接子元素（行序列 = container.children 顺序）。
 */

const HOLD_MS = 250;
const SLOP_PX = 6;
const SHIFT_MS = 170; // 让位行平移过渡
const SETTLE_MS = 150; // 松手滑入槽位 / 取消归位的过渡
const CLICK_QUIET_MS = 400; // 拖拽结束后吞 click/dblclick 的窗口
const EDGE = 48; // 距滚动容器上下边缘多少 px 内开始自动滚动
const MAX_SCROLL = 14; // 自动滚动每帧最大像素

export function useDragList(onSubmit: (from: number, to: number) => void) {
  const submitRef = useRef(onSubmit);
  submitRef.current = onSubmit;
  /** 拖拽总开关（调用方每次渲染更新；false 时长按不进入拖拽） */
  const enabledRef = useRef(true);

  const st = useRef({
    pressedIndex: -1,
    pressedRow: null as HTMLElement | null,
    dragging: false,
    from: -1,
    to: -1,
    startY: 0,
    pointerY: 0,
    scroll0: 0,
    scrollDir: 0,
    container: null as HTMLElement | null,
    rows: [] as HTMLElement[],
    tops: [] as number[],
    rowH: 48,
    holdTimer: null as ReturnType<typeof setTimeout> | null,
    settleTimer: null as ReturnType<typeof setTimeout> | null,
    raf: 0,
  }).current;
  const dragEndAt = useRef(-1e9);
  const beginRef = useRef<() => void>(() => {});

  useEffect(() => {
    const clearRows = () => {
      if (st.dragging) return; // 新一轮拖拽已开始，样式归它管
      for (const r of st.rows) {
        r.style.transform = "";
        r.style.transition = "";
        r.style.pointerEvents = "";
        r.classList.remove("drag-row-ghost");
      }
      document.body.classList.remove("dragging-rows");
    };

    // 松手提交 / 取消归位。submit=false 时只做视觉复位不回调。
    const finish = (submit: boolean) => {
      st.dragging = false;
      st.pressedIndex = -1;
      st.pressedRow = null;
      st.scrollDir = 0;
      cancelAnimationFrame(st.raf);
      dragEndAt.current = performance.now();
      const from = st.from;
      const to = st.to;
      const g = st.rows[from];
      if (!g) {
        clearRows();
        return;
      }
      if (submit && to !== from) {
        // 滑入目标槽位
        g.style.transition = `transform ${SETTLE_MS}ms cubic-bezier(0.2, 0.8, 0.2, 1)`;
        g.style.transform = `translateY(${st.tops[to] - st.tops[from]}px)`;
      } else {
        // 原地松手 / ESC 取消：被拖行与让位行都归位
        g.style.transition = `transform ${SETTLE_MS}ms ease`;
        g.style.transform = "";
        for (let i = 0; i < st.rows.length; i++) {
          if (i !== from) st.rows[i].style.transform = "";
        }
      }
      if (st.settleTimer) clearTimeout(st.settleTimer);
      st.settleTimer = setTimeout(() => {
        // 清样式 + 提交在同一任务里：onSubmit 走 store 乐观更新同步重渲染，
        // 新 DOM 顺序即视觉顺序，中间不会闪回旧顺序
        clearRows();
        if (submit && to !== from) submitRef.current(from, to);
      }, SETTLE_MS);
    };

    // 每帧：跟随指针 + 槽位计算 + 让位平移（幂等重写）+ 边缘自动滚动
    const frame = () => {
      st.raf = requestAnimationFrame(frame);
      const c = st.container;
      if (!c || !st.dragging) return;
      if (st.scrollDir !== 0) c.scrollTop += st.scrollDir;
      // 容器滚动时内容整体位移，被拖行要贴住指针：补偿滚动量。
      // 幽灵行的“内容坐标”位置 = 自然位置 + ghostDy，槽位按它计算
      const ghostDy =
        st.pointerY - st.startY + (c.scrollTop - st.scroll0);
      const g = st.rows[st.from];
      if (g) g.style.transform = `translateY(${ghostDy}px)`;
      const center = st.tops[st.from] + st.rowH / 2 + ghostDy;
      let to = Math.floor((center - st.tops[0]) / st.rowH);
      to = Math.max(0, Math.min(st.rows.length - 1, to));
      st.to = to;
      for (let i = 0; i < st.rows.length; i++) {
        if (i === st.from) continue;
        let shift = 0;
        if (st.from < to && i > st.from && i <= to) shift = -st.rowH;
        else if (to < st.from && i >= to && i < st.from) shift = st.rowH;
        st.rows[i].style.transform = shift ? `translateY(${shift}px)` : "";
      }
    };

    const begin = () => {
      const row = st.pressedRow;
      if (!row || st.pressedIndex < 0 || st.dragging) return;
      if (!enabledRef.current) return; // 该列表未开启拖拽（非手动排序视图等）
      const container = row.parentElement;
      if (!container) return;
      // 行 = 容器直接子元素中未标记 data-drag-skip 的部分：虚拟窗口的
      // 上下占位 div 也是子元素，不打标记会把行索引整体顶偏（拖错行）
      const rows = (Array.from(container.children) as HTMLElement[]).filter(
        (el) => !el.hasAttribute("data-drag-skip")
      );
      const tops = rows.map((r) => r.offsetTop);
      const rowH = tops.length > 1 ? tops[1] - tops[0] : row.offsetHeight;
      if (rowH <= 0 || !tops.length) return;
      st.rows = rows;
      st.tops = tops;
      st.rowH = rowH;
      st.container = container;
      st.from = st.pressedIndex;
      st.to = st.pressedIndex;
      st.scroll0 = container.scrollTop;
      st.dragging = true;
      document.body.classList.add("dragging-rows");
      for (const r of rows) {
        // 入场动画必须禁用：rowIn 的 delay 绑定 --row-idx，重排后 delay
        // 变化会让已完成的动画倒放（整列表重新入场闪一遍）
        r.style.animation = "none";
        // 拖拽期间禁 hover（防闪烁）；被拖行恢复交互跟随指针
        r.style.pointerEvents = "none";
        r.style.transition = `transform ${SHIFT_MS}ms cubic-bezier(0.2, 0.8, 0.2, 1)`;
      }
      row.style.pointerEvents = "";
      row.style.transition = "none"; // 被拖行 1:1 跟随，不平滑
      row.classList.add("drag-row-ghost");
      st.raf = requestAnimationFrame(frame);
    };
    beginRef.current = begin;

    const onMove = (e: MouseEvent) => {
      st.pointerY = e.clientY;
      if (st.dragging) {
        e.preventDefault();
        const c = st.container;
        if (c) {
          const r = c.getBoundingClientRect();
          const overTop = e.clientY - r.top;
          const overBottom = r.bottom - e.clientY;
          st.scrollDir =
            overTop < EDGE
              ? -Math.max(2, Math.min(MAX_SCROLL, (EDGE - overTop) / 3))
              : overBottom < EDGE
                ? Math.max(2, Math.min(MAX_SCROLL, (EDGE - overBottom) / 3))
                : 0;
        }
        return;
      }
      // 未到长按时限就大幅移动：判定为点击/滚动，取消拖拽候选
      if (st.pressedIndex >= 0 && Math.abs(e.clientY - st.startY) > SLOP_PX) {
        if (st.holdTimer) clearTimeout(st.holdTimer);
        st.holdTimer = null;
        st.pressedIndex = -1;
        st.pressedRow = null;
      }
    };

    const onUp = () => {
      if (st.holdTimer) {
        clearTimeout(st.holdTimer);
        st.holdTimer = null;
      }
      if (st.dragging) finish(true);
      st.pressedIndex = -1;
      st.pressedRow = null;
    };

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && st.dragging) finish(false);
    };
    const onBlur = () => {
      if (st.dragging) finish(false);
    };
    // 拖拽刚结束的 click / dblclick 吞掉（长按松手必发 click，
    // 会误触行内按钮；“双击播放”也依赖两次 click，一并防住）
    const onQuiet = (e: MouseEvent) => {
      if (performance.now() - dragEndAt.current < CLICK_QUIET_MS) {
        e.preventDefault();
        e.stopPropagation();
      }
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    window.addEventListener("click", onQuiet, true);
    window.addEventListener("dblclick", onQuiet, true);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("click", onQuiet, true);
      window.removeEventListener("dblclick", onQuiet, true);
      if (st.holdTimer) clearTimeout(st.holdTimer);
      if (st.settleTimer) clearTimeout(st.settleTimer);
      cancelAnimationFrame(st.raf);
      st.dragging = false;
      st.pressedIndex = -1;
      st.pressedRow = null;
      clearRows();
    };
  }, []);

  const rowProps = useCallback(
    (index: number) => ({
      onMouseDown: (e: ReactMouseEvent) => {
        if (e.button !== 0) return;
        if (!enabledRef.current) return;
        // 行内按钮/输入框不触发拖拽（长按“喜欢”不应拖走整行）
        const t = e.target as HTMLElement;
        if (t.closest("button, input, a, textarea, [data-nodrag]")) return;
        st.pressedIndex = index;
        st.pressedRow = e.currentTarget as HTMLElement;
        st.startY = e.clientY;
        st.pointerY = e.clientY;
        if (st.holdTimer) clearTimeout(st.holdTimer);
        st.holdTimer = setTimeout(() => beginRef.current(), HOLD_MS);
      },
    }),
    []
  );

  return { rowProps, setEnabled: (v: boolean) => (enabledRef.current = v) };
}
