import { useEffect, useRef, type RefObject } from "react";
import { getFxFrame } from "../fx";
import { useI18n } from "../i18n";

interface VUMeterProps {
  playing: boolean;
}

const CX = 110;
const CY = 102;
const R_ARC = 76;

function polar(r: number, angleDeg: number) {
  const rad = ((angleDeg - 90) * Math.PI) / 180;
  return {
    x: CX + r * Math.cos(rad),
    y: CY + r * Math.sin(rad),
  };
}

function arcPath(r: number, a1: number, a2: number) {
  const p1 = polar(r, a1);
  const p2 = polar(r, a2);
  return `M ${p1.x.toFixed(1)} ${p1.y.toFixed(1)} A ${r} ${r} 0 0 1 ${p2.x.toFixed(1)} ${p2.y.toFixed(1)}`;
}

const MAJOR_TICKS: { deg: number; label: string; red?: boolean }[] = [
  { deg: -44, label: "-20" },
  { deg: -30, label: "-10" },
  { deg: -18, label: "-7" },
  { deg: -8, label: "-5" },
  { deg: 2, label: "-3" },
  { deg: 12, label: "-1" },
  { deg: 22, label: "0", red: true },
  { deg: 33, label: "+2", red: true },
  { deg: 43, label: "+3", red: true },
];

const MINOR_DEGS = [-37, -24, -13, -3, 7, 17, 27.5, 38];

