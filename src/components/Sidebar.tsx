//! 侧栏分发器：按界面风格渲染对应布局。
//! 标准 / 液态玻璃共用 Classic 布局（浮岛玻璃卡片），拟物为「机架面板」专属布局。
import { useStore } from "../store";
import SidebarClassic from "./SidebarClassic";
import SidebarSkeuo from "./SidebarSkeuo";

export default function Sidebar() {
  const uiTheme = useStore((s) => s.uiTheme);
  return uiTheme === "skeuo" ? <SidebarSkeuo /> : <SidebarClassic />;
}
