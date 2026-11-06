//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import SkinPickerStandard from "./SkinPicker.standard";
import SkinPickerSkeuo from "./SkinPicker.skeuo";
import SkinPickerLiquid from "./SkinPicker.liquid";
import { useStore } from "../store";

export default function SkinPicker(props: Parameters<typeof SkinPickerStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <SkinPickerLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <SkinPickerSkeuo {...(props as any)} />;
  return <SkinPickerStandard {...(props as any)} />;
}
