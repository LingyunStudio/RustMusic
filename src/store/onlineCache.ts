//! 在线曲目元数据缓存的容量上限（防御性封顶）。
//! 播放队列条目只存 {kind, id}，播放时元数据从这些缓存解析（playQueueIndex），
//! 因此这里不能做激进的 LRU——上限取得极宽裕（重度搜索/导入整个会话也难触达），
//! 只防长时间使用下无上限增长。淘汰按写入顺序（FIFO）。

const ONLINE_CACHE_CAP = 2000;
export type OnlineCacheKind = "netease" | "qq" | "kugou" | "bili" | "nd";
const onlineCacheOrder: Record<OnlineCacheKind, (string | number)[]> = {
  netease: [],
  qq: [],
  kugou: [],
  bili: [],
  nd: [],
};

/**
 * 提交在线缓存前调用：登记新写入的键（相对 prev），超上限按写入序淘汰最旧条目。
 * next 必须是调用方新建的副本（本项目的写入模式均是拷贝后修改），可安全原地删
 */
export function capOnlineCache<K extends string | number, V>(
  kind: OnlineCacheKind,
  next: Record<K, V>,
  prev: Record<K, V>
): Record<K, V> {
  const order = onlineCacheOrder[kind] as K[];
  for (const k in next) {
    if (!(k in prev)) order.push(k);
  }
  while (order.length > ONLINE_CACHE_CAP) {
    const oldest = order.shift();
    if (oldest !== undefined && oldest in next) delete next[oldest];
  }
  return next;
}
