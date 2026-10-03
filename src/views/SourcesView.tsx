import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { createPortal } from "react-dom";
import {
  Calendar,
  Check,
  ChevronLeft,
  ChevronRight,
  Download,
  Folder,
  Heart,
  Link2,
  ListMusic,
  Loader2,
  Play,
  Plus,
  Server,
  Square,
  Star,
  Trash2,
  Tv,
  Users,
  X,
} from "lucide-react";
import { useStore } from "../store";
import NavidromePanel from "./NavidromePanel";
import Modal from "../components/Modal";
import CoverImg from "../components/CoverImg";
import { fmtDate, fmtTime } from "../utils";
import type { BiliCollection, BiliFollow, BiliSpaceItem, QueueItem, SourcesResult } from "../types";
import { useLocatePillEl } from "../hooks/useLocatePillEl";
import LocateCurrentPill from "../components/LocateCurrentPill";

const ORDERS: { key: "pubdate" | "click" | "stow"; label: string }[] = [
  { key: "pubdate", label: "最新发布" },
  { key: "click", label: "最多播放" },
  { key: "stow", label: "最多收藏" },
];

type SourceTab = "bilibili" | "navidrome" | "other";

const TABS: { key: SourceTab; label: string }[] = [
  { key: "bilibili", label: "Bilibili" },
  { key: "navidrome", label: "Navidrome" },
  { key: "other", label: "其它" },
];

function fmtPlay(n: number): string {
  if (n >= 10000_0000) return `${(n / 10000_0000).toFixed(1)}亿`;
  if (n >= 10000) return `${(n / 10000).toFixed(1)}万`;
  return String(n);
}

/** 收藏 UP 主横排：一行头像（下方名字），最右一对箭头按钮横向滚动；
 *  支持按住头像左右拖动排序（mouse 事件实现：Tauri 窗口的原生文件拖放
 *  拦截会吃掉页面内 HTML5 拖放事件，与曲库手动排序 useDragList 同思路；
 *  拖动中被拖头像跟随指针、其余头像实时让位，松手滑入槽位后提交） */
