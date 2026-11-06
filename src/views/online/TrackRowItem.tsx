//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import { TrackRowItem as TrackRowItemStandard } from "./TrackRowItem.standard";
import { TrackRowItem as TrackRowItemSkeuo } from "./TrackRowItem.skeuo";
import { TrackRowItem as TrackRowItemLiquid } from "./TrackRowItem.liquid";
import { useStore } from "../../store";

export function TrackRowItem(props: Parameters<typeof TrackRowItemStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <TrackRowItemLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <TrackRowItemSkeuo {...(props as any)} />;
  return <TrackRowItemStandard {...(props as any)} />;
}
