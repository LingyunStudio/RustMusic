//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import SourcesViewStandard from "./SourcesView.standard";
import SourcesViewSkeuo from "./SourcesView.skeuo";
import SourcesViewLiquid from "./SourcesView.liquid";
import { useStore } from "../store";

export default function SourcesView() {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <SourcesViewLiquid />;
  if (uiTheme === "skeuo") return <SourcesViewSkeuo />;
  return <SourcesViewStandard />;
}
