import { useState } from "react";
import { Check, ImagePlus, RefreshCw, X } from "lucide-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import Modal from "./Modal";
import { useStore } from "../store";
import { api } from "../api";
import {
  activeCustomImage,
  applySkin,
  CUSTOM_SKIN,
  DEFAULT_SKIN,
  loadCustomImages,
  loadCustomSkinSettings,
  saveCustomImages,
  saveCustomSkinSettings,
  setActiveCustomImage,
  SKINS,
  skinSupportsFx,
  skinUri,
  type CustomFill,
  type CustomSkinSettings,
} from "../skins";

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
  const fxAudio = useStore((s) => s.fxAudio);
  const setFxAudio = useStore((s) => s.setFxAudio);
  const [custom, setCustom] = useState<CustomSkinSettings>(loadCustomSkinSettings);
  const [customImgs, setCustomImgs] = useState<string[]>(loadCustomImages);
  const [activeImg, setActiveImg] = useState<string>(activeCustomImage);

  const update = (next: CustomSkinSettings) => {
    setCustom(next);
    saveCustomSkinSettings(next);
    // 自定义皮肤启用中：设置变化即时生效
    if (skin === CUSTOM_SKIN) applySkin(CUSTOM_SKIN);
  };

  const pickImage = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const sel = await open({
        multiple: true,
        title: "添加皮肤图片",
        filters: [
          { name: "图片", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "gif"] },
        ],
      });
      const paths = typeof sel === "string" ? [sel] : (sel ?? []);
      if (!paths.length) return;
      // 大图先换屏幕级压缩副本（失败/动图/小图原样返回），
      // 避免原始分辨率壁纸整图解码后常驻渲染进程图片缓存
      const prepared = await Promise.all(
        paths.map((p) =>
          api
            .prepareSkinImage(p)
            .catch(() => p)
        )
      );
      const list = [...customImgs, ...prepared.filter((p) => !customImgs.includes(p))];
      setCustomImgs(list);
      saveCustomImages(list);
      // asset 协议作用域默认不含用户任意目录：选图后逐个放行（原图需放行，压缩副本在应用数据目录内已默认可见）
      for (const p of paths) {
        await api.assetScopeAllow(p).catch(() => {});
      }
      setActiveCustomImage(prepared[prepared.length - 1]);
      setActiveImg(prepared[prepared.length - 1]);
      setSkin(CUSTOM_SKIN);
      useStore.getState().toast(`已添加 ${paths.length} 张皮肤图片`, "success");
    } catch (e) {
      useStore.getState().toast(String(e), "error");
    }
  };

  const removeImage = (path: string) => {
    const list = customImgs.filter((p) => p !== path);
    setCustomImgs(list);
    saveCustomImages(list);
    if (activeImg === path) {
      // 删除的是当前启用的图片：切回默认皮肤
      setActiveCustomImage("");
      setActiveImg("");
      setSkin(DEFAULT_SKIN);
    }
  };

  const card = (key: string, name: string, desc: string, bg: React.CSSProperties) => {
    const selected = skin === key;
    return (
      <button
        key={key}
        onClick={() => setSkin(key)}
        // pt-[56.25%] 撑出 16:9（aspect-ratio 在 WebKitGTK 的 button 上不可靠，
        // 高度会塌陷成 0 导致卡片内容相互叠压）；百分比 padding 全引擎行为一致
        className="relative w-full pt-[56.25%] rounded-xl overflow-hidden text-left transition-all duration-200 hover:scale-[1.03] hover:shadow-lg"
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
      <div className="skin-picker-fixed">
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
        {/* 自定义图片图库（多张，hover 出删除） */}
        {customImgs.map((img) => {
          const active = skin === CUSTOM_SKIN && activeImg === img;
          return (
            <div
              key={img}
              onClick={() => {
                setActiveCustomImage(img);
                setActiveImg(img);
                setSkin(CUSTOM_SKIN);
              }}
              className={`group relative w-full pt-[56.25%] rounded-xl overflow-hidden cursor-pointer transition-all duration-200 hover:scale-[1.03] hover:shadow-lg ${
                active ? "" : "opacity-80"
              }`}
              style={{
                backgroundImage: `url("${convertFileSrc(img)}")`,
                backgroundSize: "var(--skin-fill, cover)",
                backgroundPosition: "center",
              }}
              title="使用这张图片"
            >
              {active && (
                <span className="absolute top-1.5 right-1.5 w-5 h-5 rounded-full bg-[var(--accent)] text-[var(--accent-on)] flex items-center justify-center shadow">
                  <Check size={12} strokeWidth={3} />
                </span>
              )}
              <span
                className="absolute top-1.5 left-1.5 w-5 h-5 rounded-full bg-black/55 text-white flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity hover:bg-red-500/80"
                onClick={(e) => {
                  e.stopPropagation();
                  removeImage(img);
                }}
                title="删除"
              >
                <X size={11} strokeWidth={3} />
              </span>
              <div className="absolute inset-x-0 bottom-0 px-2.5 pb-1.5 pt-6 bg-gradient-to-t from-black/60 to-transparent">
                <div className="text-[12.5px] text-white font-semibold leading-tight drop-shadow">
                  自定义图片
                </div>
              </div>
            </div>
          );
        })}
        {/* 添加图片 */}
        <button
          onClick={pickImage}
          className="relative w-full pt-[56.25%] rounded-xl overflow-hidden transition-all duration-200 hover:scale-[1.03]"
          style={{ background: "var(--shade)" }}
          title="添加图片"
        >
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-1.5 text-[var(--ink-3)]">
            <ImagePlus size={20} />
            <span className="text-[11.5px]">添加图片</span>
          </div>
        </button>
      </div>
      {/* 自定义皮肤设置区 */}
      {skin === CUSTOM_SKIN && activeImg && (
        <div className="mt-3 p-4 rounded-xl bg-[var(--shade)] flex flex-col gap-3.5">
          <div className="flex items-center gap-3">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[64px] shrink-0">填充方式</span>
            <div className="flex gap-1">
              {(
                [
                  ["cover", "封面"],
                  ["contain", "包含"],
                  ["stretch", "拉伸"],
                  ["tile", "平铺"],
                ] as const
              ).map(([k, label]) => (
                <button
                  key={k}
                  onClick={() => update({ ...custom, fill: k as CustomFill })}
                  className={`px-3 py-1 rounded-full text-[12px] transition-colors ${
                    custom.fill === k
                      ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                      : "text-[var(--ink-2)] hover:bg-[var(--shade-hover)]"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
            <button
              className="ml-auto btn-ghost w-8 h-8 text-[var(--ink-2)]"
              onClick={pickImage}
              title="更换图片"
            >
              <RefreshCw size={14} />
            </button>
          </div>
          <div className="flex items-center gap-3">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[64px] shrink-0">透明度</span>
            <input
              type="range"
              min={0}
              max={100}
              value={custom.scrim}
              onChange={(e) => update({ ...custom, scrim: Number(e.target.value) })}
              className="flex-1 accent-[var(--accent)]"
            />
            <span className="text-[12px] text-[var(--ink-2)] tabular-nums w-10 text-right">
              {custom.scrim}%
            </span>
          </div>
          <div className="flex items-center gap-3">
            <span className="text-[12.5px] text-[var(--ink-2)] w-[64px] shrink-0">文字色系</span>
            <div className="flex gap-1">
              {(
                [
                  ["auto", "自动"],
                  ["light", "浅色文字"],
                  ["dark", "深色文字"],
                ] as const
              ).map(([k, label]) => (
                <button
                  key={k}
                  onClick={() => update({ ...custom, text: k })}
                  className={`px-3 py-1 rounded-full text-[12px] transition-colors ${
                    custom.text === k
                      ? "bg-[var(--accent-weak)] text-[var(--accent-strong)] font-medium"
                      : "text-[var(--ink-2)] hover:bg-[var(--shade-hover)]"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
      {/* 音频律动开关：仅支持律动的皮肤（带基座版）显示 */}
      {skinSupportsFx(skin) && (
        <div className="mt-3 p-4 rounded-xl bg-[var(--shade)] flex items-center gap-3">
          <span className="text-[12.5px] text-[var(--ink-2)] shrink-0">音频律动</span>
          <span className="text-[11.5px] text-[var(--ink-3)] flex-1 leading-snug">
            频谱柱、涟漪与光晕跟随正在播放的歌曲实时起伏（引擎 FFT，两种输出模式均生效）
          </span>
          <button
            className={`relative w-10 h-[22px] rounded-full transition-colors shrink-0 ${
              fxAudio ? "bg-[var(--accent)]" : "bg-[var(--shade-strong)]"
            }`}
            onClick={() => setFxAudio(!fxAudio)}
            title={fxAudio ? "关闭音频律动" : "开启音频律动"}
          >
            <span
              className={`absolute top-[3px] w-4 h-4 rounded-full bg-white shadow transition-all ${
                fxAudio ? "left-[21px]" : "left-[3px]"
              }`}
            />
          </button>
        </div>
      )}
      <div className="text-[10.5px] text-[var(--ink-3)] mt-3 leading-relaxed">
        皮肤替换主界面背景，与浅色/暗色主题、强调色自由组合；播放时封面主色会为壁纸添上一层随音乐呼吸的微光。
      </div>
      </div>
    </Modal>
  );
}