function UpFollowRow({
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

export default function SourcesView() {
  const playSourceId = useStore((s) => s.playSourceId);
  const playBilibiliList = useStore((s) => s.playBilibiliList);
  const playNext = useStore((s) => s.playNext);
  const addToQueue = useStore((s) => s.addToQueue);
  const entryToQueueItem = useStore((s) => s.entryToQueueItem);
  const toggleLikeOnline = useStore((s) => s.toggleLikeOnline);
  const downloadOnline = useStore((s) => s.downloadOnline);
  const addOnlineToPlaylist = useStore((s) => s.addOnlineToPlaylist);
  const playlists = useStore((s) => s.playlists);
  const createPlaylist = useStore((s) => s.createPlaylist);
  const savedOnline = useStore((s) => s.savedOnline);
  const current = useStore((s) => s.current);
  const biliLoggedIn = useStore((s) => s.biliLoggedIn);
  const biliNickname = useStore((s) => s.biliNickname);
  const biliSetLogin = useStore((s) => s.biliSetLogin);
  const biliLogout = useStore((s) => s.biliLogout);
  const result = useStore((s) => s.sourcesResult);
  const setResult = useStore((s) => s.setSourcesResult);
  const tab = useStore((s) => s.sourcesTab);
  const setTab = useStore((s) => s.setSourcesTab);
  const biliFollows = useStore((s) => s.biliFollows);
  const biliToggleFollow = useStore((s) => s.biliToggleFollow);
  const biliReorderFollows = useStore((s) => s.biliReorderFollows);

  const [url, setUrl] = useState("");
  const [title, setTitle] = useState("");
  const [bili, setBili] = useState("");
  const [biliBusy, setBiliBusy] = useState(false);
  /** 收藏夹列表加载中 */
  const [favLoading, setFavLoading] = useState(false);
  /** 多选模式与选中行（key = rid） */
  const [selMode, setSelMode] = useState(false);
  const [selKeys, setSelKeys] = useState<Set<string>>(new Set());
  /** 右键菜单 */
  const [menu, setMenu] = useState<{ x: number; y: number; row: BiliSpaceItem } | null>(null);
  /** “添加到播放列表”弹窗（支持批量） */
  const [pickerRows, setPickerRows] = useState<BiliSpaceItem[] | null>(null);
  const [newPlName, setNewPlName] = useState("");
  // “创建并添加”进行中：双击会创建两个同名歌单（后端不按名去重）
  const [creatingPl, setCreatingPl] = useState(false);
  /** B 站扫码登录弹窗 */
  const [qr, setQr] = useState<{ qr: string } | null>(null);
  const qrRunRef = useRef(0);
  // 组件卸载（切到其他视图）时终止扫码轮询：Modal 随卸载消失但不触发
  // onClose，轮询循环唯一的常规取消路径会漏掉这种情况
  useEffect(() => {
    return () => {
      qrRunRef.current++;
    };
  }, []);
  // UP 空间请求代次：切排序/进合集/换空间会使在途旧响应作废，
  // 防止慢的旧响应覆盖新结果或把已退出的合集装回 UI
  const spaceGenRef = useRef(0);

  const canAdd = useMemo(() => /^https?:\/\//.test(url.trim()), [url]);

  const rowKey = (r: BiliSpaceItem) => `bilibili-${r.rid}`;
  const rowActive = (r: BiliSpaceItem) =>
    current?.kind === "bilibili" && current.qid != null && current.qid === r.rid;

  // 「回到当前歌曲」药丸 + 队列自动续页（B 站空间列表，卡片网格用元素级定位）
  const listRef = useRef<HTMLDivElement | null>(null);
  const pill = useLocatePillEl({
    containerRef: listRef,
    activeSelector: current?.kind === "bilibili" ? '[data-now-playing="1"]' : null,
  });
  const resultRef = useRef(result);
  resultRef.current = result;
  const loadMoreRef = useRef<() => Promise<BiliSpaceItem[] | null>>(() =>
    Promise.resolve(null)
  );
  const registerQueueExtender = (expectedTail: string) => {
    let tail = expectedTail;
    useStore.getState().setQueueExtender(async () => {
      const st = useStore.getState();
      const last = st.queue[st.queue.length - 1];
      if (!last || last.kind !== "bilibili" || String(last.id) !== tail) {
        return null; // 队列尾部已变：上下文过期
      }
      const res = resultRef.current;
      const hasMore =
        res && res.type === "space"
          ? res.activeCollection
            ? res.activeCollection.hasMore
            : res.hasMore
          : false;
      if (!hasMore) return null;
      const fresh = await loadMoreRef.current();
      if (!fresh?.length) return null;
      tail = String(fresh[fresh.length - 1].rid);
      return fresh.map((r) => ({ kind: "bilibili", id: r.rid }) as QueueItem);
    });
  };
  const rowLiked = (r: BiliSpaceItem) => !!savedOnline[rowKey(r)];

  const toOnlineRow = (r: BiliSpaceItem) => ({
    kind: "bilibili" as const,
    id: r.rid,
    name: r.title,
    artist: r.artist,
    album: "哔哩哔哩",
    cover: r.cover,
    durationMs: r.durationMs,
    mediaMid: "",
    vip: false,
  });

  const menuEntryToQueueItem = (r: BiliSpaceItem) =>
    entryToQueueItem({
      rowid: 0,
      kind: "bilibili",
      trackId: null,
      onlineId: r.rid,
      title: r.title,
      artist: r.artist,
      album: "哔哩哔哩",
      cover: r.cover,
      duration: r.durationMs / 1000,
      mediaMid: "",
      vip: false,
      lastPlayed: 0,
    });

  /** 当前结果区的行（多选/全选操作对象） */
  const visibleRows: BiliSpaceItem[] = !result
    ? []
    : result.type === "video"
      ? result.rows
      : result.type === "direct"
        ? result.rows
        : result.activeCollection
          ? result.activeCollection.rows
          : result.rows;

  const replaceResult = (r: SourcesResult | null) => {
    setResult(r);
    setSelKeys(new Set());
  };

  /** 解析 UP 主空间（输入可为 space 链接或纯 UID），写入结果区 */
  const openSpace = async (input: string) => {
    const gen = ++spaceGenRef.current;
    const r = await useStore.getState().biliSpace(input, "pubdate");
    if (gen !== spaceGenRef.current) return;
    replaceResult({
      type: "space",
      mid: r.mid,
      name: r.name,
      face: r.face,
      fans: r.fans,
      total: r.total,
      order: "pubdate",
      rows: r.items,
      pn: 1,
      hasMore: r.hasMore,
      loadingMore: false,
      collections: r.collections,
      activeCollection: null,
    });
  };

  const submitBili = async () => {
    const input = bili.trim();
    if (!input || biliBusy) return;
    setBiliBusy(true);
    try {
      if (/space\.bilibili\.com\/\d+/.test(input) || /^\d+$/.test(input)) {
        await openSpace(input);
      } else {
        const rows = await useStore.getState().biliVideoInfo(input);
        if (!rows.length) throw new Error("未解析到视频");
        replaceResult({ type: "video", rows });
      }
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    } finally {
      setBiliBusy(false);
      setBili("");
    }
  };

  /** 点击收藏的 UP 主：直接打开其空间（搜索框置为空，走 UID 解析） */
  const openFollowed = (f: BiliFollow) => {
    if (biliBusy) return;
    setBiliBusy(true);
    openSpace(f.mid)
      .catch((e) => useStore.getState().toast(String(e), "error"))
      .finally(() => setBiliBusy(false));
  };

  const changeOrder = async (order: "pubdate" | "click" | "stow") => {
    if (!result || result.type !== "space") return;
    const s = result;
    // 收藏夹伪空间没有投稿排序概念
    if (s.mid === "fav") return;
    const gen = ++spaceGenRef.current;
    try {
      setResult({
        ...s,
        order,
        rows: [],
        pn: 0,
        hasMore: false,
        loadingMore: true,
        activeCollection: null,
      });
      setSelKeys(new Set());
      const r = await useStore.getState().biliSpaceMore(s.mid, order, 1);
      if (gen !== spaceGenRef.current) return;
      setResult((cur) =>
        cur && cur.type === "space" && cur.mid === s.mid
          ? { ...cur, order, rows: r.items, pn: 1, hasMore: r.hasMore, loadingMore: false }
          : cur
      );
    } catch (e) {
      if (gen !== spaceGenRef.current) return;
      setResult({ ...s, loadingMore: false });
      useStore.getState().toast(String(e), "error");
    }
  };

  const loadMore = async (): Promise<BiliSpaceItem[] | null> => {
    if (!result || result.type !== "space") return null;
    const s = result;
    const gen = spaceGenRef.current;
    if (s.activeCollection) {
      const c = s.activeCollection;
      if (c.loadingMore || !c.hasMore) return null;
      setResult({ ...s, activeCollection: { ...c, loadingMore: true } });
      try {
        const r = await useStore
          .getState()
          .biliSpaceCollectionMore(s.mid, c.id, c.kind, c.pn + 1);
        if (gen !== spaceGenRef.current) return null;
        setResult((cur) =>
          cur && cur.type === "space" && cur.activeCollection?.id === c.id
            ? {
                ...cur,
                activeCollection: {
                  ...c,
                  rows: [...c.rows, ...r.items],
                  pn: c.pn + 1,
                  hasMore: r.hasMore,
                  loadingMore: false,
                },
              }
            : cur
        );
        return r.items;
      } catch (e) {
        if (gen !== spaceGenRef.current) return null;
        setResult({ ...s, activeCollection: { ...c, loadingMore: false } });
        useStore.getState().toast(String(e), "error");
      }
    } else {
      if (s.loadingMore || !s.hasMore) return null;
      setResult({ ...s, loadingMore: true });
      try {
        const r = await useStore.getState().biliSpaceMore(s.mid, s.order, s.pn + 1);
        if (gen !== spaceGenRef.current) return null;
        setResult((cur) =>
          cur && cur.type === "space" && cur.mid === s.mid
            ? {
                ...cur,
                rows: [...cur.rows, ...r.items],
                pn: s.pn + 1,
                hasMore: r.hasMore,
                loadingMore: false,
              }
            : cur
        );
        return r.items;
      } catch (e) {
        if (gen !== spaceGenRef.current) return null;
        setResult({ ...s, loadingMore: false });
        useStore.getState().toast(String(e), "error");
      }
    }
    return null;
  };
  loadMoreRef.current = loadMore;

  const openCollection = async (c: BiliCollection) => {
    if (!result || result.type !== "space") return;
    const s = result;
    const gen = ++spaceGenRef.current;
    try {
      setResult({
        ...s,
        activeCollection: { ...c, rows: [], pn: 0, hasMore: false, loadingMore: true },
      });
      setSelKeys(new Set());
      const r = await useStore.getState().biliSpaceCollection(s.mid, c.id, c.kind);
      if (gen !== spaceGenRef.current) return;
      setResult((cur) =>
        cur && cur.type === "space" && cur.mid === s.mid
          ? {
              ...cur,
              activeCollection: {
                ...c,
                rows: r.items,
                pn: 1,
                hasMore: r.hasMore,
                loadingMore: false,
              },
            }
          : cur
      );
    } catch (e) {
      if (gen !== spaceGenRef.current) return;
      setResult({ ...s, activeCollection: null });
      useStore.getState().toast(String(e), "error");
    }
  };

  /** 打开登录用户的收藏夹：装载成伪空间（mid="fav"），收藏夹作为
   *  kind="fav" 的合集 chips 展示，内容走 biliSpaceCollection 的 fav 分支 */
  const openFavs = async () => {
    if (favLoading) return;
    const gen = ++spaceGenRef.current;
    setFavLoading(true);
    try {
      const { folders, name, face } = await useStore.getState().biliFavFolders();
      if (gen !== spaceGenRef.current) return;
      const total = folders.reduce((a, f) => a + f.total, 0);
      replaceResult({
        type: "space",
        mid: "fav",
        name: name || "我的收藏夹",
        face,
        fans: "",
        total,
        order: "pubdate",
        rows: [],
        pn: 1,
        hasMore: false,
        loadingMore: false,
        collections: folders.map((f) => ({
          id: f.id,
          kind: "fav",
          title: f.title,
          total: f.total,
        })),
        activeCollection: null,
      });
      // 自动展开第一个收藏夹（openCollection 读渲染闭包的 result，
      // 这里是刚 replace 的，直接拉内容回填）
      if (folders.length) {
        const first = folders[0];
        const r = await useStore.getState().biliSpaceCollection("fav", first.id, "fav");
        if (gen !== spaceGenRef.current) return;
        setResult((cur) =>
          cur && cur.type === "space" && cur.mid === "fav"
            ? {
                ...cur,
                activeCollection: {
                  id: first.id,
                  kind: "fav",
                  title: first.title,
                  total: first.total,
                  rows: r.items,
                  pn: 1,
                  hasMore: r.hasMore,
                  loadingMore: false,
                },
              }
            : cur
        );
      }
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    } finally {
      if (gen === spaceGenRef.current) setFavLoading(false);
    }
  };

  /** 打开扫码登录：过期自动换新码循环，直到成功或关闭弹窗 */
  const startBiliLogin = async () => {
    const runId = ++qrRunRef.current;
    const { api } = await import("../api");
    try {
      for (;;) {
        if (qrRunRef.current !== runId) return;
        const { key, qr } = await api.biliQrCreate();
        if (qrRunRef.current !== runId) return;
        setQr({ qr });
        let scanned = false;
        const t0 = Date.now();
        for (;;) {
          await new Promise((r) => setTimeout(r, 1000));
          if (qrRunRef.current !== runId) return;
          if (Date.now() - t0 > 100_000 && !scanned) break;
          let status = "";
          let nickname: string | undefined;
          try {
            const r = await api.biliQrCheck(key);
            status = r.status;
            nickname = r.nickname;
          } catch {
            continue;
          }
          if (status === "success") {
            biliSetLogin(true, nickname ?? "");
            useStore
              .getState()
              .toast(`B 站登录成功${nickname ? `：${nickname}` : ""}`, "success");
            setQr(null);
            return;
          }
          if (status === "scanned") scanned = true;
          if (status === "expired") {
            if (scanned) {
              useStore.getState().toast("确认超时，已刷新二维码", "info");
            }
            break;
          }
        }
      }
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    }
  };

  // ---------- 多选批量 ----------

  const selRows = visibleRows.filter((r) => selKeys.has(r.rid));

  const toggleSel = (rid: string) => {
    setSelKeys((prev) => {
      const next = new Set(prev);
      if (next.has(rid)) next.delete(rid);
      else next.add(rid);
      return next;
    });
  };

  const selectAll = () => {
    if (selRows.length === visibleRows.length) setSelKeys(new Set());
    else setSelKeys(new Set(visibleRows.map((r) => r.rid)));
  };

  const batchLike = () => {
    const targets = selRows.filter((r) => !rowLiked(r));
    if (!targets.length) {
      useStore.getState().toast("所选都已收藏过", "info");
      return;
    }
    for (const r of targets) toggleLikeOnline(toOnlineRow(r));
    useStore.getState().toast(`正在收藏 ${targets.length} 首…`, "success");
  };

  const batchDownload = () => {
    if (!selRows.length) return;
    for (const r of selRows) downloadOnline(toOnlineRow(r));
    useStore.getState().toast(`开始下载 ${selRows.length} 首…`, "info");
  };

  /** 渲染一张结果卡片（16:9 封面网格） */
  const renderRow = (r: BiliSpaceItem, extra?: { sourceId?: number }) => {
    const active = !selMode && rowActive(r);
    const checked = selMode && selKeys.has(r.rid);
    const removeRow = () => {
      setResult((cur) => {
        if (!cur) return cur;
        if (cur.type === "video")
          return { ...cur, rows: cur.rows.filter((x) => x.rid !== r.rid) };
        if (cur.type === "space") {
          if (cur.activeCollection)
            return {
              ...cur,
              activeCollection: {
                ...cur.activeCollection,
                rows: cur.activeCollection.rows.filter((x) => x.rid !== r.rid),
              },
            };
          return { ...cur, rows: cur.rows.filter((x) => x.rid !== r.rid) };
        }
        return { ...cur, rows: cur.rows.filter((x) => x.rid !== r.rid) };
      });
    };
    return (
      <div
        key={r.rid}
        data-now-playing={active ? "1" : undefined}
        className={`group cursor-pointer rounded-xl p-1.5 transition-colors ${
          checked ? "bg-[var(--accent-weak)]" : "hover:bg-[var(--shade)]"
        }`}
        onContextMenu={(e) => {
          if (selMode) return;
          e.preventDefault();
          setMenu({ x: e.clientX, y: e.clientY, row: r });
        }}
        onClick={() => {
          if (selMode) toggleSel(r.rid);
        }}
      >
        {/* 16:9 封面 */}
        <div className="relative aspect-video rounded-lg overflow-hidden bg-[var(--shade)]">
          <CoverImg
            src={r.cover}
            seed={r.title}
            className="absolute inset-0 w-full h-full"
            iconSize={26}
          />
          {/* 时长角标 */}
          {r.durationMs > 0 && (
            <span className="absolute bottom-1 right-1 px-1.5 py-0.5 rounded bg-black/70 text-white text-[10.5px] tabular-nums">
              {fmtTime(r.durationMs)}
            </span>
          )}
          {/* 播放按钮浮层 */}
          {!selMode && (
            <button
              className="absolute inset-0 m-auto w-10 h-10 rounded-full bg-black/55 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
              title="播放（整个列表连播）"
              onClick={(e) => {
                e.stopPropagation();
                if (extra?.sourceId != null) playSourceId(extra.sourceId);
                else {
                  const idx = visibleRows.findIndex((x) => x.rid === r.rid);
                  playBilibiliList(visibleRows, idx >= 0 ? idx : 0);
                  registerQueueExtender(String(visibleRows[visibleRows.length - 1].rid));
                }
              }}
            >
              <Play size={16} className="fill-current text-white ml-0.5" />
            </button>
          )}
          {/* 多选勾选框 */}
          {selMode && (
            <span
              className={`absolute top-1.5 left-1.5 w-[18px] h-[18px] rounded-[5px] border flex items-center justify-center transition-colors ${
                checked
                  ? "bg-[var(--accent)] border-[var(--accent)] text-[var(--accent-on)]"
                  : "border-white/70 bg-black/30"
              }`}
            >
              {checked && <Check size={13} strokeWidth={3} />}
            </span>
          )}
          {/* 已收藏标记 */}
          {rowLiked(r) && !selMode && (
            <span className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full bg-black/45 flex items-center justify-center">
              <Heart size={10} className="fill-[#e0533f] text-[#e0533f]" />
            </span>
          )}
          {/* 移除 */}
          {!selMode && (
            <button
              className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full bg-black/45 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity hover:bg-black/70"
              title="从结果中移除"
              onClick={(e) => {
                e.stopPropagation();
                removeRow();
              }}
            >
              <X size={11} className="text-white" />
            </button>
          )}
          {/* 正在播放标记 */}
          {active && (
            <span className="absolute left-1.5 bottom-1 px-1.5 py-0.5 rounded bg-[#fb7299] text-white text-[10px] font-semibold">
              播放中
            </span>
          )}
        </div>
        {/* 标题 + 元信息 */}
        <div
          className={`text-[12.5px] leading-snug mt-1.5 ${
            active ? "text-[#fb7299] font-semibold" : "text-[var(--ink)]"
          }`}
          style={{
            display: "-webkit-box",
            WebkitLineClamp: 2,
            WebkitBoxOrient: "vertical",
            overflow: "hidden",
          }}
        >
          {r.title || "B站视频"}
        </div>
        <div className="text-[11px] text-[var(--ink-3)] truncate mt-0.5 flex items-center gap-1.5">
          <span className="truncate">{r.artist || "哔哩哔哩"}</span>
          {r.play > 0 && (
            <>
              <span>·</span>
              <span className="shrink-0">{fmtPlay(r.play)}</span>
            </>
          )}
          {r.created > 0 && (
            <>
              <span>·</span>
              <span className="shrink-0">{fmtDate(r.created)}</span>
            </>
          )}
        </div>
      </div>
    );
  };

  const locatedKeyRef = useRef("");
  useEffect(() => {
    const key = result
      ? result.type === "space"
        ? `space:${result.mid}`
        : result.type === "video"
          ? `video:${result.rows[0]?.rid ?? ""}`
          : `direct:${result.rows[0]?.sourceId ?? ""}`
      : "";
    if (!key || locatedKeyRef.current === key) return;
    locatedKeyRef.current = key;
    // 新解析结果：正在播放的 B 站歌曲若在列表中，定位到它
    requestAnimationFrame(() => {
      listRef.current
        ?.querySelector('[data-now-playing="1"]')
        ?.scrollIntoView({ block: "center" });
    });
  }, [result]);

  const space = result && result.type === "space" ? result : null;
  const spaceFollowed = space ? biliFollows.some((f) => f.mid === space.mid) : false;
  const listRows = selMode ? visibleRows : [];

  /** 当前页签下结果区显示的结果（页签与结果类型不匹配时显示空态；切回页签即还原） */
  const tabResult =
    result &&
    ((tab === "bilibili" && (result.type === "video" || result.type === "space")) ||
      (tab === "other" && result.type === "direct"))
      ? result
      : null;

  return (
    <div className="flex-1 min-h-0 flex flex-col pb-[84px]">
      <LocateCurrentPill
        show={pill.show}
        onClick={pill.locate}
        className="fixed bottom-[92px] left-1/2 -translate-x-1/2"
      />
      <header className="pt-5 pb-3 px-5">
        {/* 音源页签 */}
        <div className="flex items-center gap-1.5">
          {TABS.map((t) => (
            <button
              key={t.key}
              className={`h-8 px-4 rounded-full text-[12.5px] transition-colors ${
                tab === t.key
                  ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-semibold"
                  : "text-[var(--ink-2)] hover:bg-[var(--shade)]"
              }`}
              onClick={() => setTab(t.key)}
            >
              {t.label}
            </button>
          ))}
        </div>

        {tab === "bilibili" && (
          <>
            <div className="flex gap-2 mt-3">
              <div className="relative flex-1 max-w-[480px]">
                <Tv
                  size={14}
                  className="absolute left-3 top-1/2 -translate-y-1/2 text-[#fb7299]"
                />
                <input
                  type="text"
                  value={bili}
                  onChange={(e) => setBili(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && bili.trim() && !biliBusy) submitBili();
                  }}
                  placeholder="B站视频链接 / BV号，或 UP 主空间链接 / UID"
                  className="w-full h-9 rounded-lg bg-[var(--shade)] border border-[var(--line)] pl-8 pr-3 text-[12.5px] focus:border-[var(--line)] transition-colors"
                />
              </div>
              <button
                className="btn-primary"
                disabled={!bili.trim() || biliBusy}
                style={{ opacity: bili.trim() && !biliBusy ? 1 : 0.45 }}
                onClick={submitBili}
              >
                <Tv size={14} />
                解析
              </button>
              <button
                className="btn-secondary h-9 px-3 text-[12px] shrink-0 flex items-center gap-1.5"
                disabled={favLoading}
                onClick={openFavs}
                title="查看登录用户创建的收藏夹"
              >
                {favLoading ? (
                  <Loader2 size={13} className="animate-spin" />
                ) : (
                  <Folder size={13} />
                )}
                我的收藏夹
              </button>
              <button
                className="btn-secondary h-9 px-3 text-[12px] shrink-0 flex items-center gap-1.5"
                onClick={() => (biliLoggedIn ? undefined : startBiliLogin())}
                title={biliLoggedIn ? "" : "登录 B 站后可显示视频字幕（作歌词）、按收藏排序"}
              >
                <Tv size={13} className={biliLoggedIn ? "text-[#fb7299]" : ""} />
                {biliLoggedIn ? biliNickname || "已登录" : "B站登录"}
              </button>
              {biliLoggedIn && (
                <button
                  className="btn-secondary h-9 px-3 text-[12px] shrink-0 flex items-center gap-1.5"
                  onClick={() => biliLogout()}
                  title="退出 B 站登录"
                >
                  退出
                </button>
              )}
            </div>
            {/* 收藏的 UP 主：头像+名字一行，右侧箭头滚动 */}
            <UpFollowRow
              follows={biliFollows}
              activeMid={space?.mid ?? null}
              busy={biliBusy}
              onOpen={openFollowed}
              onRemove={biliToggleFollow}
              onReorder={biliReorderFollows}
            />
          </>
        )}

        {tab === "other" && (
          <div className="flex gap-2 mt-3">
            <div className="relative flex-1 max-w-[480px]">
              <Link2
                size={14}
                className="absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ink-2)]"
              />
              <input
                type="url"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && canAdd) submitDirect();
                }}
                placeholder="https://example.com/song.mp3（添加后立即播放）"
                className="w-full h-9 rounded-lg bg-[var(--shade)] border border-[var(--line)] pl-8 pr-3 text-[12.5px] focus:border-[var(--line)] transition-colors"
              />
            </div>
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && canAdd) submitDirect();
              }}
              placeholder="名称（可选）"
              className="w-[160px] h-9 rounded-lg bg-[var(--shade)] border border-[var(--line)] px-3 text-[12.5px] focus:border-[var(--line)] transition-colors"
            />
            <button
              className="btn-primary"
              disabled={!canAdd}
              style={{ opacity: canAdd ? 1 : 0.45 }}
              onClick={submitDirect}
            >
              <Plus size={14} />
              添加
            </button>
          </div>
        )}
      </header>

      {/* 多选操作条（仅当当前页签下有对应结果时显示） */}
      {selMode && tabResult && (
        <div className="mx-5 mb-2 h-10 px-3 rounded-xl glass-strong flex items-center gap-1.5 shrink-0">
          <Square size={13} className="text-[var(--accent)] shrink-0" />
          <span className="text-[12px] text-[var(--ink)] mr-1 shrink-0">
            已选 {selRows.length}
          </span>
          <button className="btn-ghost h-7 px-2.5 text-[11.5px]" onClick={selectAll}>
            {selRows.length === visibleRows.length && visibleRows.length > 0
              ? "全不选"
              : "全选"}
          </button>
          <button
            className="btn-ghost h-7 px-2.5 text-[11.5px] flex items-center gap-1"
            disabled={!selRows.length}
            onClick={batchLike}
          >
            <Heart size={12} /> 收藏
          </button>
          <button
            className="btn-ghost h-7 px-2.5 text-[11.5px] flex items-center gap-1"
            disabled={!selRows.length}
            onClick={batchDownload}
          >
            <Download size={12} /> 下载
          </button>
          <button
            className="btn-ghost h-7 px-2.5 text-[11.5px] flex items-center gap-1"
            disabled={!selRows.length}
            onClick={() => {
              if (selRows.length) setPickerRows(selRows);
            }}
          >
            <ListMusic size={12} /> 加歌单
          </button>
          <div className="flex-1" />
          <button
            className="btn-ghost h-7 px-2.5 text-[11.5px] flex items-center gap-1"
            onClick={() => {
              setSelMode(false);
              setSelKeys(new Set());
            }}
          >
            <X size={12} /> 完成
          </button>
        </div>
      )}

      <div ref={listRef} className="flex-1 min-h-0 overflow-y-auto px-5 pb-2">
        {/* Navidrome 常驻挂载：切 Tab 用 display:none 隐藏而非卸载——
            保留连接状态、当前页面（导航栈）与滚动位置，
            切回来不再闪登录页重新自动连接 */}
        <div className={tab === "navidrome" ? "h-full" : "hidden"}>
          <NavidromePanel />
        </div>
        {tab !== "navidrome" &&
          (!result || !tabResult ? (
          <div className="h-full flex flex-col items-center justify-center gap-3 text-[var(--ink-3)]">
            <div
              className="w-16 h-16 rounded-2xl flex items-center justify-center"
              style={{
                background: "rgba(255,255,255,0.04)",
                border: "1px solid rgba(255,255,255,0.06)",
              }}
            >
              {tab === "other" ? (
                <Link2 size={26} className="text-[var(--ink-3)]" />
              ) : (
                <Tv size={26} className="text-[var(--ink-3)]" />
              )}
            </div>
            <div className="text-[13.5px]">
              {tab === "other" ? "粘贴音频文件直链开始" : "粘贴 B 站视频或 UP 主空间链接开始"}
            </div>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            {space && (
              <>
                <div className="flex items-center gap-3 px-3.5 py-3 rounded-xl bg-white/[0.03] border border-white/[0.05]">
                  <CoverImg
                    src={space.face}
                    seed={space.name}
                    className="w-14 h-14 rounded-full shrink-0"
                    iconSize={18}
                  />
                  <div className="min-w-0 flex-1">
                    <div className="text-[14.5px] font-bold text-[var(--ink)] truncate flex items-center gap-2">
                      <Tv size={13} className="text-[#fb7299] shrink-0" />
                      {space.name}
                    </div>
                    <div className="text-[11.5px] text-[var(--ink-2)] flex items-center gap-1.5 mt-0.5">
                      <Users size={10} />
                      {space.mid === "fav"
                        ? `共 ${space.total} 个视频`
                        : `${space.fans} · 共 ${space.total} 个视频`}
                    </div>
                  </div>
                  {!selMode && space.mid !== "fav" && (
                    <button
                      className={`btn-ghost h-8 px-3 text-[12px] shrink-0 flex items-center gap-1.5 ${
                        spaceFollowed ? "text-[#fb7299]" : ""
                      }`}
                      onClick={() =>
                        biliToggleFollow({
                          mid: space.mid,
                          name: space.name,
                          face: space.face,
                        })
                      }
                      title={spaceFollowed ? "取消收藏该 UP 主" : "收藏该 UP 主（显示在顶部横排）"}
                    >
                      <Star size={12} className={spaceFollowed ? "fill-current" : ""} />
                      {spaceFollowed ? "已收藏" : "收藏UP主"}
                    </button>
                  )}
                  {!selMode && (
                    <button
                      className="btn-ghost h-8 px-3 text-[12px] shrink-0 flex items-center gap-1.5"
                      onClick={() => {
                        setSelMode(true);
                        setSelKeys(new Set());
                      }}
                    >
                      <Square size={12} /> 多选
                    </button>
                  )}
                  <button
                    className="btn-ghost h-8 px-3 text-[12px] shrink-0"
                    onClick={() => {
                      replaceResult(null);
                      setSelMode(false);
                    }}
                  >
                    关闭
                  </button>
                </div>
                <div className="flex items-center gap-1.5 flex-wrap">
                  {/* 收藏夹伪空间没有投稿排序 */}
                  {space.mid !== "fav" &&
                    ORDERS.map((o) => (
                      <button
                        key={o.key}
                        className={`h-7 px-3 rounded-full text-[11.5px] transition-colors ${
                          !space.activeCollection && space.order === o.key
                            ? "bg-[#fb7299]/15 text-[#fb7299] font-semibold"
                            : "text-[var(--ink-2)] hover:bg-[var(--shade)]"
                        }`}
                        onClick={() => changeOrder(o.key)}
                      >
                        {o.label}
                      </button>
                    ))}
                  {space.collections.map((c) => (
                    <button
                      key={`${c.kind}-${c.id}`}
                      className={`h-7 px-3 rounded-full text-[11.5px] transition-colors max-w-[220px] truncate ${
                        space.activeCollection?.id === c.id
                          ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-semibold"
                          : "text-[var(--ink-2)] hover:bg-[var(--shade)]"
                      }`}
                      title={`${c.title}（${c.total}个）`}
                      onClick={() => openCollection(c)}
                    >
                      <ListMusic size={10} className="inline mr-1 -mt-px" />
                      {c.title}
                    </button>
                  ))}
                  {/* 多选时非 space 结果也要能进多选：video/direct 的入口在列表上方 */}
                </div>
                {space.activeCollection && (
                  <div className="text-[12px] text-[var(--ink-2)] px-1 flex items-center gap-2">
                    <span className="text-[var(--ink)] font-medium">
                      {space.activeCollection.title}
                    </span>
                    <span>{space.activeCollection.total} 个视频</span>
                    <button
                      className="text-[var(--accent)] hover:underline"
                      onClick={() => {
                        setResult((cur) =>
                          cur && cur.type === "space"
                            ? { ...cur, activeCollection: null }
                            : cur
                        );
                        setSelKeys(new Set());
                      }}
                    >
                      返回全部投稿
                    </button>
                  </div>
                )}
              </>
            )}
            {!space && !selMode && visibleRows.length > 0 && (
              <div className="flex items-center justify-between px-1">
                <span className="text-[11.5px] text-[var(--ink-3)]">
                  {visibleRows.length} 个结果
                </span>
                <button
                  className="btn-ghost h-7 px-2.5 text-[11.5px] flex items-center gap-1.5"
                  onClick={() => {
                    setSelMode(true);
                    setSelKeys(new Set());
                  }}
                >
                  <Square size={12} /> 多选
                </button>
              </div>
            )}
            {!space && selMode && listRows.length === 0 && (
              <div className="text-[12px] text-[var(--ink-3)] text-center py-2">
                当前结果为空
              </div>
            )}

            <div className="grid grid-cols-[repeat(auto-fill,minmax(190px,1fr))] gap-x-3 gap-y-4">
              {result.type === "video" && result.rows.map((r) => renderRow(r))}
              {result.type === "direct" &&
                result.rows.map((r) => renderRow(r, { sourceId: r.sourceId }))}
              {space &&
                (space.activeCollection ? space.activeCollection.rows : space.rows).map(
                  (r) => renderRow(r)
                )}
            </div>

            {space &&
              (space.activeCollection ? space.activeCollection.hasMore : space.hasMore) && (
                <button
                  className="h-10 rounded-xl text-[12.5px] text-[var(--ink-2)] hover:bg-[var(--shade)] border border-white/[0.05] flex items-center justify-center gap-2 transition-colors disabled:opacity-50"
                  disabled={
                    space.activeCollection
                      ? space.activeCollection.loadingMore
                      : space.loadingMore
                  }
                  onClick={loadMore}
                >
                  <Plus size={13} />
                  {space.activeCollection
                    ? space.activeCollection.loadingMore
                      ? "加载中…"
                      : `加载更多（已显示 ${space.activeCollection.rows.length}/${space.activeCollection.total}）`
                    : space.loadingMore
                      ? "加载中…"
                      : `加载更多（已显示 ${space.rows.length}/${space.total}）`}
                </button>
              )}
          </div>
            ))}
      </div>

      {/* 右键菜单（Portal 到 body：fixed 相对视口定位） */}
      {menu &&
        createPortal(
          <div
            className="fixed z-[75] w-[190px] glass-strong rounded-xl p-1.5 shadow-2xl anim-menu"
            style={{ left: menu.x, top: menu.y }}
            onMouseDown={(e) => e.stopPropagation()}
            onMouseLeave={() => setMenu(null)}
          >
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                const idx = visibleRows.findIndex((x) => x.rid === menu.row.rid);
                playBilibiliList(visibleRows, idx >= 0 ? idx : 0);
                setMenu(null);
              }}
            >
              <Play size={13} /> 播放
            </button>
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                const item = menuEntryToQueueItem(menu.row);
                if (item) playNext(item);
                setMenu(null);
              }}
            >
              <Play size={13} /> 下一首播放
            </button>
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                const item = menuEntryToQueueItem(menu.row);
                if (item) addToQueue(item);
                setMenu(null);
              }}
            >
              <Play size={13} /> 加入队列
            </button>
            <div className="my-1 mx-2 border-t border-[var(--line)]" />
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                setPickerRows([menu.row]);
                setMenu(null);
              }}
            >
              <ListMusic size={13} /> 添加到播放列表…
            </button>
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                toggleLikeOnline(toOnlineRow(menu.row));
                setMenu(null);
              }}
            >
              <Heart size={13} /> {rowLiked(menu.row) ? "取消喜欢" : "收藏到“我喜欢”"}
            </button>
            <button
              className="w-full h-8 px-2.5 rounded-lg flex items-center gap-2.5 text-[12.5px] text-[var(--ink)] hover:bg-[var(--shade-strong)] text-left"
              onClick={() => {
                downloadOnline(toOnlineRow(menu.row));
                setMenu(null);
              }}
            >
              <ListMusic size={13} /> 下载到本地
            </button>
          </div>,
          document.body
        )}

      {/* 添加到播放列表弹窗（单行/批量共用） */}
      <Modal
        open={pickerRows != null}
        onClose={() => setPickerRows(null)}
        title={pickerRows && pickerRows.length > 1 ? `添加 ${pickerRows.length} 首到播放列表` : "添加到播放列表"}
        width={380}
      >
        <div className="flex flex-col gap-1.5 max-h-[260px] overflow-y-auto">
          {playlists.map((p) => (
            <button
              key={p.id}
              className="h-10 px-3 rounded-lg text-left text-[13px] text-[var(--ink)] hover:bg-[var(--shade)] flex items-center justify-between transition-colors"
              onClick={async () => {
                if (pickerRows) {
                  for (const r of pickerRows) {
                    await addOnlineToPlaylist(p.id, toOnlineRow(r));
                  }
                }
                setPickerRows(null);
              }}
            >
              <span className="truncate">{p.name}</span>
              <span className="text-[11px] text-[var(--ink-2)]">
                {p.entries.length} 首
              </span>
            </button>
          ))}
          {!playlists.length && (
            <div className="text-[12.5px] text-[var(--ink-2)] py-2">
              还没有播放列表，在下方创建
            </div>
          )}
        </div>
        <div className="flex gap-2 mt-3">
          <input
            type="text"
            value={newPlName}
            onChange={(e) => setNewPlName(e.target.value)}
            placeholder="新播放列表名称"
            className="flex-1 h-9 rounded-lg bg-[var(--shade)] border border-[var(--line)] px-3 text-[13px] focus:border-[var(--line)] outline-none"
          />
          <button
            className="btn-secondary"
            disabled={creatingPl}
            onClick={async () => {
              if (creatingPl || !newPlName.trim() || !pickerRows?.length) return;
              setCreatingPl(true);
              try {
                const pid = await createPlaylist(newPlName.trim());
                if (pid >= 0) {
                  for (const r of pickerRows) {
                    await addOnlineToPlaylist(pid, toOnlineRow(r));
                  }
                }
              } finally {
                setCreatingPl(false);
              }
              setNewPlName("");
              setPickerRows(null);
            }}
          >
            创建并添加
          </button>
        </div>
      </Modal>

      {/* B 站扫码登录弹窗 */}
      <Modal
        open={!!qr}
        onClose={() => {
          qrRunRef.current++;
          setQr(null);
        }}
        title="B 站登录"
        width={300}
      >
        <div className="flex flex-col items-center gap-3 py-2">
          {qr && (
            <img
              src={qr.qr}
              alt="B站登录二维码"
              className="w-[220px] h-[220px] rounded-xl bg-white p-2"
            />
          )}
          <div className="text-[12.5px] text-[var(--ink-2)] text-center leading-relaxed">
            用 <span className="text-[#fb7299] font-medium">哔哩哔哩 App</span> 扫码登录
            <br />
            登录后可显示视频字幕（作歌词）、按收藏排序
          </div>
        </div>
      </Modal>
    </div>
  );

  /** 直链：入库（供 playSource 播放）+ 作为当前结果展示 */
  async function submitDirect() {
    const u = url.trim();
    if (!/^https?:\/\//.test(u)) return;
    try {
      const { api } = await import("../api");
      const id = await api.addSource(u, title.trim());
      setUrl("");
      setTitle("");
      replaceResult({
        type: "direct",
        rows: [
          {
            rid: `direct-${id}`,
            title: title.trim() || u.split("/").pop() || "在线音源",
            artist: "在线音源",
            cover: "",
            durationMs: 0,
            play: 0,
            created: Date.now() / 1000,
            sourceId: id,
          },
        ],
      });
      useStore.getState().toast("已添加并播放", "success");
      playSourceId(id);
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    }
  }
}
