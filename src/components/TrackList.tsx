//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import TrackListStandard from "./TrackList.standard";
import TrackListSkeuo from "./TrackList.skeuo";
import TrackListLiquid from "./TrackList.liquid";
import { useStore } from "../store";
export type { SortKey } from "./TrackList.standard";

export default function TrackList(props: Parameters<typeof TrackListStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <TrackListLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <TrackListSkeuo {...(props as any)} />;
  return <TrackListStandard {...(props as any)} />;
}
