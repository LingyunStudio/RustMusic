//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import SliderStandard from "./Slider.standard";
import SliderSkeuo from "./Slider.skeuo";
import SliderLiquid from "./Slider.liquid";
import { useStore } from "../store";

export default function Slider(props: Parameters<typeof SliderStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <SliderLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <SliderSkeuo {...(props as any)} />;
  return <SliderStandard {...(props as any)} />;
}
