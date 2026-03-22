import { convertFileSrc } from "@tauri-apps/api/core";

/** Иконка сборки: http(s) как есть, иначе URL для локального файла (Tauri asset). */
export function instanceIconSrc(icon: string | undefined | null): string | null {
  if (icon == null) return null;
  const s = String(icon).trim();
  if (!s) return null;
  if (/^https?:\/\//i.test(s)) return s;
  try {
    return convertFileSrc(s);
  } catch {
    return null;
  }
}
