//! UI/应用域：视图导航、提示、主题皮肤、桌面歌词、全局初始化（事件注册）。
import { create } from "zustand";
import type { StateCreator } from "zustand";
import { api, listenEvent, type ListenerUnbind } from "../api";
import {
  ACCENTS,
  applyAccent,
  applyTheme,
  loadAccent,
  loadTheme,
  saveAccent,
  saveTheme,
  loadDesktopLyricsColors,
  saveDesktopLyricsColors,
  type DesktopLyricsColors,
} from "../theme";
import { applySkin, loadSkin, saveSkin } from "../skins";
import type { PlayState, ScanState } from "../types";
import type { Store } from "./contract";
import { pushDesktopLyrics, resetDesktopLyricsFrame } from "./util";

type SetFn = (partial: Partial<Store> | ((s: Store) => Partial<Store>)) => void;
type GetFn = () => Store;

type Fields = Pick<
  Store,
  | "ready"
  | "view"
  | "viewParam"
  | "detailName"
  | "viewHistory"
  | "onlineRec"
  | "onlineNav"
  | "lastKw"
  | "search"
  | "nowPlayingOpen"
  | "fullscreen"
  | "queueOpen"
  | "scan"
  | "download"
  | "toasts"
  | "theme"
  | "accent"
  | "skin"
  | "desktopLyricsOn"
  | "desktopLyricsLock"
  | "dlyricsColors"
  | "sourcesResult"
  | "sourcesTab"
  | "init"
  | "toast"
  | "dismissToast"
  | "setView"
  | "setOnlineRec"
  | "pushOnlineNav"
  | "popOnlineNav"
  | "clearOnlineNav"
  | "setLastKw"
  | "openDetailPage"
  | "popDetailPage"
  | "setSearch"
  | "setNowPlayingOpen"
  | "toggleFullscreen"
  | "setQueueOpen"
  | "setTheme"
  | "setAccent"
  | "setSkin"
  | "openDesktopLyrics"
  | "closeDesktopLyrics"
  | "unlockDesktopLyrics"
  | "setDlyricsColors"
  | "setSourcesResult"
  | "setSourcesTab"
>;

let toastSeq = 1;
let unbinds: ListenerUnbind[] = [];
/** 最近一次收到引擎进度帧的时间（看门狗判断引擎是否静默用） */
let lastPosEventAt = 0;
/** init 单例：React StrictMode 双挂载 / 并发调用时只注册一次事件监听 */
let initPromise: Promise<void> | null = null;

/** 应用后端播放状态：player://state 事件与挂起恢复后的主动拉取共用。
 *  posOverride：拉取快照自带进度时直接采用（事件路径由 isFreshStart 决定是否归零）。 */
