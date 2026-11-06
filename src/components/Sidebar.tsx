//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import SidebarStandard from "./Sidebar.standard";
import SidebarSkeuo from "./Sidebar.skeuo";
import SidebarLiquid from "./Sidebar.liquid";
import { useStore } from "../store";

export default function Sidebar() {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <SidebarLiquid />;
  if (uiTheme === "skeuo") return <SidebarSkeuo />;
  return <SidebarStandard />;
}
