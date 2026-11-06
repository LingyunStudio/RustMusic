//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import SettingsViewStandard from "./SettingsView.standard";
import SettingsViewSkeuo from "./SettingsView.skeuo";
import SettingsViewLiquid from "./SettingsView.liquid";
import { useStore } from "../store";

export default function SettingsView() {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <SettingsViewLiquid />;
  if (uiTheme === "skeuo") return <SettingsViewSkeuo />;
  return <SettingsViewStandard />;
}