function applyPlayState(set: SetFn, get: GetFn, p: PlayState, posOverride?: number) {
  const liked =
    p.kind === "track" && p.id != null
      ? get().tracks.find((t) => t.id === p.id)?.liked ?? false
      : false;
  // seq 增加 = 换曲开播，进度归零；seq 不变 = 暂停/恢复，保留进度
  const isFreshStart =
    p.seq != null ? p.seq > get().playSeq : p.playing && !get().playing;
  if (p.seq != null) set({ playSeq: p.seq });
  // 同一首歌（挂起恢复的拉取/补发常如此）保持 current 引用不变，避免播放条闪烁
  const cur = get().current;
  const sameTrack =
    cur != null &&
    cur.kind === p.kind &&
    cur.id === (p.id ?? null) &&
    cur.path === p.path &&
    cur.title === p.title &&
    cur.artist === p.artist &&
    cur.album === p.album &&
    cur.cover === p.cover &&
    cur.durationMs === p.durationMs &&
    cur.nid === (p.nid ?? null) &&
    cur.qid === (p.qid ?? null) &&
    cur.kgid === (p.kgid ?? null) &&
    cur.quality === (p.quality ?? null) &&
    cur.liked === liked;
  set({
    ...(sameTrack
      ? {}
      : {
          current: {
            id: p.id ?? null,
            kind: p.kind,
            path: p.path,
            title: p.title,
            artist: p.artist,
            album: p.album,
            cover: p.cover,
            durationMs: p.durationMs,
            nid: p.nid ?? null,
            qid: p.qid ?? null,
            kgid: p.kgid ?? null,
            quality: p.quality ?? null,
            liked,
          },
        }),
    playing: p.playing,
    dur: p.durationMs,
    pos: posOverride != null ? posOverride : isFreshStart ? 0 : get().pos,
  });
  // 自动加载当前曲目的歌词（播放栏滚动展示用）
  const key =
    p.kind === "track" && p.id != null
      ? `track-${p.id}`
      : p.kind === "netease" && p.nid != null
        ? `net-${p.nid}`
        : p.kind === "qq" && p.qid != null
          ? `qq-${p.qid}`
          : p.kind === "kugou" && p.kgid != null
            ? `kug-${p.kgid}`
            : p.kind === "bilibili" && p.qid != null
              ? `bili-${p.qid}`
              : p.kind === "navidrome" && p.qid != null
                ? `nd-${p.qid}`
                : null;
  if (key) get().loadLyricsByKey(key);
  // 换曲开播：在线曲目更新“最近播放”；本地曲目只在本地更新单条的
  // lastPlayed/playCount（后端 record_play 已在开播时落库）——
  // 不再全量拉取曲目列表（大曲库下每首歌一次全量 IPC + 整表重渲染）
  if (isFreshStart && p.kind !== "track") get().refreshRecentOnline();
  if (isFreshStart && p.kind === "track" && p.id != null) {
    const now = Math.floor(Date.now() / 1000);
    set((s) => ({
      tracks: s.tracks.map((t) =>
        t.id === p.id ? { ...t, lastPlayed: now, playCount: t.playCount + 1 } : t
      ),
    }));
  }
  // 播放/暂停/停止的即时同步：暂停后 pos 事件停发，
  // 不在这里推一帧的话桌面歌词会一直按旧 playing 状态外推
  pushDesktopLyrics(get());
}

/** 主动拉取后端播放状态快照（挂起恢复 / 引擎静默看门狗共用） */
function pullPlayState(set: SetFn, get: GetFn) {
  api
    .getPlayState()
    .then((p) => {
      if (p) applyPlayState(set, get, p, p.pos);
      // 快照为空 = 引擎从未开播：纠正残留的“播放中”按钮状态
      else set({ playing: false });
    })
    .catch(() => {});
}

