//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import TitlebarStandard from "./Titlebar.standard";
import TitlebarSkeuo from "./Titlebar.skeuo";
import TitlebarLiquid from "./Titlebar.liquid";
import { useStore } from "../store";

export default function Titlebar() {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <TitlebarLiquid />;
  if (uiTheme === "skeuo") return <TitlebarSkeuo />;
  return <TitlebarStandard />;
}
