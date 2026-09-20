import { Check } from "lucide-react";
import Modal from "./Modal";
import { useStore } from "../store";
import { DEFAULT_SKIN, SKINS, skinUri } from "../skins";

/** 皮肤选择弹窗：背景场景缩略卡片网格，即时切换、选中高亮 */
export default function SkinPicker({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const skin = useStore((s) => s.skin);
  const setSkin = useStore((s) => s.setSkin);

  const card = (key: string, name: string, desc: string, bg: React.CSSProperties) => {
    const selected = skin === key;
    return (
      <button
        key={key}
        onClick={() => setSkin(key)}
        className={`relative aspect-video rounded-xl overflow-hidden text-left transition-all duration-200 hover:scale-[1.03] hover:shadow-lg ${
          selected ? "ring-2 ring-[var(--accent)]" : "ring-1 ring-[var(--line)]"
        }`}
        style={bg}
        title={desc}
      >
        {/* 底部文字保护渐变 */}
        <div className="absolute inset-x-0 bottom-0 px-2.5 pb-1.5 pt-6 bg-gradient-to-t from-black/60 to-transparent" />
        <div className="absolute left-2.5 bottom-1.5 min-w-0">
          <div className="text-[12.5px] text-white font-semibold leading-tight drop-shadow">
            {name}
          </div>
          <div className="text-[10px] text-white/75 truncate drop-shadow">{desc}</div>
        </div>
        {selected && (
          <span className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full bg-[var(--accent)] text-[var(--accent-on)] flex items-center justify-center shadow">
            <Check size={12} strokeWidth={3} />
          </span>
        )}
      </button>
    );
  };

  return (
    <Modal open={open} onClose={onClose} title="皮肤" width={720}>
      <div className="grid grid-cols-3 gap-3 max-h-[54vh] overflow-y-auto pr-1">
        {card(
          DEFAULT_SKIN,
          "默认",
          "主题氛围 · 无背景图",
          {
            background:
              "radial-gradient(ellipse 120% 100% at 50% -20%, #3a2a18 0%, #1c140c 55%, #0f0c09 100%)",
          }
        )}
        {SKINS.map((s) =>
          card(s.key, s.name, s.desc, {
            backgroundImage: skinUri(s.key) ?? undefined,
            backgroundSize: "cover",
            backgroundPosition: "center",
          })
        )}
      </div>
      <div className="text-[10.5px] text-[var(--ink-3)] mt-3 leading-relaxed">
        皮肤替换主界面背景，与浅色/暗色主题、强调色自由组合；播放时封面主色会为壁纸添上一层随音乐呼吸的微光。
      </div>
    </Modal>
  );
}
