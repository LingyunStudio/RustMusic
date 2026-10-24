//! 在线曲目元数据缓存的容量上限（防御性封顶）。
//! 播放队列条目只存 {kind, id}，播放时元数据从这些缓存解析（playQueueIndex），
//! 因此淘汰必须避开正在被引用的键：播放 3000 首的在线歌单时，无差别 FIFO
//! 曾把队首 1000 条的元数据逐出，从头播放触发上千次“曲目信息缺失”连跳。

const ONLINE_CACHE_CAP = 2000;
export type OnlineCacheKind = "netease" | "qq" | "kugou" | "bili" | "nd";
const onlineCacheOrder: Record<OnlineCacheKind, (string | number)[]> = {
  netease: [],
  qq: [],
  kugou: [],
  bili: [],
  nd: [],
};

/** 构建某源的“引用保护键”集合：当前播放队列 + 我喜欢列表里的该源条目。
 *  这些键的元数据正被（或即将被）播放使用，容量淘汰时必须跳过 */
export function pinnedIdsOf(
  kind: string,
  s: {
    queue: { kind: string; id: string | number }[];
    likedOnline: { kind: string; onlineId?: string | null }[];
  }
): Set<string | number> {
  const out = new Set<string | number>();
  // 键形态混存：netease 队列项 id 是 number、缓存键也是 number，
  // 但 likedOnline.onlineId 恒为 string——同一曲目要同时登记两种形态，
  // 否则严格相等比较下 pin 不生效
  for (const q of s.queue) {
    if (q.kind === kind) {
      out.add(q.id);
      out.add(String(q.id));
    }
  }
  for (const e of s.likedOnline) {
    if (e.kind === kind && e.onlineId) {
      out.add(e.onlineId);
      const n = Number(e.onlineId);
      if (Number.isFinite(n)) out.add(n);
    }
  }
  return out;
}

/**
 * 提交在线缓存前调用：登记新写入的键（相对 prev），超上限按写入序淘汰最旧条目。
 * next 必须是调用方新建的副本（本项目的写入模式均是拷贝后修改），可安全原地删。
 * pins：引用保护键集合（见 pinnedIdsOf）——被引用的键跳过淘汰；
 * 引用解除（换队列/取消收藏）后下一次调用即可正常淘汰，上限是软上限。
 */
export function capOnlineCache<K extends string | number, V>(
  kind: OnlineCacheKind,
  next: Record<K, V>,
  prev: Record<K, V>,
  pins?: ReadonlySet<string | number>
): Record<K, V> {
  const order = onlineCacheOrder[kind] as K[];
  for (const k in next) {
    if (!(k in prev)) order.push(k as K);
  }
  // 引用集不计入淘汰配额：缓存被队列引用着就是“在用”，不是泄漏
  const evictable = order.filter((k) => !pins?.has(k));
  while (evictable.length > ONLINE_CACHE_CAP) {
    const oldest = evictable.shift();
    if (oldest !== undefined && oldest in next) {
      delete next[oldest];
      const at = order.indexOf(oldest);
      if (at >= 0) order.splice(at, 1);
    }
  }
  return next;
}
