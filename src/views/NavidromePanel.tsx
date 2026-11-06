//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import NavidromePanelStandard from "./NavidromePanel.standard";
import NavidromePanelSkeuo from "./NavidromePanel.skeuo";
import NavidromePanelLiquid from "./NavidromePanel.liquid";
import { useStore } from "../store";

export default function NavidromePanel() {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <NavidromePanelLiquid />;
  if (uiTheme === "skeuo") return <NavidromePanelSkeuo />;
  return <NavidromePanelStandard />;
}
