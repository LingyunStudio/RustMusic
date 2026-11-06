//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import ModalStandard from "./Modal.standard";
import ModalSkeuo from "./Modal.skeuo";
import ModalLiquid from "./Modal.liquid";
import { useStore } from "../store";

export default function Modal(props: Parameters<typeof ModalStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <ModalLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <ModalSkeuo {...(props as any)} />;
  return <ModalStandard {...(props as any)} />;
}
