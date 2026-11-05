//! 底部播放条分发器：按界面风格渲染对应布局。
//! 标准 / 液态玻璃共用 Classic 布局，拟物为「机架音响面板」专属布局。
import { useStore } from "../store";
import PlayerBarClassic from "./PlayerBarClassic";
import PlayerBarSkeuo from "./PlayerBarSkeuo";

export default function PlayerBar({ centered = false }: { centered?: boolean }) {
  const uiTheme = useStore((s) => s.uiTheme);
  return uiTheme === "skeuo" ? (
    <PlayerBarSkeuo centered={centered} />
  ) : (
    <PlayerBarClassic centered={centered} />
  );
}
