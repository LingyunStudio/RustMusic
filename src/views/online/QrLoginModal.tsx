//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import { QrLoginModal as QrLoginModalStandard } from "./QrLoginModal.standard";
import { QrLoginModal as QrLoginModalSkeuo } from "./QrLoginModal.skeuo";
import { QrLoginModal as QrLoginModalLiquid } from "./QrLoginModal.liquid";
import { useStore } from "../../store";

export function QrLoginModal(props: Parameters<typeof QrLoginModalStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <QrLoginModalLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <QrLoginModalSkeuo {...(props as any)} />;
  return <QrLoginModalStandard {...(props as any)} />;
}
