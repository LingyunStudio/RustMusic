//! 收藏 UP 主横排：一行头像（下方名字），最右一对箭头按钮横向滚动；
//! 支持按住头像左右拖动排序（mouse 事件实现：Tauri 窗口的原生文件拖放
//! 拦截会吃掉页面内 HTML5 拖放事件，与曲库手动排序 useDragList 同思路；
//! 拖动中被拖头像跟随指针、其余头像实时让位，松手滑入槽位后提交）。
import { useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { ChevronLeft, ChevronRight, X } from "lucide-react";
import CoverImg from "../../components/CoverImg";
import type { BiliFollow } from "../../types";

/** 收藏 UP 主横排：一行头像（下方名字），最右一对箭头按钮横向滚动；
 *  支持按住头像左右拖动排序（mouse 事件实现：Tauri 窗口的原生文件拖放
 *  拦截会吃掉页面内 HTML5 拖放事件，与曲库手动排序 useDragList 同思路；
 *  拖动中被拖头像跟随指针、其余头像实时让位，松手滑入槽位后提交） */
export function UpFollowRow({
  follows,
  activeMid,
  busy,
  onOpen,
  onRemove,
  onReorder,
}: {
  follows: BiliFollow[];
  activeMid: string | null;
  busy?: boolean;
  onOpen: (f: BiliFollow) => void;
  onRemove: (f: BiliFollow) => void;
  onReorder: (from: number, to: number) => void;
}) {
  const scRef = useRef<HTMLDivElement | null>(null);
  const [canLeft, setCanLeft] = useState(false);
  const [canRight, setCanRight] = useState(false);
  /** 拖拽状态：from = 起始下标；to = 落点（移除源后的最终下标）；
   *  dx = 被拖头像的指针位移；settle = 松手后滑入槽位的动画阶段 */
  const [drag, setDrag] = useState<
    { from: number; to: number; dx: number; pitch: number; settle?: boolean } | null
  >(null);
  const dragRef = useRef(drag);
  dragRef.current = drag;
  /** 拖拽会话：mousedown 时记录布局参数，移动超阈值才转正为拖拽（区分点击） */
  const sessionRef = useRef<{
    index: number;
    rect0: number;
    pitch: number;
    count: number;
    scroll0: number;
    startX: number;
  } | null>(null);
  const activeRef = useRef(false);
  /** 拖拽结束时刻：短窗口内吞掉 click，防止松手误开空间 */
  const dragEndAt = useRef(-1e9);
  const settleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const onItemMouseDown = (e: ReactMouseEvent<HTMLDivElement>, i: number) => {
    if (e.button !== 0) return;
    const el = scRef.current;
    if (!el || follows.length < 2) return;
    const kids = Array.from(el.children) as HTMLElement[];
    if (kids.length < 2) return;
    // 实测格距（头像等宽 + gap），不用写死
    const pitch = kids[1].offsetLeft - kids[0].offsetLeft;
    if (pitch <= 0) return;
    sessionRef.current = {
      index: i,
      rect0: kids[0].offsetLeft,
      pitch,
      count: kids.length,
      scroll0: el.scrollLeft,
      startX: e.clientX,
    };
  };

  // 拖拽生命周期挂在 window 上（按下在行内、移动/松手可能离开行）
  useEffect(() => {
    const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

    const onMove = (e: MouseEvent) => {
      const p = sessionRef.current;
      if (!p) return;
      let d = dragRef.current;
      if (!activeRef.current) {
        // 未超过位移阈值前不算拖拽：普通点击打开空间不受影响
        if (Math.abs(e.clientX - p.startX) < 6) return;
        activeRef.current = true;
        document.body.classList.add("dragging-ups");
        d = { from: p.index, to: p.index, dx: 0, pitch: p.pitch };
        dragRef.current = d;
        setDrag(d);
      }
      e.preventDefault();
      const el = scRef.current;
      if (!el || !d) return;
      // 被拖头像 1:1 跟随指针（限制在横排范围内）；容器滚动时补偿位移
      const dx = clamp(
        e.clientX - p.startX + (el.scrollLeft - p.scroll0),
        -p.index * p.pitch,
        (p.count - 1 - p.index) * p.pitch
      );
      // 落点 = 头像中心所在槽位（即移除源后的最终下标，无需再折算）
      const center = p.rect0 + p.index * p.pitch + p.pitch / 2 + dx;
      const to = clamp(
        Math.round((center - p.rect0 - p.pitch / 2) / p.pitch),
        0,
        p.count - 1
      );
      if (to !== d.to || dx !== d.dx) {
        const next = { ...d, to, dx };
        dragRef.current = next;
        setDrag(next);
      }
    };

    /** 松手提交 / ESC 或失焦取消（不提交，视觉复位） */
    const finish = (commit: boolean) => {
      sessionRef.current = null;
      if (!activeRef.current) return;
      activeRef.current = false;
      document.body.classList.remove("dragging-ups");
      dragEndAt.current = performance.now();
      const d = dragRef.current;
      if (!d) return;
      if (commit && d.to !== d.from) {
        // 两段式：先滑入目标槽位，落定同一帧清样式并提交——
        // 提交触发的重渲染里新顺序的自然位置与动画终点一致，无跳变
        const settled = { ...d, dx: (d.to - d.from) * d.pitch, settle: true };
        dragRef.current = settled;
        setDrag(settled);
        settleTimer.current = setTimeout(() => {
          settleTimer.current = null;
          dragRef.current = null;
          setDrag(null);
          reorderRef.current(d.from, d.to);
        }, 150);
      } else {
        dragRef.current = null;
        setDrag(null);
      }
    };

    const onUp = () => finish(true);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") finish(false);
    };
    const onBlur = () => finish(false);
    const onQuiet = (e: MouseEvent) => {
      if (performance.now() - dragEndAt.current < 350) {
        e.preventDefault();
        e.stopPropagation();
      }
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    window.addEventListener("click", onQuiet, true);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("click", onQuiet, true);
      if (settleTimer.current) clearTimeout(settleTimer.current);
      document.body.classList.remove("dragging-ups");
      sessionRef.current = null;
      activeRef.current = false;
    };
  }, []);

  // onReorder 经 ref 转发进 window 级监听，回调变化不重挂监听
  const reorderRef = useRef(onReorder);
  reorderRef.current = onReorder;

  const updateArrows = () => {
    const el = scRef.current;
    if (!el) return;
    setCanLeft(el.scrollLeft > 2);
    setCanRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 2);
  };

  // 数量变化后重算箭头可用态（首增/清空时滚动条出现或消失）
  useEffect(updateArrows, [follows.length]);

  const scroll = (dir: 1 | -1) => {
    const el = scRef.current;
    if (!el) return;
    el.scrollBy({ left: dir * Math.max(240, el.clientWidth * 0.8), behavior: "smooth" });
  };

  return (
    <div className="flex items-center gap-2 mt-2">
      <div
        ref={scRef}
        className="flex-1 min-w-0 flex items-center gap-1 overflow-x-hidden py-1"
        onScroll={updateArrows}
      >
        {!follows.length && (
          <span className="text-[11.5px] text-[var(--ink-3)] px-1 leading-relaxed">
            暂无收藏的 UP 主：解析 UP 主空间后点「收藏UP主」，会显示在这里
          </span>
        )}
        {follows.map((f, i) => {
          const active = activeMid === f.mid;
          // 拖拽中：from→to 之间的头像让位平移一格（实时预览落点）
          let shift = 0;
          if (drag) {
            if (drag.to > drag.from && i > drag.from && i <= drag.to) shift = -drag.pitch;
            else if (drag.to < drag.from && i >= drag.to && i < drag.from) shift = drag.pitch;
          }
          const dragged = drag?.from === i;
          return (
            <div
              key={f.mid}
              onMouseDown={(e) => onItemMouseDown(e, i)}
              className="relative group shrink-0 select-none"
              style={{
                transform: dragged
                  ? `translateX(${drag ? drag.dx : 0}px)`
                  : shift
                    ? `translateX(${shift}px)`
                    : undefined,
                transition:
                  dragged && !drag?.settle
                    ? undefined
                    : drag
                      ? "transform 150ms ease"
                      : undefined,
                opacity: dragged ? 0.45 : undefined,
                zIndex: dragged ? 1 : undefined,
              }}
            >
              <button
                className={`flex flex-col items-center gap-1 w-[68px] py-1 rounded-lg transition-colors ${
                  active ? "bg-[#fb7299]/10" : "hover:bg-[var(--shade)]"
                }`}
                disabled={busy}
                onClick={() => onOpen(f)}
                title={`打开「${f.name}」的投稿空间`}
              >
                <CoverImg
                  src={f.face}
                  seed={f.name}
                  className={`w-10 h-10 rounded-full shrink-0 ${
                    active ? "ring-2 ring-[#fb7299]" : ""
                  }`}
                  iconSize={16}
                />
                <span
                  className={`w-full px-1 text-[10.5px] leading-tight truncate text-center ${
                    active ? "text-[#fb7299] font-semibold" : "text-[var(--ink-2)]"
                  }`}
                >
                  {f.name}
                </span>
              </button>
              <button
                className="absolute top-0 right-0.5 w-4 h-4 rounded-full bg-black/55 text-white hidden group-hover:flex items-center justify-center hover:bg-black/80 transition-colors"
                title="取消收藏"
                onClick={(e) => {
                  e.stopPropagation();
                  onRemove(f);
                }}
              >
                <X size={9} />
              </button>
            </div>
          );
        })}
      </div>
      <div className="flex items-center gap-1 shrink-0">
        <button
          className="w-7 h-7 rounded-full flex items-center justify-center text-[var(--ink-2)] hover:bg-[var(--shade)] disabled:opacity-30 disabled:hover:bg-transparent transition-colors"
          disabled={!canLeft}
          onClick={() => scroll(-1)}
          title="向左滚动"
        >
          <ChevronLeft size={15} />
        </button>
        <button
          className="w-7 h-7 rounded-full flex items-center justify-center text-[var(--ink-2)] hover:bg-[var(--shade)] disabled:opacity-30 disabled:hover:bg-transparent transition-colors"
          disabled={!canRight}
          onClick={() => scroll(1)}
          title="向右滚动"
        >
          <ChevronRight size={15} />
        </button>
      </div>
    </div>
  );
}
