//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import NowPlayingStandard from "./NowPlaying.standard";
import NowPlayingSkeuo from "./NowPlaying.skeuo";
import NowPlayingLiquid from "./NowPlaying.liquid";
import { useStore } from "../store";

export default function NowPlaying() {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <NowPlayingLiquid />;
  if (uiTheme === "skeuo") return <NowPlayingSkeuo />;
  return <NowPlayingStandard />;
}
