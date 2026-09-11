// RustMusic 应用图标 —— 手绘 SVG 母图
//
// 设计语言（与应用"液态玻璃 + 暖调"主题一致）：
//   - 暖黑超圆角玻璃底，左上受光、下缘反光，1px 内描边
//   - 黑胶唱片：同心刻纹 + 右下弧光（vinyl 经典高光）
//   - 琥珀色中心标签 + 白色音符（Material Symbols 音符字形路径）
//   - 32px 以下自动降级：整图简化为"深底 + 琥珀圆点"仍可识别
const AMBER = "#f0a24a";
const AMBER_DEEP = "#e28a2e";
const LABEL_TOP = "#f7bc72";
const LABEL_BOT = "#e2892c";
const VINYL = "#3d2f27";
const VINYL_EDGE = "#241b16";

const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs>
    <!-- 暖黑垂直渐变底 -->
    <linearGradient id="bg" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#261e18"/>
      <stop offset="1" stop-color="#15100c"/>
    </linearGradient>
    <!-- 左上柔和高光 -->
    <radialGradient id="gloss" cx="0.22" cy="0.16" r="0.9">
      <stop offset="0" stop-color="#ffffff" stop-opacity="0.13"/>
      <stop offset="0.55" stop-color="#ffffff" stop-opacity="0.04"/>
      <stop offset="1" stop-color="#ffffff" stop-opacity="0"/>
    </radialGradient>
    <!-- 唱片面：上亮下暗微渐变 -->
    <linearGradient id="vinylFace" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#463830"/>
      <stop offset="1" stop-color="${VINYL_EDGE}"/>
    </linearGradient>
    <!-- 标签琥珀渐变 -->
    <linearGradient id="label" x1="0" y1="0" x2="0.35" y2="1">
      <stop offset="0" stop-color="${LABEL_TOP}"/>
      <stop offset="1" stop-color="${LABEL_BOT}"/>
    </linearGradient>
    <!-- 弧光渐变（右下高光） -->
    <linearGradient id="arcGlow" x1="0" y1="0" x2="0.7" y2="0.9">
      <stop offset="0" stop-color="#ffe9c4" stop-opacity="0.55"/>
      <stop offset="0.5" stop-color="#ffe9c4" stop-opacity="0.25"/>
      <stop offset="1" stop-color="#ffe9c4" stop-opacity="0"/>
    </linearGradient>
  </defs>

  <!-- 底：超圆角方形（92% 占宽，圆角 26%） -->
  <rect x="41" y="41" width="942" height="942" rx="245" fill="url(#bg)"/>
  <rect x="41" y="41" width="942" height="942" rx="245" fill="url(#gloss)"/>
  <!-- 玻璃内描边 -->
  <rect x="41" y="41" width="942" height="942" rx="245" fill="none"
        stroke="#ffe8d0" stroke-opacity="0.16" stroke-width="5"/>

  <!-- 唱片：半径 343 -->
  <g>
    <circle cx="512" cy="512" r="343" fill="url(#vinylFace)"/>
    <!-- 外缘暗环 -->
    <circle cx="512" cy="512" r="343" fill="none" stroke="${VINYL_EDGE}" stroke-width="14"/>
    <!-- 同心刻纹：从外 92% 到内 42%，内密外疏 -->
    ${Array.from({ length: 11 }, (_, i) => {
      const f = i / 10; // 0 外 … 1 内
      const rr = 343 * (0.92 - f * 0.50);
      const op = (0.30 * (1 - f) + 0.10).toFixed(2);
      return `<circle cx="512" cy="512" r="${rr.toFixed(0)}" fill="none" stroke="#ffe3c0" stroke-opacity="${op}" stroke-width="${f < 0.5 ? 3 : 2}"/>`;
    }).join("\n    ")}
    <!-- 右下弧光（黑胶反光）：以唱片中心为圆心的标准弧 r=300，
         θ=10°→80°（y 向下坐标系顺时针），完全贴唱片内缘 -->
    <path d="M ${(512 + 300 * Math.cos(0.1745)).toFixed(1)} ${(512 + 300 * Math.sin(0.1745)).toFixed(1)} A 300 300 0 0 1 ${(512 + 300 * Math.cos(1.3963)).toFixed(1)} ${(512 + 300 * Math.sin(1.3963)).toFixed(1)}"
          fill="none" stroke="url(#arcGlow)" stroke-width="26" stroke-linecap="round"
          opacity="0.85"/>
  </g>

  <!-- 中心标签：半径 138 -->
  <circle cx="512" cy="512" r="138" fill="url(#label)"/>
  <circle cx="512" cy="512" r="138" fill="none" stroke="#fff4de" stroke-opacity="0.7" stroke-width="4"/>

  <!-- 连杆双八分音符 ♫（双符头 + 双符杆 + 顶部横梁）：
       区别于单音符形（QQ音乐 / Apple Music）；宽 209 / 高 183，占标签直径 ~76% -->
  <g transform="translate(512,512)">
    <ellipse cx="-70" cy="68" rx="35" ry="25" transform="rotate(-18, -70, 68)" fill="#fffaf2"/>
    <ellipse cx="70" cy="68" rx="35" ry="25" transform="rotate(-18, 70, 68)" fill="#fffaf2"/>
    <rect x="-51" y="-90" width="16" height="158" fill="#fffaf2"/>
    <rect x="35" y="-90" width="16" height="158" fill="#fffaf2"/>
    <polygon points="-51,-90 51,-90 51,-57 -51,-57" fill="#fffaf2"/>
  </g>
</svg>`;

require("fs").writeFileSync("app-icon-new.svg", svg);
console.log("svg written: app-icon-new.svg");
