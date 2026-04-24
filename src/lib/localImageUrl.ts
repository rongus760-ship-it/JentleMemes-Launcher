import { convertFileSrc, invoke } from "@tauri-apps/api/core";

const cache = new Map<string, string>();

/** Превью и полноэкранный фон лаунчера: на Linux WebKit data URL надёжнее, чем asset:// из convertFileSrc. */
export async function resolveBackgroundImageSrc(absPath: string): Promise<string> {
  const p = absPath?.trim();
  if (!p) return "";
  const hit = cache.get(p);
  if (hit) return hit;
  try {
    const dataUrl = await invoke<string>("read_local_image_data_url", { path: p });
    cache.set(p, dataUrl);
    return dataUrl;
  } catch {
    const fallback = convertFileSrc(p);
    cache.set(p, fallback);
    return fallback;
  }
}

export function invalidateBackgroundImageCache(path?: string) {
  if (path) cache.delete(path);
  else cache.clear();
}
