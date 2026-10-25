// RustMusic 菜单栏歌词：GNOME 顶栏显示当前歌词行。
// 数据来源：RustMusic 应用以原子写（rename）更新 $XDG_RUNTIME_DIR/rustmusic-menubar.json，
// 本扩展每 400ms 轮询；内容 {"line": "...", "ts": <unix_ms>}，
// ts 距今超过 15s（应用退出/崩溃）即隐藏，避免顶栏残留过期歌词。
import GLib from "gi://GLib";
import GObject from "gi://GObject";
import St from "gi://St";
import Clutter from "gi://Clutter";
import Pango from "gi://Pango";
// GNOME 45+ 的模块导出形态：panelMenu/main 均为具名导出（Button / panel 等），
// 没有 PanelMenu/Main 包装对象（49+ 移除），用命名空间导入取实时绑定
import * as PanelMenu from "resource:///org/gnome/shell/ui/panelMenu.js";
import * as Main from "resource:///org/gnome/shell/ui/main.js";
import { Extension } from "resource:///org/gnome/shell/extensions/extension.js";

const FILE_PATH = GLib.get_user_runtime_dir() + "/rustmusic-menubar.json";
const POLL_MS = 400;
const STALE_MS = 15000;
// 按钮固定宽度：顶栏元素位置恒定，歌词长短不推移任何元素。
// label 自适应文本并在按钮内右对齐（贴住右侧图标）——短句的间隙只
// 出现在左侧空白处；超长行 max-width 截断 + 省略号
const BUTTON_STYLE = "width: 280px;";
const LABEL_STYLE = "font-weight: 600; margin: 0 6px; max-width: 268px;";

const RustMusicLyricsIndicator = GObject.registerClass(
  class RustMusicLyricsIndicator extends PanelMenu.Button {
    _init() {
      // dontCreateMenu=true：纯文本指示器，点击不弹菜单
      super._init(0.0, "RustMusic 歌词", true);
      this._timer = null;
      this.set_style(BUTTON_STYLE);
      this._label = new St.Label({
        text: "",
        y_align: Clutter.ActorAlign.CENTER,
        x_align: Clutter.ActorAlign.END,
      });
      this._label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
      this._label.set_style(LABEL_STYLE);
      this.add_child(this._label);
      this.visible = false;
      this._timer = null;
      // 面板销毁路径可能不走我们的 destroy()：挂 destroy 信号兜底停轮询
      this.connect("destroy", () => this._stopTimer());
      this._poll();
      this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, POLL_MS, () => {
        this._poll();
        return GLib.SOURCE_CONTINUE;
      });
    }

    _stopTimer() {
      if (this._timer) {
        GLib.source_remove(this._timer);
        this._timer = null;
      }
    }

    _poll() {
      let text = "";
      let color = "";
      let fresh = false;
      try {
        const [, bytes] = GLib.file_get_contents(FILE_PATH);
        const obj = JSON.parse(new TextDecoder().decode(bytes));
        fresh = Date.now() - (obj.ts ?? 0) < STALE_MS;
        if (fresh) {
          text = String(obj.line ?? "");
          // 只接受颜色字面量，防 CSS 注入
          const c = typeof obj.color === "string" ? obj.color.trim() : "";
          if (/^#[0-9a-fA-F]{3,8}$/.test(c) || /^rgba?\(\s*[\d.,\s]+\)$/.test(c)) {
            color = c;
          }
        }
      } catch {
        // 文件不存在（应用未运行/未开启）：隐藏
      }
      this._label.text = text;
      this._label.set_style(LABEL_STYLE + (color ? ` color: ${color};` : ""));
      // 应用在写入期间（含间奏的空行）保留占位，顶栏元素位置完全稳定；
      // 仅在停止写入（过期/无文件）时隐藏
      this.visible = fresh;
      this._reposition();
    }

    /**
     * 位置自校正：歌词必须始终位于所有软件图标的最左侧。托盘 SNI 图标是
     * 应用启动时动态插入的且总插在箱首，会把歌词往右推——每次轮询校正：
     * 目标下标 = 第一个非面板自身元素（时钟/activities）的子项之前。
     * 全部包在 try 里：indicator 被 C 侧 dispose 后轮询仍可能触发一次
     */
    _reposition() {
      try {
        const container = this.container;
        const box = container.get_parent();
        if (!box) return;
        const chrome = new Set();
        for (const role of ["dateMenu", "activities"]) {
          const c = Main.panel.statusArea[role]?.container;
          if (c) chrome.add(c);
        }
        const children = box.get_children();
        const mine = children.indexOf(container);
        let target = -1;
        for (let i = 0; i < children.length; i++) {
          if (i === mine || chrome.has(children[i])) continue;
          target = i;
          break;
        }
        if (target < 0 || mine < 0) return;
        // 若自己位于目标之前，先移除自己会使目标前移一位
        const insertAt = mine < target ? target - 1 : target;
        if (insertAt !== mine) {
          box.remove_child(container);
          box.insert_child_at_index(container, insertAt);
        }
      } catch {
        // indicator 已销毁：静默
      }
    }

    destroy() {
      this._stopTimer();
      super.destroy();
    }
  }
);

export default class RustMusicLyricsExtension extends Extension {
  enable() {
    this._indicator = new RustMusicLyricsIndicator();
    // 目标：所有软件/托盘图标（应用菜单、托盘 SNI、系统指示器）整体的最左侧。
    // 图标可能分布在左箱（应用菜单/托盘图标）与右箱（系统指示器）——
    // 依次扫两箱，取第一个图标容器的下标；左箱命中即插左箱（整体最左），
    // 左箱没有才落右箱。activities 是面板自身按钮、dateMenu 是时钟，
    // 都不算软件图标，跳过（歌词保持在其右侧）
    const skip = new Set(["activities", "dateMenu", "rustmusic-lyrics"]);
    let target = null; // { boxName, idx }
    for (const boxName of ["left", "right"]) {
      const box =
        boxName === "left" ? Main.panel._leftBox : Main.panel._rightBox;
      const children = box.get_children();
      let minIdx = -1;
      for (const [role, ind] of Object.entries(Main.panel.statusArea)) {
        if (skip.has(role) || !ind?.container) continue;
        const i = children.indexOf(ind.container);
        if (i >= 0 && (minIdx < 0 || i < minIdx)) minIdx = i;
      }
      if (minIdx >= 0) {
        target = { boxName, idx: minIdx };
        break;
      }
    }
    const boxName = target?.boxName ?? "right";
    const idx = target?.idx ?? 0;
    print(`[rustmusic-lyrics] 菜单栏歌词插入位置: ${boxName}[${idx}]`);
    Main.panel.addToStatusArea("rustmusic-lyrics", this._indicator, idx, boxName);
  }

  disable() {
    this._indicator?.destroy();
    this._indicator = null;
  }
}
