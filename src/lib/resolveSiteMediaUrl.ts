/** Абсолютный URL для картинок с API (аватары и т.д.), если в ответе пришёл путь без хоста. */
export function resolveSiteMediaUrl(
  apiBaseUrl: string,
  url: string | null | undefined,
): string | null {
  if (url == null) return null;
  const s = String(url).trim();
  if (!s) return null;
  if (/^https?:\/\//i.test(s)) return s;
  if (s.startsWith("blob:") || s.startsWith("data:")) return s;
  if (s.startsWith("//")) return `https:${s}`;
  const base = String(apiBaseUrl || "https://jentlememes.ru").replace(/\/$/, "");
  if (s.startsWith("/")) return `${base}${s}`;
  return `${base}/${s}`;
}
