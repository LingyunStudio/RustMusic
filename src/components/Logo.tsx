/**
 * RustMusic 品牌标识（与桌面/托盘图标同构：中性暗色圆角贴片 + 彩虹六色声波条，
 * 呼应 10 段均衡器）。彩虹为固定品牌色，不随主题强调色变化，任意尺寸矢量清晰。
 */

// 六根条：x / y / 高（viewBox 64，与 app-icon-new.svg 按 16:1 等比对应）
const BARS = [
  { x: 14.375, y: 25.75, h: 12.5 },
  { x: 20.625, y: 22.3125, h: 19.375 },
  { x: 26.875, y: 18.25, h: 27.5 },
  { x: 33.125, y: 18.5625, h: 26.875 },
  { x: 39.375, y: 22.625, h: 18.75 },
  { x: 45.625, y: 26.0625, h: 11.875 },
];

// 彩虹六色：每根条上亮下沉的同色系渐变（红 橙 黄 绿 蓝 紫）
const RAINBOW: [string, string][] = [
  ["#ff9a8a", "#f0563f"],
  ["#ffb45e", "#f07f1e"],
  ["#ffe08a", "#f0b429"],
  ["#7fd88f", "#2f9e5f"],
  ["#7ab3f0", "#2f6fd4"],
  ["#b39bf0", "#7d55d4"],
];

export default function Logo({ size = 44 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 64 64"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className="shrink-0 drop-shadow-[0_6px_16px_rgba(0,0,0,0.4)]"
    >
      <defs>
        <linearGradient id="rm-tile" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#262230" />
          <stop offset="1" stopColor="#120f16" />
        </linearGradient>
        {RAINBOW.map(([top, bot], i) => (
          <linearGradient key={i} id={`rm-bar-${i}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor={top} />
            <stop offset="1" stopColor={bot} />
          </linearGradient>
        ))}
      </defs>

      {/* 圆角贴片 */}
      <rect x="2.56" y="2.56" width="58.88" height="58.88" rx="15.31" fill="url(#rm-tile)" />

      {/* 彩虹声波条 */}
      {BARS.map((b, i) => (
        <rect key={i} x={b.x} y={b.y} width="4" height={b.h} rx="2" fill={`url(#rm-bar-${i})`} />
      ))}
    </svg>
  );
}
