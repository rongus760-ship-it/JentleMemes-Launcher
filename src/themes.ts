export interface ThemeDef {
  id: string;
  name: string;
  cssClass: string;
  preview: { bg: string; accent: string; card: string };
  isLight: boolean;
  defaultBg?: string;
}

export interface ThemeColors {
  bg: string;
  accent: string;
  accentLight: string;
  card: string;
  text: string;
  textSecondary: string;
  accentRgb: string;
  inputBg: string;
  headerBg: string;
  border: string;
}

export interface CustomThemeDef {
  id: string;
  name: string;
  isLight: boolean;
  colors: ThemeColors;
}

export const builtinThemes: ThemeDef[] = [
  {
    id: "jentle-dark",
    name: "Jentle Dark",
    cssClass: "theme-jentle-dark",
    preview: { bg: "#0b110b", accent: "#86A886", card: "#22302270" },
    isLight: false,
  },
  {
    id: "jentle-site",
    name: "Jentle Gold",
    cssClass: "theme-jentle-site",
    preview: { bg: "#111111", accent: "#d4a843", card: "#28231470" },
    isLight: false,
  },
  {
    id: "purple-dark",
    name: "Purple Dark",
    cssClass: "theme-purple-dark",
    preview: { bg: "#0e0b15", accent: "#9b6dd7", card: "#281c3c70" },
    isLight: false,
  },
  {
    id: "purple-light",
    name: "Purple Light",
    cssClass: "theme-purple-light",
    preview: { bg: "#f5f0fa", accent: "#7c3aed", card: "#f0ebfa" },
    isLight: true,
  },
  {
    id: "red-dark",
    name: "Red Dark",
    cssClass: "theme-red-dark",
    preview: { bg: "#110b0b", accent: "#d44343", card: "#3c1c1c70" },
    isLight: false,
  },
  {
    id: "red-light",
    name: "Red Light",
    cssClass: "theme-red-light",
    preview: { bg: "#faf0f0", accent: "#dc2626", card: "#faf0f0" },
    isLight: true,
  },
  {
    id: "light-gold",
    name: "Light Gold",
    cssClass: "theme-light-gold",
    preview: { bg: "#f8f5ee", accent: "#b8860b", card: "#f8f5eb" },
    isLight: true,
  },
  {
    id: "furry",
    name: "Furry Pink",
    cssClass: "theme-furry",
    preview: { bg: "#1a0b18", accent: "#ff7eb3", card: "#30142870" },
    isLight: false,
  },
];

export function getThemeDef(id: string): ThemeDef {
  return builtinThemes.find((t) => t.id === id) || builtinThemes[0];
}

/* ═══ Color math helpers ═══ */

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "");
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

function rgbToHex(r: number, g: number, b: number): string {
  return "#" + [r, g, b].map((c) => Math.round(Math.max(0, Math.min(255, c))).toString(16).padStart(2, "0")).join("");
}

function rgbToHsl(r: number, g: number, b: number): [number, number, number] {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h = 0;
  if (max === r) h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
  else if (max === g) h = ((b - r) / d + 2) / 6;
  else h = ((r - g) / d + 4) / 6;
  return [h * 360, s, l];
}

function hslToHex(h: number, s: number, l: number): string {
  h = ((h % 360) + 360) % 360;
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = l - c / 2;
  let r = 0, g = 0, b = 0;
  if (h < 60) { r = c; g = x; }
  else if (h < 120) { r = x; g = c; }
  else if (h < 180) { g = c; b = x; }
  else if (h < 240) { g = x; b = c; }
  else if (h < 300) { r = x; b = c; }
  else { r = c; b = x; }
  return rgbToHex((r + m) * 255, (g + m) * 255, (b + m) * 255);
}

/* ═══ Palette generation from a single accent color ═══ */

export function generatePaletteFromAccent(accentHex: string, isLight: boolean): ThemeColors {
  const [h, s] = rgbToHsl(...hexToRgb(accentHex));
  const [r, g, b] = hexToRgb(accentHex);
  const accentRgb = `${r},${g},${b}`;

  if (isLight) {
    const bg = hslToHex(h, Math.min(s * 0.15, 0.08), 0.96);
    const card = hslToHex(h, Math.min(s * 0.2, 0.1), 0.94);
    return {
      bg,
      accent: accentHex,
      accentLight: hslToHex(h, Math.min(s * 0.8, 0.6), 0.65),
      card: card + "e6",
      text: hslToHex(h, Math.min(s * 0.3, 0.15), 0.12),
      textSecondary: hslToHex(h, Math.min(s * 0.2, 0.1), 0.42),
      accentRgb,
      inputBg: hslToHex(h, Math.min(s * 0.12, 0.06), 0.92),
      headerBg: `rgba(${hexToRgb(bg).join(",")}, 0.8)`,
      border: `rgba(${r},${g},${b}, 0.15)`,
    };
  }

  const bg = hslToHex(h, Math.min(s * 0.3, 0.2), 0.06);
  const card = hslToHex(h, Math.min(s * 0.4, 0.25), 0.14);
  return {
    bg,
    accent: accentHex,
    accentLight: hslToHex(h, Math.min(s * 0.8, 0.6), 0.72),
    card: card + "b3",
    text: "#ffffff",
    textSecondary: hslToHex(h, Math.min(s * 0.2, 0.12), 0.62),
    accentRgb,
    inputBg: hslToHex(h, Math.min(s * 0.25, 0.15), 0.07),
    headerBg: `rgba(${hexToRgb(bg).map((c) => Math.max(0, c - 8)).join(",")}, 0.5)`,
    border: `rgba(${r},${g},${b}, 0.15)`,
  };
}

