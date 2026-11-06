//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import ToastStandard from "./Toast.standard";
import ToastSkeuo from "./Toast.skeuo";
import ToastLiquid from "./Toast.liquid";
import { useStore } from "../store";

export default function Toast() {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <ToastLiquid />;
  if (uiTheme === "skeuo") return <ToastSkeuo />;
  return <ToastStandard />;
}
