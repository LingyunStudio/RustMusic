//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import PlaylistDetailStandard from "./PlaylistDetail.standard";
import PlaylistDetailSkeuo from "./PlaylistDetail.skeuo";
import PlaylistDetailLiquid from "./PlaylistDetail.liquid";
import { useStore } from "../store";

export default function PlaylistDetail(props: Parameters<typeof PlaylistDetailStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <PlaylistDetailLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <PlaylistDetailSkeuo {...(props as any)} />;
  return <PlaylistDetailStandard {...(props as any)} />;
}
