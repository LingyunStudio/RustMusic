import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { initTheme } from "./theme";
import { applySkin, loadSkin } from "./skins";

initTheme();
applySkin(loadSkin());
// 平台标记（CSS 按平台差异化）。UA 即时可得，无异步闪烁；
// Windows WebView2 恒含 "Windows NT"
document.documentElement.dataset.platform = /windows/i.test(navigator.userAgent)
  ? "windows"
  : "linux";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
