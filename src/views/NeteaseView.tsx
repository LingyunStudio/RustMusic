//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import NeteaseViewStandard from "./NeteaseView.standard";
import NeteaseViewSkeuo from "./NeteaseView.skeuo";
import NeteaseViewLiquid from "./NeteaseView.liquid";
import { useStore } from "../store";

export default function NeteaseView(props: Parameters<typeof NeteaseViewStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <NeteaseViewLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <NeteaseViewSkeuo {...(props as any)} />;
  return <NeteaseViewStandard {...(props as any)} />;
}
