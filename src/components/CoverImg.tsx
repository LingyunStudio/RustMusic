import { useEffect, useState } from "react";
import { Music2 } from "lucide-react";
import { api, coverSrc } from "../api";
import { remoteCoverCache } from "../utils";
import { gradientFor } from "../utils";

interface CoverImgProps {
  src?: string;
  seed: string;
  className?: string;
  iconSize?: number;
  /**
   * 列表小图档：本地 .jpg 封面优先加载 160px 缩略图（<hash>_s.jpg），
   * 缺失（旧库未迁移）时回退原图。64px 行高用不到 600px 解码位图，
   * 小图把列表滚动的解码缓存从 ~1.4MB/张 压到 ~100KB/张
   */
  small?: boolean;
}

export default function CoverImg({ src, seed, className = "", iconSize = 18, small = false }: CoverImgProps) {
  const [err, setErr] = useState(false);
  const [smallMissing, setSmallMissing] = useState(false);
  // 远程封面经后端拉取落盘：WebKit 的 <img> 遵循系统代理设置，用户代理
  // （GNOME manual → 127.0.0.1:xxxx）没在运行时全部远程图加载失败（整列表
  // 灰占位）。后端 ureq 直连不读代理，还有磁盘缓存。按 URL 缓存解析结果
  const [remoteLocal, setRemoteLocal] = useState<string | null>(null);
  const isRemote = !!src && (src.startsWith("http://") || src.startsWith("https://"));
  useEffect(() => {
    setErr(false);
    setSmallMissing(false);
    setRemoteLocal(null);
    if (!isRemote || !src) return;
    let alive = true;
    // 在线缓存的封面 URL 频繁重复：模块级 Map 进程内去重 + 后端磁盘缓存兜底
    remoteCoverCache.get(src)?.then((p) => {
      if (alive) setRemoteLocal(p);
      return;
    });
    if (!remoteCoverCache.has(src)) {
      const p = api.fetchCover(src).catch(() => null);
      remoteCoverCache.set(src, p as Promise<string>);
      p.then((path) => {
        if (alive && path) setRemoteLocal(path);
      });
    }
    return () => {
      alive = false;
    };
  }, [src, isRemote]);

  const isLocal = !!src && !isRemote;
  const useSmall = small && isLocal && !smallMissing && /\.(jpg|jpeg)$/i.test(src);
  const url = src
    ? isLocal
      ? coverSrc(useSmall ? src.replace(/\.(jpg|jpeg)$/i, "_s.jpg") : src)
      : remoteLocal
        ? coverSrc(remoteLocal)
        : ""
    : "";
  if (url && !err) {
    return (
      <img
        src={url}
        alt=""
        draggable={false}
        // lazy：不在可视区附近的封面不发起加载（长列表滚动时按需解码，
        // 渲染进程的图片缓存不再随曲库规模线性增长）
        loading="lazy"
        decoding="async"
        // 不带 Referer：应用来源（tauri.localhost）会被 B 站等图床的
        // 防盗链拒绝（403），无 Referer 反而放行
        referrerPolicy="no-referrer"
        onError={() => {
          // 小图不存在（老库迁移完成前）：回退原图重试一次
          if (useSmall) setSmallMissing(true);
          else setErr(true);
        }}
        className={`object-cover bg-[var(--shade)] ${className}`}
      />
    );
  }
  return (
    <div
      className={`flex items-center justify-center text-[var(--ink)]/40 ${className}`}
      style={{ background: gradientFor(seed) }}
    >
      <Music2 size={iconSize} strokeWidth={1.6} />
    </div>
  );
}