/* ═══ Apply theme colors to documentElement ═══ */

export function applyThemeColors(colors: ThemeColors) {
  const root = document.documentElement;
  root.style.setProperty("--bg", colors.bg);
  root.style.setProperty("--accent", colors.accent);
  root.style.setProperty("--accent-light", colors.accentLight);
  root.style.setProperty("--card", colors.card);
  root.style.setProperty("--text", colors.text);
  root.style.setProperty("--text-secondary", colors.textSecondary);
  root.style.setProperty("--accent-rgb", colors.accentRgb);
  root.style.setProperty("--input-bg", colors.inputBg);
  root.style.setProperty("--header-bg", colors.headerBg);
  root.style.setProperty("--border", colors.border);
}

export function clearInlineThemeColors() {
  const root = document.documentElement;
  const props = ["--bg", "--accent", "--accent-light", "--card", "--text", "--text-secondary", "--accent-rgb", "--input-bg", "--header-bg", "--border"];
  props.forEach((p) => root.style.removeProperty(p));
}

/* ═══ Extract dominant color from an image ═══ */

export function extractColorsFromImage(src: string): Promise<{ dominant: string; isLight: boolean }> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      try {
        const canvas = document.createElement("canvas");
        const size = 50;
        canvas.width = size;
        canvas.height = size;
        const ctx = canvas.getContext("2d")!;
        ctx.drawImage(img, 0, 0, size, size);
        const data = ctx.getImageData(0, 0, size, size).data;

        const buckets: { r: number; g: number; b: number; sat: number; count: number }[] = Array.from({ length: 12 }, () => ({ r: 0, g: 0, b: 0, sat: 0, count: 0 }));
        let totalLum = 0;
        let pixCount = 0;

        for (let i = 0; i < data.length; i += 4) {
          const r = data[i], g = data[i + 1], b = data[i + 2];
          const [h, s, l] = rgbToHsl(r, g, b);
          totalLum += l;
          pixCount++;
          if (l < 0.08 || l > 0.92 || s < 0.08) continue;
          const idx = Math.min(11, Math.floor(h / 30));
          buckets[idx].r += r;
          buckets[idx].g += g;
          buckets[idx].b += b;
          buckets[idx].sat += s;
          buckets[idx].count++;
        }

        let best = buckets[0];
        for (const b of buckets) {
          if (b.count > 0 && b.sat / b.count > (best.count > 0 ? best.sat / best.count : 0)) {
            best = b;
          }
        }

        const avgLum = pixCount > 0 ? totalLum / pixCount : 0.5;

        if (best.count === 0) {
          const grey = avgLum > 0.5 ? "#6b7280" : "#9ca3af";
          resolve({ dominant: grey, isLight: avgLum > 0.5 });
          return;
        }

        const dr = Math.round(best.r / best.count);
        const dg = Math.round(best.g / best.count);
        const db = Math.round(best.b / best.count);
        const [dh, ds] = rgbToHsl(dr, dg, db);
        const boosted = hslToHex(dh, Math.max(ds, 0.5), avgLum > 0.5 ? 0.45 : 0.6);

        resolve({ dominant: boosted, isLight: avgLum > 0.5 });
      } catch (e) {
        resolve({ dominant: "#86A886", isLight: false });
      }
    };
    img.onerror = () => resolve({ dominant: "#86A886", isLight: false });
    img.src = src;
  });
}

/* ═══ Custom themes persistence (via Tauri invoke) ═══ */

import { invoke } from "@tauri-apps/api/core";

let _customThemesCache: CustomThemeDef[] | null = null;

export async function loadCustomThemes(): Promise<CustomThemeDef[]> {
  if (_customThemesCache) return _customThemesCache;
  try {
    const s: any = await invoke("load_settings");
    _customThemesCache = (s.custom_themes as CustomThemeDef[]) || [];
  } catch {
    _customThemesCache = [];
  }
  return _customThemesCache;
}

export async function saveCustomThemes(themes: CustomThemeDef[]) {
  _customThemesCache = themes;
  try {
    const s: any = await invoke("load_settings");
    s.custom_themes = themes;
    await invoke("save_settings", { settings: s });
  } catch (e) {
    console.error("Failed to save custom themes:", e);
  }
}

export function invalidateCustomThemesCache() {
  _customThemesCache = null;
}
