import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import { showToast } from "./jmEvents";

const LABEL = "jm-overlay";

// Маркер версии конфигурации оверлейного окна. При изменении чего-либо в
// параметрах создания (transparent/decorations/class) инкрементируем —
// следующий toggleIngameOverlay() снесёт старое окно и создаст свежее.
// Иначе после обновления лаунчера live-окно остаётся с прежним `transparent`
// state и пользователь видит сплошной прямоугольник.
const WINDOW_VERSION = "2.0.0-transparent-1";
const WINDOW_VERSION_KEY = "jm_overlay_window_version";

type OverlayRect = {
  x: number;
  y: number;
  width: number;
  height: number;
  source: string;
};

function overlayWebviewUrl(): string {
  const u = new URL(window.location.href);
  u.hash = "";
  u.searchParams.set("overlay", "1");
  return u.href;
}

function clampRect(r: OverlayRect): { x: number; y: number; width: number; height: number } {
  const width = Math.max(320, Math.floor(Number(r.width) || 1280));
  const height = Math.max(240, Math.floor(Number(r.height) || 720));
  const x = Math.floor(Number(r.x) || 0);
  const y = Math.floor(Number(r.y) || 0);
  return { x, y, width, height };
}

async function destroyOverlayIfPresent(): Promise<void> {
  const w = await WebviewWindow.getByLabel(LABEL);
  if (!w) return;
  try {
    await w.close();
  } catch {
    /* ignore */
  }
}

async function waitCreatedOrError(win: WebviewWindow): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    let settled = false;
    const done = (fn: () => void) => {
      if (settled) return;
      settled = true;
      fn();
    };
    void win.once("tauri://created", () => done(resolve));
    void win.once("tauri://error", (ev) => {
      const p = (ev as { payload?: string })?.payload;
      done(() => reject(new Error(p ? String(p) : "ошибка создания окна")));
    });
  });
}

async function getOrCreateOverlayWindow(overlayUrl: string, rect: OverlayRect): Promise<WebviewWindow> {
  let w = await WebviewWindow.getByLabel(LABEL);

  // Если версия конфигурации окна не совпадает — уничтожаем живое окно,
  // чтобы пересоздать его с актуальными параметрами (transparent, размер, url).
  let savedVersion: string | null = null;
  try {
    savedVersion = localStorage.getItem(WINDOW_VERSION_KEY);
  } catch {
    savedVersion = null;
  }
  if (w && savedVersion !== WINDOW_VERSION) {
    await destroyOverlayIfPresent();
    w = null;
  }

  if (w) {
    try {
      await w.isVisible();
    } catch {
      await destroyOverlayIfPresent();
      w = null;
    }
  }
  if (w) return w;

  const { x, y, width, height } = clampRect(rect);
  const win = new WebviewWindow(LABEL, {
    url: overlayUrl,
    width,
    height,
    x,
    y,
    decorations: false,
    transparent: true,
    alwaysOnTop: true,
    skipTaskbar: true,
    resizable: false,
    focus: true,
  });

  try {
    await waitCreatedOrError(win);
    try {
      localStorage.setItem(WINDOW_VERSION_KEY, WINDOW_VERSION);
    } catch {
      /* ignore quota errors */
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    showToast(`Оверлей: ${msg}`);
    throw e;
  }

  return win;
}

/** Показать/скрыть оверлей: размер и позиция как у окна Minecraft / монитора. */
export async function toggleIngameOverlay(): Promise<void> {
  const overlayUrl = overlayWebviewUrl();

  let rect: OverlayRect;
  try {
    rect = await invoke<OverlayRect>("get_overlay_target_rect");
  } catch {
    rect = { x: 0, y: 0, width: 1280, height: 720, source: "fallback" };
  }

  let w: WebviewWindow | null = null;
  try {
    w = await getOrCreateOverlayWindow(overlayUrl, rect);
  } catch {
    return;
  }

  try {
    const vis = await w.isVisible().catch(() => false);
    if (vis) await w.hide();
    else {
      try {
        const r = await invoke<OverlayRect>("get_overlay_target_rect");
        const c = clampRect(r);
        await w.setSize(new PhysicalSize(c.width, c.height));
        await w.setPosition(new PhysicalPosition(c.x, c.y));
      } catch {
        /* ignore */
      }
      await w.show();
      await w.setFocus();
    }
  } catch (e) {
    showToast(`Оверлей: ${e instanceof Error ? e.message : String(e)}`);
  }
}
