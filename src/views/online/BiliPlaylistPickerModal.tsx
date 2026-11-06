//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import { BiliPlaylistPickerModal as BiliPlaylistPickerModalStandard } from "./BiliPlaylistPickerModal.standard";
import { BiliPlaylistPickerModal as BiliPlaylistPickerModalSkeuo } from "./BiliPlaylistPickerModal.skeuo";
import { BiliPlaylistPickerModal as BiliPlaylistPickerModalLiquid } from "./BiliPlaylistPickerModal.liquid";
import { useStore } from "../../store";

export function BiliPlaylistPickerModal(props: Parameters<typeof BiliPlaylistPickerModalStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <BiliPlaylistPickerModalLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <BiliPlaylistPickerModalSkeuo {...(props as any)} />;
  return <BiliPlaylistPickerModalStandard {...(props as any)} />;
}