export const createUiSlice: StateCreator<Store, [], [], Fields> = (set, get) => ({
  ready: false,
  view: "library",
  viewParam: 0,
  detailName: "",
  viewHistory: [],
  onlineRec: { netease: null, qq: null, kugou: null },
  onlineNav: { netease: [], qq: [], kugou: [] },
  lastKw: { netease: "", qq: "", kugou: "" },
  search: "",
  nowPlayingOpen: false,
  fullscreen: false,
  queueOpen: false,
  scan: { active: false, done: 0, total: 0 },
  download: null,
  toasts: [],
  lyrics: null,
  lyricsLoading: false,
  lyricsFor: null,
  sourcesResult: null,
  sourcesTab: "bilibili",
  desktopLyricsOn: false,
  desktopLyricsLock: false,
  dlyricsColors: loadDesktopLyricsColors(),
  theme: "light",
  accent: "amber",
  skin: "default",
  // ---------- 初始化 ----------

  async init() {
    if (initPromise) return initPromise;
    initPromise = (async () => {
    unbinds.push(
      await listenEvent<PlayState>("player://state", (p) => applyPlayState(set, get, p))
    );

    unbinds.push(
      await listenEvent<{ pos: number; dur: number }>("player://pos", (p) => {
        lastPosEventAt = Date.now();
        // 拖动进度条期间不回写事件进度，避免位置抖动
        if (get().scrubbing) return;
        // 自愈：引擎只在播放中发 pos 事件。UI 的 playing 若与此不符
        // （在线切歌失败等路径误改），以引擎为准纠正。
        // 没有当前曲目时忽略：空引擎的 pos 帧（旧版 resync 补发）会把
        // 按钮误置为“播放中”
        if (!get().playing && get().current) set({ playing: true });
        set({ pos: p.pos, dur: p.dur > 0 ? p.dur : get().dur });
        // 桌面歌词跟随（250ms 一帧，歌词窗口自行插值当前行）
        pushDesktopLyrics(get());
      })
    );

    unbinds.push(
      await listenEvent("player://ended", () => get().next(true, true))
    );

    // 独占模式协商失败回退共享模式时，明确告知用户（否则无声降级难以察觉）
    unbinds.push(
      await listenEvent<{ reason: string }>("player://exclusive-fallback", (p) => {
        get().toast(`WASAPI 独占模式不可用，已自动回退普通模式：${p.reason}`, "error");
      })
    );

    // 在线音质回退提示：设置档位未被账号权益满足时明确告知（否则静默降级）
    unbinds.push(
      await listenEvent<{ message: string }>("player://quality-fallback", (p) => {
        get().toast(p.message, "info");
      })
    );

    // 看门狗：UI 认为在播放但引擎 3 秒没有进度事件（托盘挂起期间状态事件
    // 丢失、恢复补发也没送达等极端情况的兜底自愈），主动拉取权威快照纠正。
    // 正常播放中 pos 250ms 一帧，不会触发；拉取走 invoke 请求-响应，
    // 不依赖恢复窗口期的事件投递。
    window.setInterval(() => {
      if (!get().playing || Date.now() - lastPosEventAt < 3000) return;
      pullPlayState(set, get);
    }, 5000);

    // 桌面歌词窗口就绪握手：窗口创建/重开的初期发出的瘦身帧（不带 lines）
    // 可能一条都没被收到（监听尚未注册），握手后强制补推一帧全量状态
    //（含当前歌词），保证窗口起来就一定能显示到当前歌词
    unbinds.push(
      await listenEvent("dlyrics://ready", () => {
        resetDesktopLyricsFrame();
        pushDesktopLyrics(get());
      })
    );

    // 歌词窗口自己的 ✕ 关闭 / 锁定切换：同步主窗口“词”按钮状态
    unbinds.push(
      await listenEvent("dlyrics://closed", () =>
        set({ desktopLyricsOn: false, desktopLyricsLock: false })
      )
    );
    unbinds.push(
      await listenEvent<{ locked: boolean }>("dlyrics://lock", (p) =>
        set({ desktopLyricsLock: p.locked })
      )
    );

    // 主窗口隐藏到托盘时 WebView 会被挂起（后端 TrySuspend 回收渲染内存），
    // 挂起期间发往前端的事件全部丢失：恢复后刷新一遍数据收敛状态
    unbinds.push(
      await listenEvent("webview://resumed", () => {
        const s = get();
        s.refreshTracks();
        s.refreshFolders();
        s.refreshPlaylists();
        s.refreshRecentOnline();
        s.refreshCacheBytes();
        // 扫描进度以快照为准；下载完成事件若丢失则复位下载条
        api.getScanState().then((scan) => {
          const s = get().scan;
          // 快照没变化就不动引用，避免恢复时无谓重渲染
          if (
            s.active !== scan.active ||
            s.done !== scan.done ||
            s.total !== scan.total
          ) {
            set({ scan });
          }
        }).catch(() => {});
        set({ download: null });
        // 播放状态不依赖后端 250ms 补发推送（恢复窗口期投递不可靠）：
        // 主动拉取权威快照，播放/暂停/进度一律以后端为准
        pullPlayState(set, get);
        // 桌面歌词窗口在挂起期间可能被直接关闭（关闭事件丢失）：校准“词”按钮
        api
          .desktopLyricsIsOpen()
          .then((open) => {
            if (!open && get().desktopLyricsOn) {
              set({ desktopLyricsOn: false, desktopLyricsLock: false });
            }
          })
          .catch(() => {});
      })
    );

    unbinds.push(
      await listenEvent<{ action: string; value?: number }>("media://control", (p) =>
        get().applyMediaControl(p.action, p.value)
      )
    );

    unbinds.push(
      await listenEvent<ScanState>("scan://progress", (p) => {
        const wasActive = get().scan.active;
        set({ scan: p });
        if (wasActive && !p.active) {
          get().refreshTracks();
          get().refreshFolders();
        }
      })
    );

    unbinds.push(
      await listenEvent<{
        url: string;
        pct?: number;
        done?: boolean;
        error?: string;
        received?: number;
        total?: number;
      }>("download://progress", (p) => {
        if (p.error) {
          set({ download: null });
          get().toast(`音源下载失败：${p.error}`, "error");
          return;
        }
        if (p.done) {
          set({ download: null });
          return;
        }
        set({
          download: {
            title: p.url.split("/").pop() ?? p.url,
            pct: p.pct ?? 0,
          },
        });
      })
    );

    try {
      const [settings, tracks, folders, playlists, sources, neteaseStatus, qqStatus, biliStatus, kugouStatus] =
        await Promise.all([
          api.getSettings(),
          api.listTracks(),
          api.listFolders(),
          api.listPlaylists(),
          api.listSources(),
          api.neteaseStatus(),
          api.qqStatus(),
          api.biliStatus(),
          api.kugouStatus(),
        ]);
      set({
        theme: loadTheme(),
        accent: loadAccent(),
        skin: loadSkin(),
        volume: settings.volume,
        speed: settings.speed,
        eqGains: settings.eqGains,
        eqEnabled: settings.eqEnabled,
        tracks,
        folders,
        playlists,
        sources,
        neteaseLoggedIn: neteaseStatus.loggedIn,
        neteaseNickname: neteaseStatus.nickname,
        qqLoggedIn: qqStatus.loggedIn,
        qqNickname: qqStatus.nickname,
        biliLoggedIn: biliStatus.loggedIn,
        biliNickname: biliStatus.nickname,
        kugouLoggedIn: kugouStatus.loggedIn,
        kugouNickname: kugouStatus.nickname,
        quality: settings.quality,
        closeAction: settings.closeAction ?? "tray",
        autoUpdate: settings.autoUpdate ?? true,
        wasapiExclusive: settings.wasapiExclusive ?? false,
        cacheLimit: settings.cacheLimit ?? 2 * 1024 * 1024 * 1024,
        ready: true,
      });
      get().refreshCacheBytes();
      if (neteaseStatus.loggedIn) get().neteaseSyncLikes();
      get().refreshLikedOnline();
      get().refreshRecentOnline();
      get().loadManualOrder("library");
      get().loadManualOrder("liked");
    } catch (e) {
      set({ ready: true });
      get().toast(`初始化失败：${e}`, "error");
    }
    })();
    return initPromise;
  },
  // ---------- 提示 ----------

  toast(msg, type = "info") {
    const id = toastSeq++;
    set((s) => ({ toasts: [...s.toasts, { id, msg, type }] }));
    setTimeout(() => get().dismissToast(id), 3600);
    return id;
  },

  dismissToast(id) {
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
  },
  // ---------- 视图 ----------

  setView(v, param = 0) {
    set({ view: v, viewParam: param, search: "" });
  },

  setOnlineRec(source, rec) {
    set((s) => ({ onlineRec: { ...s.onlineRec, [source]: rec } }));
  },

  pushOnlineNav(source, snap) {
    set((s) => ({
      onlineNav: {
        ...s.onlineNav,
        [source]: [...s.onlineNav[source].slice(-9), snap],
      },
    }));
  },

  popOnlineNav(source) {
    const stack = get().onlineNav[source];
    if (!stack.length) return null;
    const prev = stack[stack.length - 1];
    set((s) => ({
      onlineNav: { ...s.onlineNav, [source]: s.onlineNav[source].slice(0, -1) },
    }));
    return prev;
  },

  clearOnlineNav(source) {
    set((s) => ({ onlineNav: { ...s.onlineNav, [source]: [] } }));
  },

  setLastKw(source, kw) {
    set((s) => ({ lastKw: { ...s.lastKw, [source]: kw } }));
  },

  openDetailPage(kind, name) {
    const nameTrim = name.trim();
    if (!nameTrim || nameTrim === "未知艺术家" || nameTrim === "未知专辑") return;
    set((s) => ({
      view: kind,
      viewParam: 0,
      detailName: nameTrim,
      search: "",
      viewHistory: [
        ...s.viewHistory.slice(-9),
        // 记录 detailName：详情页可以嵌套跳转（歌手页→专辑页），返回时名字也要还原
        { view: s.view, viewParam: s.viewParam, detailName: s.detailName },
      ],
    }));
  },

  popDetailPage() {
    set((s) => {
      const prev = s.viewHistory[s.viewHistory.length - 1];
      if (!prev) return {};
      return {
        view: prev.view,
        viewParam: prev.viewParam,
        detailName: prev.detailName,
        viewHistory: s.viewHistory.slice(0, -1),
      };
    });
  },

  setSearch(s) {
    set({ search: s });
  },

  setNowPlayingOpen(v) {
    set({ nowPlayingOpen: v });
  },

  toggleFullscreen(v) {
    const next = v ?? !get().fullscreen;
    set({ fullscreen: next });
    // 真全屏（占据整个显示器、隐藏任务栏）。最大化状态下 Windows 会拒绝
    // 切全屏，先还原窗口；错误显式提示而非静默吞掉
    import("@tauri-apps/api/window")
      .then(async ({ getCurrentWindow }) => {
        const win = getCurrentWindow();
        try {
          if (next && await win.isMaximized()) {
            await win.unmaximize();
          }
          await win.setFullscreen(next);
        } catch (e) {
          get().toast(`全屏切换失败：${e}`, "error");
        }
      })
      .catch((e) => get().toast(`全屏切换失败：${e}`, "error"));
  },

  setQueueOpen(v) {
    set({ queueOpen: v });
  },
  // ---------- 桌面歌词 ----------

  async openDesktopLyrics() {
    try {
      await api.desktopLyricsOpen();
      set({ desktopLyricsOn: true, desktopLyricsLock: false });
      // 窗口是全新的（没有历史帧可沿用）：强制下一帧携带完整歌词
      resetDesktopLyricsFrame();
      // 立即推一帧当前状态（窗口加载完成可能晚于这次推送，靠后续 pos 事件补）
      pushDesktopLyrics(get());
      get().toast("桌面歌词已开启（L 键切换）", "info");
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async closeDesktopLyrics() {
    try {
      await api.desktopLyricsClose();
      set({ desktopLyricsOn: false, desktopLyricsLock: false });
    } catch (e) {
      get().toast(String(e), "error");
    }
  },

  async unlockDesktopLyrics() {
    try {
      await api.desktopLyricsUnlock();
      set({ desktopLyricsLock: false });
      // 后端只恢复了鼠标事件；歌词窗口的 locked 状态必须经事件同步，
      // 否则窗口收得到点击但控制条/缩放手柄永不渲染（卡死在无 UI 状态）
      const { emit } = await import("@tauri-apps/api/event");
      await emit("dlyrics://lock", { locked: false });
    } catch {
      // 窗口可能已关闭：静默
    }
  },

  setDlyricsColors(c) {
    set({ dlyricsColors: c });
    saveDesktopLyricsColors(c);
    // 即时生效：推一帧新颜色给悬浮窗（窗口没开时 push 内部自跳过）
    pushDesktopLyrics(get());
  },
  setTheme(t) {
    set({ theme: t });
    saveTheme(t);
    applyTheme(t);
    applyAccent(get().accent); // 强调色需按模式重算
  },

  setAccent(key) {
    set({ accent: key });
    saveAccent(key);
    applyAccent(key);
  },

  setSkin(key) {
    set({ skin: key });
    saveSkin(key);
    applySkin(key);
  },
  setSourcesResult(r) {
    if (typeof r === "function") {
      set((s) => ({ sourcesResult: r(s.sourcesResult) }));
    } else {
      set({ sourcesResult: r });
    }
  },

  setSourcesTab(t) {
    set({ sourcesTab: t });
  },
});
