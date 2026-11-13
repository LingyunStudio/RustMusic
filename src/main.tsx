import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { initTheme } from "./theme";
import { applySkin, loadSkin } from "./skins";
import { initI18n } from "./i18n";

import { PLATFORM } from "./platform";

initI18n();
initTheme();
applySkin(loadSkin());
// 平台标记（CSS 按平台差异化），首屏即可用，无异步闪烁
document.documentElement.dataset.platform = PLATFORM;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
