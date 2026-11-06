//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import LibraryViewStandard from "./LibraryView.standard";
import LibraryViewSkeuo from "./LibraryView.skeuo";
import LibraryViewLiquid from "./LibraryView.liquid";
import { useStore } from "../store";

export default function LibraryView(props: Parameters<typeof LibraryViewStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <LibraryViewLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <LibraryViewSkeuo {...(props as any)} />;
  return <LibraryViewStandard {...(props as any)} />;
}
