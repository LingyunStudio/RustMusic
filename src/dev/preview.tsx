// 浏览器 UI 预览入口：先装 Tauri mock，再挂载真实应用。
// 与主入口 main.tsx 内容一致，仅多一步 mock（preview.html 专用）。
// 注意必须动态 import：静态 import 会被提升到模块体之前，mock 就装晚了。
// （不用顶层 await：tsconfig target ES2021）
import { installMockTauri } from "./mockTauri";

installMockTauri();

import("../main").catch(() => {});
