interface TonearmProps {
  playing: boolean;
  className?: string;
}

/**
 * 响应式高精度 S 型镀铬唱臂（百分比自适应唱机台面尺寸，严丝合缝不溢出）：
 * - 停机时稳稳落在右侧唱臂架上（rotate(0deg)）
 * - 播放时优雅旋入黑胶沟槽（rotate(25deg)）
 */
export default function Tonearm({ playing, className = "" }: TonearmProps) {
  return (
    <div className={`pointer-events-none select-none ${className}`}>
      <svg
        viewBox="0 0 116 250"
        className="w-full h-full block"
        style={{ overflow: "visible" }}
      >
        <defs>
          <linearGradient id="armChrome" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#6f7682" />
            <stop offset="28%" stopColor="#ffffff" />
            <stop offset="56%" stopColor="#d4dbe4" />
            <stop offset="82%" stopColor="#5a616d" />
            <stop offset="100%" stopColor="#262a32" />
          </linearGradient>

          <linearGradient id="counterWeight" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#2a2e36" />
            <stop offset="28%" stopColor="#a7afb9" />
            <stop offset="52%" stopColor="#eef1f5" />
            <stop offset="76%" stopColor="#525864" />
            <stop offset="100%" stopColor="#1b1e24" />
          </linearGradient>

          <radialGradient id="gimbalOuter" cx="36%" cy="32%" r="68%">
            <stop offset="0%" stopColor="#f5f7fa" />
            <stop offset="38%" stopColor="#9aa3b0" />
            <stop offset="78%" stopColor="#3c424d" />
            <stop offset="100%" stopColor="#14171c" />
          </radialGradient>

          <linearGradient id="brassAccent" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#fbe6b2" />
            <stop offset="48%" stopColor="#c99c36" />
            <stop offset="100%" stopColor="#6b4e11" />
          </linearGradient>
        </defs>

        {/* 固定底座：CNC 万向节外座 + 防滑旋钮 + 唱臂停靠架 */}
        <g id="armRestAndBase">
          <circle cx="66" cy="48" r="28" fill="rgba(0,0,0,0.42)" />
          <circle
            cx="66"
            cy="48"
            r="26"
            fill="url(#gimbalOuter)"
            stroke="#0e1014"
            strokeWidth="1.4"
            filter="drop-shadow(0 4px 8px rgba(0,0,0,0.62))"
          />
          <circle cx="66" cy="48" r="21" fill="#181b21" stroke="#08090c" strokeWidth="1.1" />
          <circle
            cx="66"
            cy="48"
            r="16.5"
            fill="none"
            stroke="rgba(255,255,255,0.28)"
            strokeWidth="0.9"
            strokeDasharray="1.8,2.8"
          />
          {/* 防滑偏压微调旋钮 */}
          <circle
            cx="91"
            cy="64"
            r="5.2"
            fill="url(#gimbalOuter)"
            stroke="#0e1014"
            strokeWidth="1"
            filter="drop-shadow(0 2px 3px rgba(0,0,0,0.55))"
          />
          <line x1="91" y1="60.5" x2="91" y2="64" stroke="#111" strokeWidth="1" />

          {/* 唱臂停靠卡扣架 */}
          <rect
            x="67.5"
            y="84"
            width="4.5"
            height="26"
            rx="2"
            fill="url(#counterWeight)"
            stroke="#0e1014"
            strokeWidth="0.8"
          />
          <path
            d="M 63.5 109 L 75.5 109"
            stroke="#b5bcc7"
            strokeWidth="3"
            strokeLinecap="round"
          />
        </g>

        {/* 旋转唱臂总成（以枢轴 (66, 48) 为圆心） */}
        <g
          id="armAssembly"
          style={{
            transformOrigin: "66px 48px",
            transform: playing ? "rotate(25deg)" : "rotate(0deg)",
            transition: "transform 0.85s cubic-bezier(0.22, 1, 0.36, 1)",
          }}
        >
          {/* 后配重连杆 */}
          <rect x="64" y="29" width="4" height="14" fill="url(#armChrome)" />

          {/* 滚花金属配重砝码 */}
          <rect
            x="53"
            y="10"
            width="26"
            height="20"
            rx="3"
            fill="url(#counterWeight)"
            stroke="#0a0c10"
            strokeWidth="1.2"
            filter="drop-shadow(0 3px 5px rgba(0,0,0,0.62))"
          />
          <g stroke="rgba(0,0,0,0.28)" strokeWidth="0.7">
            <line x1="57" y1="12" x2="57" y2="28" />
            <line x1="61" y1="12" x2="61" y2="28" />
            <line x1="65" y1="12" x2="65" y2="28" />
            <line x1="69" y1="12" x2="69" y2="28" />
            <line x1="73" y1="12" x2="73" y2="28" />
          </g>
          <rect x="53" y="25.5" width="26" height="3.2" fill="url(#brassAccent)" opacity="0.85" />

          {/* 万向节金属桥架与中心轴承帽 */}
          <rect
            x="54"
            y="44"
            width="24"
            height="8"
            rx="2"
            fill="url(#counterWeight)"
            stroke="#0b0d11"
            strokeWidth="0.9"
          />
          <circle
            cx="66"
            cy="48"
            r="10"
            fill="url(#gimbalOuter)"
            stroke="#090b0f"
            strokeWidth="1.2"
            filter="drop-shadow(0 2px 4px rgba(0,0,0,0.68))"
          />
          <circle cx="66" cy="48" r="4.8" fill="#181c22" stroke="rgba(255,255,255,0.2)" strokeWidth="0.6" />
          <circle cx="66" cy="48" r="2" fill="url(#brassAccent)" />

          {/* 精工 S 型镀铬唱臂管 */}
          <path
            d="M 66 58 L 66 106 C 66 138, 54 166, 49 194 L 45 216"
            fill="none"
            stroke="rgba(0,0,0,0.42)"
            strokeWidth="5.2"
            strokeLinecap="round"
            transform="translate(2.5, 3)"
          />
          <path
            d="M 66 58 L 66 106 C 66 138, 54 166, 49 194 L 45 216"
            fill="none"
            stroke="url(#armChrome)"
            strokeWidth="4.2"
            strokeLinecap="round"
          />
          <path
            d="M 65.2 60 L 65.2 106 C 65.2 138, 53.2 166, 48.2 194 L 44.2 215"
            fill="none"
            stroke="rgba(255,255,255,0.65)"
            strokeWidth="0.9"
            strokeLinecap="round"
          />

          {/* 镀铬卡口锁紧环 */}
          <rect
            x="41.5"
            y="213"
            width="7"
            height="5.2"
            rx="1.2"
            fill="url(#counterWeight)"
            stroke="#0a0c10"
            strokeWidth="0.7"
            transform="rotate(9 45 216)"
          />

          {/* SME 风格镂空铝唱头壳 */}
          <g filter="drop-shadow(1px 4px 6px rgba(0,0,0,0.72))">
            <path
              d="M 46.5 217 L 40 236 L 31.5 234 L 38 215 Z"
              fill="#1b1f27"
              stroke="#0a0c10"
              strokeWidth="0.9"
            />
            <circle cx="41.2" cy="221" r="1.05" fill="#0b0d11" />
            <circle cx="39.3" cy="226.5" r="1.05" fill="#0b0d11" />
            <circle cx="37.4" cy="232" r="1.05" fill="#0b0d11" />
            <line x1="44.2" y1="218.5" x2="38.8" y2="234" stroke="url(#brassAccent)" strokeWidth="1" />
            <path
              d="M 38 225 C 30.5 223.5, 27 228, 28.5 233"
              fill="none"
              stroke="#cbd2dc"
              strokeWidth="1.4"
              strokeLinecap="round"
            />
            <rect
              x="30.5"
              y="233"
              width="8.5"
              height="6.2"
              rx="1.2"
              fill="#b91c1c"
              stroke="#450a0a"
              strokeWidth="0.7"
              transform="rotate(14 34.5 236)"
            />
            <circle cx="33.8" cy="240.5" r="1.2" fill="#fef08a" />
          </g>
        </g>
      </svg>
    </div>
  );
}
