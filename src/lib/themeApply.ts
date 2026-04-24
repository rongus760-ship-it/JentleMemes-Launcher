import {
  clearInlineThemeColors,
  applyThemeColors,
  generatePaletteFromAccent,
  extractColorsFromImage,
  loadCustomThemes,
} from "../themes";
import { resolveBackgroundImageSrc } from "./localImageUrl";

/**
 * JentleMemes 2.0 — трёхосевая темизация.
 *
 * Axis 1 — Visual Preset (structural): `html[data-preset="..."]`
 *   Переключает радиусы, плотность, тени, декоративные эффекты.
 *   Допустимые значения: blend (default) | modrinth | discord | legacy | glass.
 *
 * Axis 2 — Color Theme (palette): `.theme-*` класс на <html>
 *   Выбирает палитру цветов. Совместимо с v1.x.
 *
 * Axis 3 — Custom / auto-bg: inline-стили на <html>
 *   Переопределяют отдельные переменные (--bg, --accent и т.д.).
 *
 * Дефолт: data-preset="blend" + .theme-jentle-dark.
 */

export type VisualPreset = "blend" | "modrinth" | "discord" | "legacy" | "glass";

export const ALL_VISUAL_PRESETS: readonly VisualPreset[] = [
  "blend",
  "modrinth",
  "discord",
  "legacy",
  "glass",
] as const;

export function normalizeVisualPreset(raw: unknown): VisualPreset {
  const s = typeof raw === "string" ? raw.trim().toLowerCase() : "";
  return (ALL_VISUAL_PRESETS as readonly string[]).includes(s) ? (s as VisualPreset) : "blend";
}

export function applyVisualPreset(preset: VisualPreset) {
  const root = document.documentElement;
  root.setAttribute("data-preset", preset);
}

// ----------------------------------------------------------------------------
// Axis 4 — Shell Layout (структура и расположение дока)
// ----------------------------------------------------------------------------
//  classic      — левый сайдбар + титлбар (дефолт)
//  dock-bottom  — плавающий centered-док снизу (macOS/iPad-like)
//  split-rail   — тонкий иконочный рельс слева + крупный top-bar
//  command-only — почти без дока, весь вход через Ctrl+K + мини-хаб сверху
//  holo-arc     — «мозговзрыв»: радиальный плавающий dial в правом нижнем углу
// ----------------------------------------------------------------------------

export type ShellLayout =
  | "classic"
  | "dock-bottom"
  | "split-rail"
  | "command-only"
  | "holo-arc";

export const ALL_SHELL_LAYOUTS: readonly ShellLayout[] = [
  "classic",
  "dock-bottom",
  "split-rail",
  "command-only",
  "holo-arc",
] as const;

export function normalizeShellLayout(raw: unknown): ShellLayout {
  const s = typeof raw === "string" ? raw.trim().toLowerCase() : "";
  return (ALL_SHELL_LAYOUTS as readonly string[]).includes(s)
    ? (s as ShellLayout)
    : "classic";
}

export function applyShellLayout(layout: ShellLayout) {
  const root = document.documentElement;
  root.setAttribute("data-shell-layout", layout);
}

// ----------------------------------------------------------------------------
// Axis 5 — Overlay Layout (режим оформления in-game оверлея)
// ----------------------------------------------------------------------------
//  panel     — прежняя карточная сетка виджетов (дефолт)
//  hud       — компактная верхняя HUD-полоса + мини-виджеты по периметру
//  radial    — круговой/радиальный дашборд в центре
//  ticker    — узкая «бегущая» лента (Twitch-style)
//  neon-grid — неоновая киберпанк-сетка, большие тайлы с глитчем
// ----------------------------------------------------------------------------

export type OverlayLayout = "panel" | "hud" | "radial" | "ticker" | "neon-grid";

export const ALL_OVERLAY_LAYOUTS: readonly OverlayLayout[] = [
  "panel",
  "hud",
  "radial",
  "ticker",
  "neon-grid",
] as const;

export function normalizeOverlayLayout(raw: unknown): OverlayLayout {
  const s = typeof raw === "string" ? raw.trim().toLowerCase() : "";
  return (ALL_OVERLAY_LAYOUTS as readonly string[]).includes(s)
    ? (s as OverlayLayout)
    : "panel";
}

export function applyOverlayLayout(layout: OverlayLayout) {
  const root = document.documentElement;
  root.setAttribute("data-overlay-layout", layout);
}

export async function applyTheme(theme: string, bg: string) {
  const root = document.documentElement;
  root.className = root.className.replace(/\btheme-\S+/g, "");
  clearInlineThemeColors();

  if (theme === "auto-bg" && bg) {
    const imgSrc = await resolveBackgroundImageSrc(bg);
    const { dominant, isLight } = await extractColorsFromImage(imgSrc);
    const palette = generatePaletteFromAccent(dominant, isLight);
    applyThemeColors(palette);
  } else if (theme.startsWith("custom-")) {
    const customs = await loadCustomThemes();
    const ct = customs.find((c) => c.id === theme);
    if (ct) {
      applyThemeColors(ct.colors);
    } else {
      root.classList.add("theme-jentle-dark");
    }
  } else {
    root.classList.add(`theme-${theme || "jentle-dark"}`);
  }

  // Убедимся, что data-preset всегда задан — иначе применится дефолт "blend"
  // через fallback-значения в :root.
  if (!root.getAttribute("data-preset")) {
    root.setAttribute("data-preset", "blend");
  }

  window.dispatchEvent(new CustomEvent("jm_theme", { detail: { theme, bg } }));
  snapshotThemeForOverlay();
}

/** Снимок CSS-темы для второго окна (оверлей) — тот же origin, событие `storage` подхватит оверлей. */
export function snapshotThemeForOverlay() {
  try {
    const el = document.documentElement;
    const cs = getComputedStyle(el);
    const keys = [
      "--bg",
      "--accent",
      "--accent-light",
      "--card",
      "--text",
      "--text-secondary",
      "--accent-rgb",
      "--input-bg",
      "--header-bg",
      "--border",
    ] as const;
    const data: Record<string, string> = {
      className: el.className,
      preset: el.getAttribute("data-preset") || "blend",
      shellLayout: el.getAttribute("data-shell-layout") || "classic",
      overlayLayout: el.getAttribute("data-overlay-layout") || "panel",
    };
    for (const k of keys) {
      data[k] = cs.getPropertyValue(k).trim();
    }
    localStorage.setItem("jm_overlay_theme_v1", JSON.stringify(data));
  } catch {
    /* ignore */
  }
}
