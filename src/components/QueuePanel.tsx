//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import QueuePanelStandard from "./QueuePanel.standard";
import QueuePanelSkeuo from "./QueuePanel.skeuo";
import QueuePanelLiquid from "./QueuePanel.liquid";
import { useStore } from "../store";

export default function QueuePanel() {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <QueuePanelLiquid />;
  if (uiTheme === "skeuo") return <QueuePanelSkeuo />;
  return <QueuePanelStandard />;
}
