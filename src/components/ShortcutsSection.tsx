import { useEffect, useState } from "react";
import { Keyboard, RotateCcw } from "lucide-react";
import { useI18n } from "../i18n";
import { useStore } from "../store";
import {
  SHORTCUT_LIST,
  comboFromEvent,
  findShortcutConflict,
  formatComboParts,
  getShortcuts,
  hasCustomShortcuts,
  resetAllShortcuts,
  resetShortcut,
  setShortcut,
  setShortcutRecording,
  subscribeShortcuts,
  type ShortcutAction,
  type ShortcutMap,
} from "../shortcuts";

interface ShortcutsSectionProps {
  sectionClass: string;
  isSkeuo: boolean;
  isLiquid: boolean;
  rowIdx: number;
}

export default function ShortcutsSection({
  sectionClass,
  isSkeuo,
  isLiquid,
  rowIdx,
}: ShortcutsSectionProps) {
  const { tr } = useI18n();
  const [map, setMap] = useState<ShortcutMap>(() => getShortcuts());
  const [recordingId, setRecordingId] = useState<ShortcutAction | null>(null);

  useEffect(() => subscribeShortcuts(() => setMap(getShortcuts())), []);

  useEffect(() => {
    setShortcutRecording(recordingId !== null);
    if (!recordingId) return;

    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        setRecordingId(null);
        return;
      }
      if ((e.key === "Backspace" || e.key === "Delete") && !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
        setShortcut(recordingId, "");
        setRecordingId(null);
        return;
      }

      const combo = comboFromEvent(e);
      if (!combo) return; // 仅按下修饰键时继续等待主键

      const conflict = findShortcutConflict(recordingId, combo);
      if (conflict) {
        useStore.getState().toast(
          tr(
            `快捷键已被「${conflict.zh}」占用`,
            `Shortcut already used by "${conflict.en}"`
          ),
          "error"
        );
        return;
      }

      setShortcut(recordingId, combo);
      setRecordingId(null);
    };

    const onPointerDown = () => {
      setRecordingId(null);
    };

    window.addEventListener("keydown", onKey, true);
    window.addEventListener("pointerdown", onPointerDown);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("pointerdown", onPointerDown);
      setShortcutRecording(false);
    };
  }, [recordingId, tr]);

  // 组件卸载兜底解除录制锁
  useEffect(() => () => setShortcutRecording(false), []);

  const anyCustom = hasCustomShortcuts(map);

  const rowBgClass = isSkeuo
    ? "bg-[var(--shade)] border border-[var(--line)]"
    : isLiquid
      ? "bg-[var(--shade)] border border-[var(--line)]"
      : "bg-white/[0.03] border border-[var(--line)]";

  return (
    <section style={{ ["--row-idx" as string]: rowIdx }} className={sectionClass}>
      <div className="flex items-center justify-between gap-3 mb-3">
        <div className="flex items-center gap-2">
          <Keyboard size={15} className="text-[var(--ink-2)]" />
          <h2 className="text-[14.5px] font-semibold">{tr("快捷键", "Keyboard Shortcuts")}</h2>
        </div>
        <button
          className="btn-secondary !py-1 !px-2.5 !text-[12px] flex items-center gap-1.5"
          disabled={!anyCustom}
          onClick={() => {
            setRecordingId(null);
            resetAllShortcuts();
            useStore.getState().toast(
              tr("已恢复全部默认快捷键", "Restored all default shortcuts"),
              "success"
            );
          }}
        >
          <RotateCcw size={12} />
          {tr("恢复全部默认", "Reset All")}
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-2.5">
        {SHORTCUT_LIST.map((item) => {
          const combo = map[item.id];
          const isRec = recordingId === item.id;
          const isModified = combo !== item.defaultCombo;
          const parts = formatComboParts(combo);

          return (
            <div
              key={item.id}
              className={`flex items-center justify-between gap-2 px-3 py-2 rounded-xl transition-colors ${rowBgClass}`}
            >
              <span className="text-[12.5px] text-[var(--ink)] truncate">
                {tr(item.zh, item.en)}
              </span>
              <div className="flex items-center gap-1.5 shrink-0">
                <button
                  type="button"
                  onPointerDown={(e) => e.stopPropagation()}
                  onClick={() => setRecordingId(isRec ? null : item.id)}
                  title={tr(
                    "点击录制新快捷键（Esc 取消，Backspace 清除）",
                    "Click to record shortcut (Esc to cancel, Backspace to clear)"
                  )}
                  className={`min-w-[82px] h-7 px-2.5 rounded-lg text-[11.5px] flex items-center justify-center gap-1 transition-all cursor-pointer ${
                    isRec
                      ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] border border-[var(--accent)] font-medium animate-pulse"
                      : isSkeuo
                        ? "skeuo-well text-[var(--ink)] hover:border-[var(--accent)]"
                        : "bg-[var(--shade-strong)] border border-[var(--line)] text-[var(--ink)] hover:border-[var(--accent)]"
                  }`}
                >
                  {isRec ? (
                    <span>{tr("请按键…", "Press keys…")}</span>
                  ) : parts.length > 0 ? (
                    parts.map((p, idx) => (
                      <span key={idx} className="inline-flex items-center gap-1">
                        {idx > 0 && <span className="text-[10px] text-[var(--ink-3)]">+</span>}
                        <kbd className="font-mono text-[11px] font-semibold tracking-tight">
                          {p}
                        </kbd>
                      </span>
                    ))
                  ) : (
                    <span className="text-[var(--ink-3)]">{tr("未设置", "Unbound")}</span>
                  )}
                </button>
                {isModified && (
                  <button
                    type="button"
                    className="btn-ghost !w-7 !h-7 !p-0 text-[var(--ink-3)] hover:text-[var(--ink)]"
                    onClick={() => {
                      if (isRec) setRecordingId(null);
                      resetShortcut(item.id);
                    }}
                    title={tr("恢复默认快捷键", "Restore default shortcut")}
                  >
                    <RotateCcw size={12} />
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>

      <p className="text-[11px] text-[var(--ink-3)] mt-2.5">
        {tr(
          "点击按键框可自定义快捷键（支持单键或组合键）；录制时按 Esc 取消，按 Backspace 清除。",
          "Click any binding to customize (single key or combo). Press Esc to cancel, or Backspace to unbind."
        )}
      </p>
    </section>
  );
}
