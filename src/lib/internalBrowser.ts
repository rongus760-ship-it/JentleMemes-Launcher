import { writable } from "svelte/store";

/** Текущий URL внутреннего мини-браузера лаунчера (`null` — закрыто). */
export const internalBrowserUrl = writable<string | null>(null);

export function openInternalBrowser(url: string): void {
  const u = String(url || "").trim();
  if (!/^https?:\/\//i.test(u)) return;
  internalBrowserUrl.set(u);
}

export function closeInternalBrowser(): void {
  internalBrowserUrl.set(null);
}
