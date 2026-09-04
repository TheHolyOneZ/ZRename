import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { initTheme } from "./store/useSettingsStore";
import "./index.css";

initTheme();


if (import.meta.env.PROD) {
  document.addEventListener("contextmenu", (e) => {
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, [contenteditable='true']")) return;
    e.preventDefault();
  });
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
