//! 按界面风格分发：standard = 合并前原版 / skeuo = 拟物分支原版 / liquid = 液态玻璃分支原版。
import { BiliSpaceCard as BiliSpaceCardStandard } from "./BiliSpaceCard.standard";
import { BiliSpaceCard as BiliSpaceCardSkeuo } from "./BiliSpaceCard.skeuo";
import { BiliSpaceCard as BiliSpaceCardLiquid } from "./BiliSpaceCard.liquid";
import { useStore } from "../../store";

export function BiliSpaceCard(props: Parameters<typeof BiliSpaceCardStandard>[0]) {
  const uiTheme = useStore((s) => s.uiTheme);
  if (uiTheme === "liquid") return <BiliSpaceCardLiquid {...(props as any)} />;
  if (uiTheme === "skeuo") return <BiliSpaceCardSkeuo {...(props as any)} />;
  return <BiliSpaceCardStandard {...(props as any)} />;
}
