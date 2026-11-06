//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import { PlaylistPickerModal as PlaylistPickerModalStandard } from "./PlaylistPickerModal.standard";
import { PlaylistPickerModal as PlaylistPickerModalSkeuo } from "./PlaylistPickerModal.skeuo";
import { PlaylistPickerModal as PlaylistPickerModalLiquid } from "./PlaylistPickerModal.liquid";
import { useStore } from "../../store";

export function PlaylistPickerModal(props: Parameters<typeof PlaylistPickerModalStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <PlaylistPickerModalLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <PlaylistPickerModalSkeuo {...(props as any)} />;
  return <PlaylistPickerModalStandard {...(props as any)} />;
}
