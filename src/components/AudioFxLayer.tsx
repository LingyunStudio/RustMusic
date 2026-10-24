/**
 * 黑胶皮肤音频律动层：与背景 SVG 同 viewBox / preserveAspectRatio 的
 * 内联 SVG 覆盖层，几何与背景频谱柱逐一对齐（同 slice 规则 = 像素级
 * 重合，无需换算）。数据来自 Rust FFT（fx.ts 桥），30Hz 事件 → rAF
 * 插值直接改 DOM，全程零 React 重渲染。
 *
 * 性能关键：所有逐帧更新走合成器路径——柱子/涟漪用 CSS transform
 * （scaleY/scale）而不是改几何属性（height/y/r 会触发 SVG 完整布局，
 * 60fps 下每秒 120+ 次布局重算，渲染进程的光栅缓存池会被持续撑大，
 * 实测是 300MB 工作集的主要来源）。静态几何按各柱最大高度铺满，
 * 运行时只压缩 scaleY；数值量化 + 变化阈值跳过无谓的样式写入。
 *
 * 三类元素：36 根频谱柱（真实频段）、节拍涟漪（低频 beat 触发）、
 * 盘心光晕（总能量呼吸）。暂停/无声时数据陈旧，柱子自动落回基线。
 */
import { useEffect, useRef } from "react";
import { getFxFrame } from "../fx";

/** 与皮肤静态频谱柱同形的设计包络（"山丘"形态），律动高度按频段值缩放 */
const DESIGN_H = [
  46, 72, 58, 96, 76, 122, 92, 142, 112, 162, 132, 186, 152, 212, 172, 232,
  192, 244, 206, 256, 218, 248, 228, 258, 238, 252, 222, 236, 198, 212, 172,
  188, 148, 164, 124, 140,
];
const BAR_N = DESIGN_H.length;
const BAR_X = DESIGN_H.map((_, i) => 42 + i * 43);
/** 各柱满格高度（律动包络 × 系数）；静态几何按此铺设，scaleY 压缩 */
const BAR_MAX = DESIGN_H.map((h) => h * 1.18);
/** 基线最小可见高度（px，viewBox 单位） */
const BAR_MIN = 8;
/** 数据陈旧阈值（ms）：超过即视为暂停/无声，目标值回落基线 */
const STALE_MS = 300;
const RIPPLE_N = 3;
/** 涟漪基础半径 → 最大半径的缩放倍数（185 × 3.02 ≈ 560） */
const RIPPLE_K = 3.02;

