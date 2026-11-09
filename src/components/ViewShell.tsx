import type { ReactNode } from "react";
import type { UiTheme } from "../theme";

interface ViewShellProps {
  uiTheme: UiTheme;
  header: ReactNode;
  children: ReactNode;
  headerClass?: string;
  liquidHeaderClass?: string;
  bottomPadding?: string;
  contentCardClass?: string;
  cardHeader?: ReactNode;
  extra?: ReactNode;
}

/**
 * 页面级视图外壳（解决液态玻璃与标准/拟物主题的外层面板差异）：
 * - 液态玻璃（liquid）：右侧内容整卡顶边与侧边栏对齐，header 位于 liquid-panel 内部，外框 pl-3 pr-2 pt-2 pb-[90px]
 * - 标准与拟物（standard / skeuo）：header 浮在外部，内容卡片单独包裹在 glass rounded-3xl 中，外框 px-6 pb-[86px]/pb-[96px]
 */
export default function ViewShell({
  uiTheme,
  header,
  children,
  headerClass = "px-8 pt-7 pb-4 shrink-0",
  liquidHeaderClass = "px-5 pt-4 pb-4 shrink-0",
  bottomPadding = "pb-[86px]",
  contentCardClass = "glass rounded-3xl",
  cardHeader,
  extra,
}: ViewShellProps) {
  if (uiTheme === "liquid") {
    return (
      <div className="flex-1 min-h-0 flex flex-col">
        {extra}
        <div className="flex-1 min-h-0 flex flex-col pl-3 pr-2 pt-2 pb-[90px]">
          {cardHeader}
          <div className="relative liquid-panel flex-1 min-h-0 flex flex-col overflow-hidden">
            <header className={liquidHeaderClass}>{header}</header>
            {children}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-0 flex flex-col">
      {extra}
      <header className={headerClass}>{header}</header>
      <div className={`flex-1 min-h-0 flex flex-col px-6 ${bottomPadding}`}>
        {cardHeader}
        <div
          className={`relative ${contentCardClass} flex-1 min-h-0 flex flex-col overflow-hidden`}
        >
          {children}
        </div>
      </div>
    </div>
  );
}
