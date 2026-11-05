interface TonearmProps {
  playing: boolean;
  className?: string;
}

export default function Tonearm({ playing, className = "" }: TonearmProps) {
  return (
    <div
      className={`pointer-events-none select-none ${className}`}
      style={{
        width: 105,
        height: 250,
      }}
    >
      <svg
        viewBox="0 0 105 250"
        className="w-full h-full"
        style={{ overflow: "visible" }}
      >
        <defs>
          {/* 金属拉丝唱臂高光渐变 */}
          <linearGradient id="armChrome" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#8d95a0" />
            <stop offset="35%" stopColor="#ffffff" />
            <stop offset="65%" stopColor="#c8d0db" />
            <stop offset="100%" stopColor="#4c535e" />
          </linearGradient>

          {/* 唱臂配重砝码金属质感 */}
          <linearGradient id="counterWeight" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#404650" />
            <stop offset="40%" stopColor="#9ca5b2" />
            <stop offset="70%" stopColor="#252a32" />
            <stop offset="100%" stopColor="#14171d" />
          </linearGradient>

          {/* 万向万向轴承基座 */}
          <radialGradient id="gimbalBase" cx="40%" cy="35%" r="65%">
            <stop offset="0%" stopColor="#f3f4f6" />
            <stop offset="45%" stopColor="#9ca3af" />
            <stop offset="85%" stopColor="#374151" />
            <stop offset="100%" stopColor="#111827" />
          </radialGradient>
        </defs>

        {/* 唱臂底座支撑座与唱臂夹（固定不随唱臂旋转） */}
        <g id="armRestAndBase">
          {/* 外围金属固定座 */}
          <circle
            cx="65"
            cy="45"
            r="26"
            fill="url(#gimbalBase)"
            stroke="#111"
            strokeWidth="1.5"
            filter="drop-shadow(0 3px 6px rgba(0,0,0,0.7))"
          />
          <circle cx="65" cy="45" r="21" fill="#1e242d" stroke="#000" strokeWidth="1" />
          {/* 防滑控制刻度环 */}
          <circle
            cx="65"
            cy="45"
            r="16"
            fill="none"
            stroke="rgba(255,255,255,0.25)"
            strokeWidth="1"
            strokeDasharray="2,3"
          />

          {/* 唱臂抬升杆 / 唱臂架 (Arm Rest Clip) */}
          <path
            d="M 68 85 L 68 115"
            stroke="#4b5563"
            strokeWidth="3.5"
            strokeLinecap="round"
          />
          <path
            d="M 64 115 L 74 115"
            stroke="#9ca3af"
            strokeWidth="3"
            strokeLinecap="round"
          />
        </g>

        {/* 旋转主体：以枢轴 (65, 45) 为原点旋转 */}
        <g
          id="armAssembly"
          style={{
            transformOrigin: "65px 45px",
            transform: playing ? "rotate(23deg)" : "rotate(0deg)",
            transition: "transform 0.88s cubic-bezier(0.25, 1, 0.35, 1)",
          }}
        >
          {/* 配重砝码（位于枢轴后上方） */}
          <rect
            x="53"
            y="14"
            width="24"
            height="18"
            rx="2.5"
            fill="url(#counterWeight)"
            stroke="#0b0d11"
            strokeWidth="1.2"
            filter="drop-shadow(0 2px 4px rgba(0,0,0,0.6))"
          />
          {/* 砝码环形刻度细线 */}
          <line x1="53" y1="23" x2="77" y2="23" stroke="#fff" strokeWidth="0.8" opacity="0.6" />
          <line x1="61" y1="14" x2="61" y2="32" stroke="#000" strokeWidth="0.8" opacity="0.5" />

          {/* 万向万向轴承球头金属帽 */}
          <circle
            cx="65"
            cy="45"
            r="11"
            fill="url(#gimbalBase)"
            stroke="#000"
            strokeWidth="1.2"
            filter="drop-shadow(0 2px 4px rgba(0,0,0,0.65))"
          />
          <circle cx="65" cy="45" r="5" fill="#1f2937" />
          <circle cx="65" cy="45" r="2" fill="#d1d5db" />

          {/* 精工弯管唱臂杆 (Classic S-Wand) */}
          <path
            d="M 65 45 L 65 105 C 65 140, 52 170, 48 198 L 45 224"
            fill="none"
            stroke="url(#armChrome)"
            strokeWidth="4"
            strokeLinecap="round"
            filter="drop-shadow(2px 3px 4px rgba(0,0,0,0.45))"
          />

          {/* 唱头接插件卡口螺母 */}
          <rect
            x="42"
            y="222"
            width="6"
            height="5"
            rx="1"
            fill="#374151"
            stroke="#000"
            strokeWidth="0.6"
          />

          {/* 唱头壳 (Cartridge Headshell) */}
          <g filter="drop-shadow(0 3px 6px rgba(0,0,0,0.75))">
            {/* 唱头壳主体 */}
            <path
              d="M 45 226 L 39 242 L 34 242 L 40 226 Z"
              fill="#181d24"
              stroke="#0b0e12"
              strokeWidth="0.8"
            />
            {/* 顶盖金属铭牌 */}
            <line x1="43" y1="228" x2="38" y2="240" stroke="#f59e0b" strokeWidth="1" />
            {/* 唱头壳指拨把手 (Finger Lift) */}
            <path
              d="M 39 235 C 33 234, 30 238, 31 242"
              fill="none"
              stroke="#9ca3af"
              strokeWidth="1.2"
              strokeLinecap="round"
            />
            {/* 唱头本体 */}
            <rect
              x="33"
              y="240"
              width="6.5"
              height="6"
              rx="1"
              fill="#dc2626"
              stroke="#7f1d1d"
              strokeWidth="0.6"
            />
            {/* 红宝石/钻石唱针尖点 */}
            <circle cx="36" cy="246" r="1" fill="#fde047" />
          </g>
        </g>
      </svg>
    </div>
  );
}
