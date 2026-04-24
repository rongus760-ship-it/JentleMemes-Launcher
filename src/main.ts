import "./index.css";
import App from "./App.svelte";
import IngameOverlay from "./IngameOverlay.svelte";

/** Без контекстного меню (в т.ч. «Исследовать элемент» в webview при devtools) */
document.addEventListener("contextmenu", (e) => e.preventDefault());

const target = document.getElementById("app");
if (!target) {
  throw new Error("Element #app not found");
}

const overlayMode = new URLSearchParams(window.location.search).get("overlay") === "1";
if (overlayMode) {
  // Критично для Tauri `transparent: true`: webkit2gtk / WKWebView / WebView2
  // рендерят окно прозрачным только если у html, body И всех их контейнеров
  // background НЕ задан. Инжектим <style> синхронно до первого paint'а —
  // иначе на первый кадр Tailwind `bg-jm-bg` применяется и окно мигает
  // сплошным прямоугольником.
  document.documentElement.classList.add("jm-overlay-mode");
  const earlyReset = document.createElement("style");
  earlyReset.id = "jm-overlay-early-reset";
  earlyReset.textContent = `
    html.jm-overlay-mode,
    html.jm-overlay-mode body,
    html.jm-overlay-mode #app,
    html.jm-overlay-mode .jm-ingame-overlay-root {
      background: transparent !important;
      background-color: transparent !important;
    }
    html.jm-overlay-mode body {
      color: #fff;
    }
  `;
  document.head.appendChild(earlyReset);
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";
  document.body.style.backgroundColor = "transparent";
}
// eslint-disable-next-line @typescript-eslint/no-unused-expressions
overlayMode ? new IngameOverlay({ target }) : new App({ target });
