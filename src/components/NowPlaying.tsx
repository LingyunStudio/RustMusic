//! 播放页分发器：按界面风格渲染对应布局。
//! 标准 / 液态玻璃共用 Classic 布局（液态玻璃的装饰类在标准风格下惰性），
//! 拟物为「直驱黑胶唱机工作台」专属布局（唱臂 / VU 表 / 机架铭牌）。
import { useStore } from "../store";
import NowPlayingClassic from "./NowPlayingClassic";
import NowPlayingSkeuo from "./NowPlayingSkeuo";

export default function NowPlaying() {
  const uiTheme = useStore((s) => s.uiTheme);
  return uiTheme === "skeuo" ? <NowPlayingSkeuo /> : <NowPlayingClassic />;
}
