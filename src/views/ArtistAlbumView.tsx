//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import ArtistAlbumViewStandard from "./ArtistAlbumView.standard";
import ArtistAlbumViewSkeuo from "./ArtistAlbumView.skeuo";
import ArtistAlbumViewLiquid from "./ArtistAlbumView.liquid";
import { useStore } from "../store";

export default function ArtistAlbumView(props: Parameters<typeof ArtistAlbumViewStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <ArtistAlbumViewLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <ArtistAlbumViewSkeuo {...(props as any)} />;
  return <ArtistAlbumViewStandard {...(props as any)} />;
}