export default function AudioFxLayer() {
  const barsRef = useRef<(SVGRectElement | null)[]>([]);
  const glowRef = useRef<SVGCircleElement | null>(null);
  const rippleRefs = useRef<(SVGCircleElement | null)[]>([]);

  useEffect(() => {
    // 涟漪时序状态（秒；>2.2 视为已消亡）
    const ripples = Array.from({ length: RIPPLE_N }, () => 99);
    const disp = BAR_MAX.map((m) => BAR_MIN / m);
    let dispLevel = 0;
    // 上次写入值缓存：量化 + 阈值跳过，静止时零样式写入
    const lastBar = new Array<string>(BAR_N).fill("");
    let lastGlow = "";
    const lastRipple = new Array<string>(RIPPLE_N).fill("");
    let prev = performance.now();
    let raf = 0;

    const loop = (now: number) => {
      raf = requestAnimationFrame(loop);
      if (document.hidden) {
        prev = now;
        return;
      }
      const dt = Math.min(0.1, (now - prev) / 1000);
      prev = now;
      const { frame, age } = getFxFrame();
      const live = age < STALE_MS;
      const k = 1 - Math.exp(-dt * 14);

      for (let i = 0; i < BAR_N; i++) {
        const band = live ? frame.bands[i] ?? 0 : 0;
        const target = Math.max(BAR_MIN / BAR_MAX[i], band);
        disp[i] += (target - disp[i]) * k;
        const el = barsRef.current[i];
        if (!el) continue;
        // 量化到 1/256：亚像素级变化不值得触发一次样式写入
        const q = Math.round(disp[i] * 256) / 256;
        const t = `scaleY(${q.toFixed(3)})`;
        if (t !== lastBar[i]) {
          lastBar[i] = t;
          el.style.transform = t;
          const op = (0.2 + Math.min(0.5, q * 0.45)).toFixed(2);
          el.style.opacity = op;
        }
      }

      dispLevel += ((live ? frame.level : 0) - dispLevel) * k;
      const glow = glowRef.current;
      if (glow) {
        const q = (Math.round(dispLevel * 50) / 50).toFixed(2);
        if (q !== lastGlow) {
          lastGlow = q;
          glow.style.opacity = q;
        }
      }

      // 节拍涟漪：复用最老的一槽（限制触发密度，避免高频拍点糊成一片）
      if (live && frame.beat) {
        let oldest = 0;
        for (let i = 1; i < RIPPLE_N; i++)
          if (ripples[i] > ripples[oldest]) oldest = i;
        if (ripples[oldest] > 0.55) ripples[oldest] = 0;
      }
      for (let i = 0; i < RIPPLE_N; i++) {
        ripples[i] += dt;
        const el = rippleRefs.current[i];
        if (!el) continue;
        const life = ripples[i] / 2.2;
        if (life >= 1) {
          if (lastRipple[i] !== "h") {
            lastRipple[i] = "h";
            el.style.opacity = "0";
          }
          continue;
        }
        const eased = 1 - Math.pow(1 - life, 2.2);
        const s = (Math.round(eased * 128) / 128).toFixed(3);
        const o = (0.085 * (1 - life)).toFixed(3);
        const t = `${s}|${o}`;
        if (t !== lastRipple[i]) {
          lastRipple[i] = t;
          el.style.transform = `scale(${1 + (RIPPLE_K - 1) * Number(s)})`;
          el.style.opacity = o;
        }
      }
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, []);

  return (
    <svg
      className="absolute inset-0 w-full h-full"
      viewBox="0 0 1600 1000"
      preserveAspectRatio="xMidYMid slice"
      aria-hidden
    >
      <defs>
        <linearGradient id="fx-bars" x1="0" y1="1" x2="0" y2="0">
          <stop offset="0" stopColor="#7c421d" stopOpacity=".85" />
          <stop offset="1" stopColor="#ffb066" stopOpacity=".95" />
        </linearGradient>
        <radialGradient id="fx-glow" cx=".5" cy=".5" r=".5">
          <stop offset="0" stopColor="#ffb066" stopOpacity=".32" />
          <stop offset="1" stopColor="#ffb066" stopOpacity="0" />
        </radialGradient>
      </defs>
      <style>{`
        .fx-tb { transform-box: fill-box; transform-origin: 50% 100%; }
        .fx-tc { transform-box: fill-box; transform-origin: 50% 50%; }
      `}</style>
      {/* 总能量光晕（呼吸） */}
      <circle
        ref={glowRef}
        cx="1120"
        cy="500"
        r="520"
        fill="url(#fx-glow)"
        opacity="0"
      />
      {/* 节拍涟漪池：基础半径上 scale 放大（无几何属性变更） */}
      {[0, 1, 2].map((i) => (
        <circle
          key={i}
          ref={(el) => {
            rippleRefs.current[i] = el;
          }}
          className="fx-tc"
          cx="1120"
          cy="500"
          r="185"
          fill="none"
          stroke="#ffc98f"
          strokeWidth="2"
          opacity="0"
        />
      ))}
      {/* 频谱柱：真实频段驱动；静态几何满高，scaleY 压缩 */}
      <g fill="url(#fx-bars)">
        {BAR_X.map((x, i) => (
          <rect
            key={i}
            ref={(el) => {
              barsRef.current[i] = el;
            }}
            className="fx-tb"
            x={x}
            y={1000 - BAR_MAX[i]}
            width={22}
            height={BAR_MAX[i]}
            rx={4}
            opacity={0.2}
          />
        ))}
      </g>
    </svg>
  );
}
