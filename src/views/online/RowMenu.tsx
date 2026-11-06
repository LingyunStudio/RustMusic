//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import { RowMenu as RowMenuStandard } from "./RowMenu.standard";
import { RowMenu as RowMenuSkeuo } from "./RowMenu.skeuo";
import { RowMenu as RowMenuLiquid } from "./RowMenu.liquid";
import { useStore } from "../../store";

export function RowMenu(props: Parameters<typeof RowMenuStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <RowMenuLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <RowMenuSkeuo {...(props as any)} />;
  return <RowMenuStandard {...(props as any)} />;
}
