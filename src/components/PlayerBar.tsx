//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import PlayerBarStandard from "./PlayerBar.standard";
import PlayerBarSkeuo from "./PlayerBar.skeuo";
import PlayerBarLiquid from "./PlayerBar.liquid";
import { useStore } from "../store";

export default function PlayerBar(props: Parameters<typeof PlayerBarStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <PlayerBarLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <PlayerBarSkeuo {...(props as any)} />;
  return <PlayerBarStandard {...(props as any)} />;
}
