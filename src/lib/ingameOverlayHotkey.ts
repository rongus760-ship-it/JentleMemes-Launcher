import { showToast } from "./jmEvents";

let lastRegisteredOverlayShortcut: string | null = null;

export async function registerIngameOverlayHotkey(enabled: boolean, hotkey: string): Promise<void> {
  const m = await import("@tauri-apps/plugin-global-shortcut");
  try {
    if (lastRegisteredOverlayShortcut) {
      try {
        await m.unregister(lastRegisteredOverlayShortcut);
      } catch {
        /* not registered */
      }
      lastRegisteredOverlayShortcut = null;
    }
    if (!enabled) return;
    const hk = (hotkey || "Alt+Backquote").trim() || "Alt+Backquote";
    await m.register(hk, (event) => {
      if (event.state !== "Pressed") return;
      void import("./ingameOverlayToggle").then((x) => x.toggleIngameOverlay());
    });
    lastRegisteredOverlayShortcut = hk;
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    showToast(`Горячая клавиша оверлея: ${msg}`);
    console.warn("ingame overlay hotkey:", e);
  }
}
