/** Расположение панели навигации (сохраняется в `chrome_layout`). */
export type ChromeLayout =
  | "sidebar_left_expanded"
  | "sidebar_left_compact"
  | "sidebar_right_expanded"
  | "sidebar_right_compact"
  | "top_tabs"
  | "bottom_tabs";

export type ModalPreset = "minimal" | "glass" | "dense" | "sheet";

export type DownloadCorner = "bl" | "br" | "tl" | "tr" | "hidden";

const ALL_LAYOUTS: ChromeLayout[] = [
  "sidebar_left_expanded",
  "sidebar_left_compact",
  "sidebar_right_expanded",
  "sidebar_right_compact",
  "top_tabs",
  "bottom_tabs",
];

export function migrateChromeLayout(raw: unknown, sidebarStyle: unknown): ChromeLayout {
  const s = typeof raw === "string" ? raw.trim() : "";
  if (ALL_LAYOUTS.includes(s as ChromeLayout)) return s as ChromeLayout;
  return sidebarStyle === "compact" ? "sidebar_left_compact" : "sidebar_left_expanded";
}

export function sidebarStyleFromLayout(layout: ChromeLayout): "expanded" | "compact" {
  return layout.includes("compact") ? "compact" : "expanded";
}

/** Синхронизация атрибутов на `<html>` для CSS и позиционирования порталов. */
export function applyChromeDocumentAttrs(layout: ChromeLayout, modalPreset: ModalPreset) {
  const root = document.documentElement;
  root.setAttribute("data-chrome-layout", layout);
  root.setAttribute("data-modal-preset", modalPreset);
  const navTop = layout === "top_tabs";
  const navBottom = layout === "bottom_tabs";
  let stack = "2.5rem";
  if (navTop) stack = "calc(2.5rem + 2.75rem)";
  root.style.setProperty("--jm-chrome-stack", stack);
  root.setAttribute("data-chrome-nav-bottom", navBottom ? "1" : "0");
}

/** Переключить expanded ↔ compact для текущего «семейства» боковой панели. */
export function toggleSidebarDensity(layout: ChromeLayout): ChromeLayout {
  const map: Partial<Record<ChromeLayout, ChromeLayout>> = {
    sidebar_left_expanded: "sidebar_left_compact",
    sidebar_left_compact: "sidebar_left_expanded",
    sidebar_right_expanded: "sidebar_right_compact",
    sidebar_right_compact: "sidebar_right_expanded",
  };
  return map[layout] ?? layout;
}
