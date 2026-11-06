//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import LocateCurrentPillStandard from "./LocateCurrentPill.standard";
import LocateCurrentPillSkeuo from "./LocateCurrentPill.skeuo";
import LocateCurrentPillLiquid from "./LocateCurrentPill.liquid";
import { useStore } from "../store";

export default function LocateCurrentPill(props: Parameters<typeof LocateCurrentPillStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <LocateCurrentPillLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <LocateCurrentPillSkeuo {...(props as any)} />;
  return <LocateCurrentPillStandard {...(props as any)} />;
}