function VuDial({
  channel,
  needleRef,
  shadowRef,
}: {
  channel: "L" | "R";
  needleRef: RefObject<SVGGElement>;
  shadowRef: RefObject<SVGLineElement>;
}) {
  return (
    <div className="relative flex-1 min-w-0 h-[78px] rounded-lg overflow-hidden skeuo-vu-dial border border-black/25">
      {/* 曲面光学玻璃顶高光 */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          background:
            "linear-gradient(180deg, rgba(255,255,255,0.2) 0%, rgba(255,255,255,0.03) 26%, transparent 50%, rgba(0,0,0,0.14) 100%)",
        }}
      />
      <svg
        viewBox="0 0 220 82"
        preserveAspectRatio="xMidYMax meet"
        className="w-full h-full block"
      >
        {/* 主刻度底弧 (-20dB ~ 0dB) */}
        <path
          d={arcPath(R_ARC, -44, 22)}
          fill="none"
          stroke="currentColor"
          strokeWidth="1.2"
          opacity="0.68"
        />
        <path
          d={arcPath(R_ARC - 5.5, -44, 22)}
          fill="none"
          stroke="currentColor"
          strokeWidth="0.6"
          opacity="0.34"
        />
        {/* 0dB ~ +3dB 经典过载红区 */}
        <path
          d={arcPath(R_ARC, 22, 43)}
          fill="none"
          stroke="#dc2626"
          strokeWidth="3.2"
          opacity="0.92"
        />
        <path
          d={arcPath(R_ARC - 5.5, 22, 43)}
          fill="none"
          stroke="#dc2626"
          strokeWidth="0.9"
          opacity="0.58"
        />

        {/* 次刻度线 */}
        {MINOR_DEGS.map((deg) => {
          const p1 = polar(R_ARC - 3.5, deg);
          const p2 = polar(R_ARC + 1, deg);
          const red = deg > 22;
          return (
            <line
              key={deg}
              x1={p1.x}
              y1={p1.y}
              x2={p2.x}
              y2={p2.y}
              stroke={red ? "#dc2626" : "currentColor"}
              strokeWidth="0.85"
              opacity={red ? 0.82 : 0.48}
            />
          );
        })}

        {/* 主刻度线与大号读数 */}
        {MAJOR_TICKS.map(({ deg, label, red }) => {
          const p1 = polar(R_ARC - 5.5, deg);
          const p2 = polar(R_ARC + 3, deg);
          const pt = polar(R_ARC + 12.5, deg);
          return (
            <g key={label}>
              <line
                x1={p1.x}
                y1={p1.y}
                x2={p2.x}
                y2={p2.y}
                stroke={red ? "#dc2626" : "currentColor"}
                strokeWidth={red ? "1.35" : "1.1"}
                opacity={red ? 0.94 : 0.82}
              />
              <text
                x={pt.x}
                y={pt.y + 2.5}
                textAnchor="middle"
                fontSize="7.8"
                fontWeight="700"
                fontFamily="ui-monospace, Consolas, monospace"
                fill={red ? "#dc2626" : "currentColor"}
                opacity={red ? 0.95 : 0.85}
              >
                {label}
              </text>
            </g>
          );
        })}

        {/* 经典丝印标识 */}
        <text
          x={CX}
          y="53"
          textAnchor="middle"
          fontSize="12"
          fontWeight="900"
          letterSpacing="0.16em"
          fill="currentColor"
          opacity="0.78"
        >
          VU
        </text>
        <text
          x={CX}
          y="64"
          textAnchor="middle"
          fontSize="6.5"
          fontWeight="700"
          letterSpacing="0.22em"
          fill="currentColor"
          opacity="0.52"
        >
          CH · {channel}
        </text>

        {/* 表针投射阴影 */}
        <line
          ref={shadowRef}
          x1={CX + 1.8}
          y1={CY + 1}
          x2={CX + 1.8}
          y2={CY - R_ARC - 3}
          stroke="rgba(0,0,0,0.28)"
          strokeWidth="1.9"
          strokeLinecap="round"
          style={{
            transformOrigin: `${CX + 1.8}px ${CY + 1}px`,
            transform: "rotate(-44deg)",
          }}
        />

        {/* 动圈表针（带红针尖） */}
        <g
          ref={needleRef}
          style={{
            transformOrigin: `${CX}px ${CY}px`,
            transform: "rotate(-44deg)",
          }}
        >
          <line
            x1={CX}
            y1={CY}
            x2={CX}
            y2={CY - R_ARC - 3}
            stroke="#18130d"
            strokeWidth="1.6"
            strokeLinecap="round"
          />
          <line
            x1={CX}
            y1={CY - R_ARC + 10}
            x2={CX}
            y2={CY - R_ARC - 3}
            stroke="#dc2626"
            strokeWidth="1.6"
            strokeLinecap="round"
          />
        </g>

        {/* 底部半圆遮罩罩壳与枢轴帽 */}
        <path
          d={`M ${CX - 17} 82 A 17 17 0 0 1 ${CX + 17} 82 Z`}
          fill="#1b1f26"
          stroke="rgba(0,0,0,0.85)"
          strokeWidth="1"
        />
        <circle cx={CX} cy="81" r="4.8" fill="#3a414d" stroke="#0d0f13" strokeWidth="1" />
        <circle cx={CX} cy="81" r="1.7" fill="#d1d5db" />
      </svg>
    </div>
  );
}

