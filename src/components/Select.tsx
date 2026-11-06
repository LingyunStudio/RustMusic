//! 按界面风格分发：standard（拟物分支未改动，与标准共用）= 合并前原版 / liquid = 液态玻璃分支原版。
import { useStore } from "../store";
export type { SelectOption } from "./Select.standard";
import { Select as SelectStandard } from "./Select.standard";
import { Select as SelectLiquid } from "./Select.liquid";

export function Select(props: Parameters<typeof SelectStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  const Cmp = uiTheme === "liquid" ? SelectLiquid : SelectStandard;
  return <Cmp {...(props as any)} />;
}
