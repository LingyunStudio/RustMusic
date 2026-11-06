//! 按界面风格分发：standard（拟物分支未改动，与标准共用）= 合并前原版 / liquid = 液态玻璃分支原版。
import { useStore } from "../store";
import { InputModal as InputModalStandard, ConfirmModal as ConfirmModalStandard } from "./Dialogs.standard";
import { InputModal as InputModalLiquid, ConfirmModal as ConfirmModalLiquid } from "./Dialogs.liquid";

export function InputModal(props: Parameters<typeof InputModalStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  const Cmp = uiTheme === "liquid" ? InputModalLiquid : InputModalStandard;
  return <Cmp {...(props as any)} />;
}

export function ConfirmModal(props: Parameters<typeof ConfirmModalStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  const Cmp = uiTheme === "liquid" ? ConfirmModalLiquid : ConfirmModalStandard;
  return <Cmp {...(props as any)} />;
}