export default function VUMeter({ playing }: VUMeterProps) {
  const { tr } = useI18n();
  const needleLRef = useRef<SVGGElement>(null);
  const shadowLRef = useRef<SVGLineElement>(null);
  const needleRRef = useRef<SVGGElement>(null);
  const shadowRRef = useRef<SVGLineElement>(null);
  const peakRef = useRef<HTMLDivElement>(null);
  const barLRefs = useRef<(HTMLDivElement | null)[]>([]);
  const barRRefs = useRef<(HTMLDivElement | null)[]>([]);

  const playingRef = useRef(playing);
  playingRef.current = playing;
  const wakeRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    if (playing) wakeRef.current?.();
  }, [playing]);

  useEffect(() => {
    let curL = 0;
    let curR = 0;
    // 慢速自适应响度参考（~2.2s 时间常数）：避免高响度流行副歌长期顶死在 +3dB，
    // 同时保留主歌轻柔 ↔ 副歌高潮的真实宏观动态对比
    let envRef = 0.45;
    let raf = 0;
    let prev = performance.now();
    let lastRotL = "";
    let lastRotR = "";
    let lastPeak = false;
    const lastBarL = new Array<boolean>(8).fill(false);
    const lastBarR = new Array<boolean>(8).fill(false);

    const loop = (now: number) => {
      raf = 0;
      if (document.hidden) {
        prev = now;
        if (playingRef.current) raf = requestAnimationFrame(loop);
        return;
      }
      const dt = Math.min(0.1, (now - prev) / 1000);
      prev = now;

      const { frame, age } = getFxFrame();
      const live = playingRef.current && age < 400;

      let targetL = 0;
      let targetR = 0;

      if (live) {
        const bands = frame.bands || [];
        const n = bands.length;
        // 用 36 段对数频谱（-62dB ~ -4dB，动态余量远大于被 rms*3.4 削波的 frame.level）
        // 分别提取低中频（鼓/贝斯/人声基频）与中高频（人声泛音/吉他/镲片）的瞬态能量
        let avgAll = 0;
        if (n > 0) {
          for (let i = 0; i < n; i++) avgAll += bands[i] || 0;
          avgAll /= n;
        }
        const lowMid =
          ((bands[1] || 0) + (bands[3] || 0) + (bands[5] || 0) + (bands[8] || 0) + (bands[11] || 0)) / 5;
        const midHi =
          ((bands[10] || 0) + (bands[14] || 0) + (bands[18] || 0) + (bands[22] || 0) + (bands[26] || 0)) / 5;

        const baseLevel = frame.level || 0;
        const rawL = avgAll * 0.42 + lowMid * 0.43 + baseLevel * 0.15;
        const rawR = avgAll * 0.42 + midHi * 0.43 + baseLevel * 0.15;
        const rawAvg = (rawL + rawR) * 0.5;

        // 慢速追踪当前段落响度（下限 0.32 防止静音/前奏被过度放大）
        const envK = 1 - Math.exp(-dt * 0.55);
        envRef += (Math.max(0.32, rawAvg) - envRef) * envK;

        // 混合「绝对响度」与「相对当前段落的动态起伏」：
        // 既能看出主歌轻、副歌强，又能在高响度副歌里随鼓点和重音清晰摆动
        const normL = rawL * 0.45 + (rawL / (envRef * 1.48)) * 0.55;
        const normR = rawR * 0.45 + (rawR / (envRef * 1.48)) * 0.55;

        // 软膝盖压缩（0dB 以上平滑压缩，仅在强瞬态爆发时冲进 +2~+3dB 红区，绝不削平顶死）
        const softClip = (v: number) => {
          const x = Math.max(0, v);
          if (x <= 0.74) return x;
          return 0.74 + 0.26 * Math.tanh((x - 0.74) / 0.32);
        };

        targetL = softClip(normL + (frame.beat ? 0.08 : 0));
        targetR = softClip(normR + (frame.beat ? 0.05 : 0));
      }

      // 物理动圈阻尼特性（快起慢落）
      const attackK = 1 - Math.exp(-dt * 20);
      const decayK = 1 - Math.exp(-dt * 7.5);

      curL += (targetL - curL) * (targetL > curL ? attackK : decayK);
      curR += (targetR - curR) * (targetR > curR ? attackK : decayK);
      if (!playingRef.current && curL < 0.002 && curR < 0.002) {
        curL = 0;
        curR = 0;
      }

      // 线性映射到表盘角度：-44deg (-20dB) ~ +22deg (0dB, ~0.76) ~ +43deg (+3dB, 1.0)
      const angleL = -44 + curL * 87;
      const angleR = -44 + curR * 87;

      const rotL = `rotate(${angleL.toFixed(1)}deg)`;
      const rotR = `rotate(${angleR.toFixed(1)}deg)`;
      if (rotL !== lastRotL) {
        lastRotL = rotL;
        if (needleLRef.current) needleLRef.current.style.transform = rotL;
        if (shadowLRef.current) shadowLRef.current.style.transform = rotL;
      }
      if (rotR !== lastRotR) {
        lastRotR = rotR;
        if (needleRRef.current) needleRRef.current.style.transform = rotR;
        if (shadowRRef.current) shadowRRef.current.style.transform = rotR;
      }

      // PEAK 过载红灯：仅在真正强瞬态突破 +1.5dB（>0.86）时闪亮
      const isPeak = curL > 0.86 || curR > 0.86;
      if (isPeak !== lastPeak && peakRef.current) {
        lastPeak = isPeak;
        if (isPeak) {
          peakRef.current.classList.add("skeuo-led-red");
          peakRef.current.classList.remove("skeuo-led-off");
        } else {
          peakRef.current.classList.remove("skeuo-led-red");
          peakRef.current.classList.add("skeuo-led-off");
        }
      }

      const step = 1 / 8;
      for (let i = 0; i < 8; i++) {
        const threshold = (i + 1) * step * 0.94;
        const onL = curL >= threshold;
        if (onL !== lastBarL[i]) {
          lastBarL[i] = onL;
          const elL = barLRefs.current[i];
          if (elL) elL.style.opacity = onL ? "1" : "0.14";
        }
        const onR = curR >= threshold;
        if (onR !== lastBarR[i]) {
          lastBarR[i] = onR;
          const elR = barRRefs.current[i];
          if (elR) elR.style.opacity = onR ? "1" : "0.14";
        }
      }

      // 暂停且指针已完全归零时停止 rAF，消除空闲 60fps 消耗
      if (playingRef.current || curL > 0 || curR > 0) {
        raf = requestAnimationFrame(loop);
      }
    };

    const wake = () => {
      if (!raf) {
        prev = performance.now();
        raf = requestAnimationFrame(loop);
      }
    };
    wakeRef.current = wake;
    const onVis = () => {
      if (!document.hidden && playingRef.current) wake();
    };
    document.addEventListener("visibilitychange", onVis);
    wake();
    return () => {
      wakeRef.current = null;
      document.removeEventListener("visibilitychange", onVis);
      if (raf) cancelAnimationFrame(raf);
    };
  }, []);

  return (
    <div className="skeuo-vu-frame rounded-xl px-3 py-2 flex items-center justify-between gap-3 select-none">
      <VuDial channel="L" needleRef={needleLRef} shadowRef={shadowLRef} />

      {/* 中置 Peak 与立体声 LED 阵列 */}
      <div className="flex flex-col items-center justify-center px-1 shrink-0 gap-1.5">
        <div className="flex items-center gap-1.5">
          <div
            ref={peakRef}
            className="skeuo-led-off"
            title={tr("信号过载指示", "Peak Overload Indicator")}
          />
          <span className="text-[8.5px] font-black tracking-[0.2em] text-[var(--ink-2)]">
            PEAK
          </span>
        </div>

        <div className="skeuo-well flex flex-col gap-1 px-2 py-1 rounded-md">
          <div className="flex items-center gap-1">
            <span className="text-[7px] font-mono font-bold text-[var(--ink-3)] w-2">L</span>
            <div className="flex gap-[2px]">
              {Array.from({ length: 8 }).map((_, i) => (
                <div
                  key={i}
                  ref={(el) => {
                    barLRefs.current[i] = el;
                  }}
                  className="w-1.5 h-2 rounded-[1px] transition-opacity duration-75"
                  style={{
                    background:
                      i >= 6 ? "#ef4444" : i >= 4 ? "#f59e0b" : "var(--accent-strong)",
                    opacity: 0.14,
                  }}
                />
              ))}
            </div>
          </div>
          <div className="flex items-center gap-1">
            <span className="text-[7px] font-mono font-bold text-[var(--ink-3)] w-2">R</span>
            <div className="flex gap-[2px]">
              {Array.from({ length: 8 }).map((_, i) => (
                <div
                  key={i}
                  ref={(el) => {
                    barRRefs.current[i] = el;
                  }}
                  className="w-1.5 h-2 rounded-[1px] transition-opacity duration-75"
                  style={{
                    background:
                      i >= 6 ? "#ef4444" : i >= 4 ? "#f59e0b" : "var(--accent-strong)",
                    opacity: 0.14,
                  }}
                />
              ))}
            </div>
          </div>
        </div>

        <span className="text-[7px] font-mono font-bold text-[var(--ink-3)] tracking-[0.2em] uppercase">
          REF · 0dB
        </span>
      </div>

      <VuDial channel="R" needleRef={needleRRef} shadowRef={shadowRRef} />
    </div>
  );
}
