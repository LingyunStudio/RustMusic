import { useEffect, useState } from "react";
import { Music2 } from "lucide-react";
import { coverSrc } from "../api";
import { gradientFor } from "../utils";

interface CoverImgProps {
  src?: string;
  seed: string;
  className?: string;
  iconSize?: number;
}

export default function CoverImg({ src, seed, className = "", iconSize = 18 }: CoverImgProps) {
  const [err, setErr] = useState(false);
  useEffect(() => setErr(false), [src]);

  const url = src ? coverSrc(src) : "";
  if (url && !err) {
    return (
      <img
        src={url}
        alt=""
        draggable={false}
        onError={() => setErr(true)}
        className={`object-cover bg-white/5 ${className}`}
      />
    );
  }
  return (
    <div
      className={`flex items-center justify-center text-white/40 ${className}`}
      style={{ background: gradientFor(seed) }}
    >
      <Music2 size={iconSize} strokeWidth={1.6} />
    </div>
  );
}
