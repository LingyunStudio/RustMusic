import { useEffect, useRef } from "react";
import { getFxFrame } from "../fx";

interface VUMeterProps {
  playing: boolean;
}

export default function VUMeter({ playing }: VUMeterProps) {
  const needleLRef = useRef<SVGLineElement>(null);
  const needleRRef = useRef<SVGLineElement>(null);
  const peakRef = useRef<HTMLDivElement>(null);
  const barLRefs = useRef<(HTMLDivElement | null)[]>([]);
  const barRRefs = useRef<(HTMLDivElement | null)[]>([]);

  useEffect(() => {
    let curL = 0;
    let curR = 0;
    let raf = 0;
    let prev = performance.now();

    const loop = (now: number) => {
      raf = requestAnimationFrame(loop);
      if (document.hidden) {
        prev = now;
        return;
      }
      const dt = Math.min(0.1, (now - prev) / 1000);
      prev = now;

      const { frame, age } = getFxFrame();
      const live = playing && age < 400;

      // 左通道：偏低频与总体电平；右通道：偏中高频与总体电平
      const bands = frame.bands || [];
      const lowEnergy = ((bands[1] || 0) + (bands[3] || 0) + (bands[5] || 0)) / 3;
      const hiEnergy = ((bands[14] || 0) + (bands[18] || 0) + (bands[22] || 0)) / 3;

      const targetL = live ? Math.min(1, Math.max(0, (frame.level || 0) * 0.72 + lowEnergy * 0.45)) : 0;
      const targetR = live ? Math.min(1, Math.max(0, (frame.level || 0) * 0.72 + hiEnergy * 0.45)) : 0;

      // 物理阻尼与弹簧回弹特性（快充慢放）
      const attackK = 1 - Math.exp(-dt * 22);
      const decayK = 1 - Math.exp(-dt * 9);

      curL += (targetL - curL) * (targetL > curL ? attackK : decayK);
      curR += (targetR - curR) * (targetR > curR ? attackK : decayK);

      // 表针角度映射：-38deg (-20dB) 至 +34deg (+3dB)
      const angleL = -38 + Math.pow(curL, 0.68) * 72;
      const angleR = -38 + Math.pow(curR, 0.68) * 72;

      if (needleLRef.current) {
        needleLRef.current.style.transform = `rotate(${angleL.toFixed(1)}deg)`;
      }
      if (needleRRef.current) {
        needleRRef.current.style.transform = `rotate(${angleR.toFixed(1)}deg)`;
      }

      // 过载 Peak 指示灯（电平突破 0.8 时点亮）
      if (peakRef.current) {
        if (curL > 0.8 || curR > 0.8) {
          peakRef.current.classList.add("skeuo-led-on");
          peakRef.current.classList.remove("skeuo-led-off");
        } else {
          peakRef.current.classList.remove("skeuo-led-on");
          peakRef.current.classList.add("skeuo-led-off");
        }
      }

      // 8 段微型立体声 LED 频谱条
      const step = 1 / 8;
      for (let i = 0; i < 8; i++) {
        const threshold = (i + 1) * step;
        const elL = barLRefs.current[i];
        if (elL) {
          elL.style.opacity = curL >= threshold ? "1" : "0.15";
        }
        const elR = barRRefs.current[i];
        if (elR) {
          elR.style.opacity = curR >= threshold ? "1" : "0.15";
        }
      }
    };

    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [playing]);

  return (
    <div className="skeuo-vu-frame rounded-xl px-4 py-2.5 flex items-center justify-between gap-3 select-none">
      {/* 左通道模拟 VU 表 */}
      <div className="relative flex-1 max-w-[150px] h-[58px] rounded-lg overflow-hidden skeuo-vu-dial border border-black/40">
        {/* 表盘玻璃反光高光 */}
        <div className="absolute inset-0 bg-gradient-to-b from-white/30 via-transparent to-black/25 pointer-events-none" />
        <svg
          viewBox="0 0 130 58"
          className="w-full h-full block"
          style={{ overflow: "visible" }}
        >
          {/* 刻度底弧线 */}
          <path
            d="M 16 48 A 54 54 0 0 1 114 48"
            fill="none"
            stroke="currentColor"
            strokeWidth="0.8"
            opacity="0.45"
          />
          {/* 0dB 至 +3dB 过载红弧 */}
          <path
            d="M 94 48 A 54 54 0 0 1 114 48"
            fill="none"
            stroke="#ef4444"
            strokeWidth="2"
            opacity="0.85"
          />

          {/* 刻度线与文字 */}
          <g fontSize="6" fontWeight="bold" fontFamily="monospace" opacity="0.85">
            <text x="18" y="44" textAnchor="middle">
              -20
            </text>
            <text x="36" y="34" textAnchor="middle">
              -10
            </text>
            <text x="54" y="27" textAnchor="middle">
              -5
            </text>
            <text x="74" y="26" textAnchor="middle">
              -1
            </text>
            <text x="94" y="29" textAnchor="middle" fill="#ef4444">
              0
            </text>
            <text x="112" y="38" textAnchor="middle" fill="#ef4444">
              +3
            </text>
          </g>

          {/* 标识 */}
          <text
            x="65"
            y="43"
            textAnchor="middle"
            fontSize="7"
            fontWeight="900"
            letterSpacing="0.1em"
            opacity="0.75"
          >
            VU
          </text>
          <text
            x="65"
            y="52"
            textAnchor="middle"
            fontSize="5.5"
            fontWeight="bold"
            letterSpacing="0.18em"
            opacity="0.5"
          >
            CH · L
          </text>

          {/* 动圈表针 */}
          <line
            ref={needleLRef}
            x1="65"
            y1="56"
            x2="65"
            y2="12"
            stroke="#1a1510"
            strokeWidth="1.2"
            strokeLinecap="round"
            style={{
              transformOrigin: "65px 56px",
              transform: "rotate(-38deg)",
              filter: "drop-shadow(0 1px 1px rgba(0,0,0,0.5))",
            }}
          />
          {/* 表针枢轴金属帽 */}
          <circle cx="65" cy="56" r="4.5" fill="#2d333b" stroke="#000" strokeWidth="0.8" />
          <circle cx="65" cy="56" r="1.5" fill="#9ca3af" />
        </svg>
      </div>

      {/* 中置发烧级状态仪表舱 */}
      <div className="flex flex-col items-center justify-center px-1.5 shrink-0">
        <div className="flex items-center gap-1.5 mb-1.5">
          <div ref={peakRef} className="skeuo-led-off" title="信号过载指示" />
          <span className="text-[9px] font-black tracking-widest text-[var(--ink-2)]">
            PEAK
          </span>
        </div>

        {/* 双声道微型 LED 柱 */}
        <div className="flex items-center gap-1 bg-black/40 px-2 py-1 rounded border border-white/10">
          <div className="flex gap-0.5">
            {Array.from({ length: 8 }).map((_, i) => (
              <div
                key={i}
                ref={(el) => {
                  barLRefs.current[i] = el;
                }}
                className="w-1 h-3 rounded-[1px] transition-opacity duration-75"
                style={{
                  background:
                    i >= 6 ? "#ef4444" : i >= 4 ? "#f59e0b" : "var(--accent-strong)",
                  opacity: 0.15,
                }}
              />
            ))}
          </div>
          <span className="text-[7.5px] font-mono text-[var(--ink-3)] font-bold px-0.5">
            LR
          </span>
          <div className="flex gap-0.5">
            {Array.from({ length: 8 }).map((_, i) => (
              <div
                key={i}
                ref={(el) => {
                  barRRefs.current[i] = el;
                }}
                className="w-1 h-3 rounded-[1px] transition-opacity duration-75"
                style={{
                  background:
                    i >= 6 ? "#ef4444" : i >= 4 ? "#f59e0b" : "var(--accent-strong)",
                  opacity: 0.15,
                }}
              />
            ))}
          </div>
        </div>

        <span className="text-[8px] font-bold text-[var(--ink-3)] tracking-[0.2em] mt-1 uppercase">
          Master Audio
        </span>
      </div>

      {/* 右通道模拟 VU 表 */}
      <div className="relative flex-1 max-w-[150px] h-[58px] rounded-lg overflow-hidden skeuo-vu-dial border border-black/40">
        {/* 表盘玻璃反光高光 */}
        <div className="absolute inset-0 bg-gradient-to-b from-white/30 via-transparent to-black/25 pointer-events-none" />
        <svg
          viewBox="0 0 130 58"
          className="w-full h-full block"
          style={{ overflow: "visible" }}
        >
          {/* 刻度底弧线 */}
          <path
            d="M 16 48 A 54 54 0 0 1 114 48"
            fill="none"
            stroke="currentColor"
            strokeWidth="0.8"
            opacity="0.45"
          />
          {/* 0dB 至 +3dB 过载红弧 */}
          <path
            d="M 94 48 A 54 54 0 0 1 114 48"
            fill="none"
            stroke="#ef4444"
            strokeWidth="2"
            opacity="0.85"
          />

          {/* 刻度线与文字 */}
          <g fontSize="6" fontWeight="bold" fontFamily="monospace" opacity="0.85">
            <text x="18" y="44" textAnchor="middle">
              -20
            </text>
            <text x="36" y="34" textAnchor="middle">
              -10
            </text>
            <text x="54" y="27" textAnchor="middle">
              -5
            </text>
            <text x="74" y="26" textAnchor="middle">
              -1
            </text>
            <text x="94" y="29" textAnchor="middle" fill="#ef4444">
              0
            </text>
            <text x="112" y="38" textAnchor="middle" fill="#ef4444">
              +3
            </text>
          </g>

          {/* 标识 */}
          <text
            x="65"
            y="43"
            textAnchor="middle"
            fontSize="7"
            fontWeight="900"
            letterSpacing="0.1em"
            opacity="0.75"
          >
            VU
          </text>
          <text
            x="65"
            y="52"
            textAnchor="middle"
            fontSize="5.5"
            fontWeight="bold"
            letterSpacing="0.18em"
            opacity="0.5"
          >
            CH · R
          </text>

          {/* 动圈表针 */}
          <line
            ref={needleRRef}
            x1="65"
            y1="56"
            x2="65"
            y2="12"
            stroke="#1a1510"
            strokeWidth="1.2"
            strokeLinecap="round"
            style={{
              transformOrigin: "65px 56px",
              transform: "rotate(-38deg)",
              filter: "drop-shadow(0 1px 1px rgba(0,0,0,0.5))",
            }}
          />
          {/* 表针枢轴金属帽 */}
          <circle cx="65" cy="56" r="4.5" fill="#2d333b" stroke="#000" strokeWidth="0.8" />
          <circle cx="65" cy="56" r="1.5" fill="#9ca3af" />
        </svg>
      </div>
    </div>
  );
}
