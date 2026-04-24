/** Десктоп-уведомления (Web Notifications в webview Tauri). */

let permissionAsked = false;

export async function ensureNotificationPermission(): Promise<NotificationPermission> {
  if (typeof Notification === "undefined") return "denied";
  if (Notification.permission === "granted" || Notification.permission === "denied") {
    return Notification.permission;
  }
  if (permissionAsked) return Notification.permission;
  permissionAsked = true;
  try {
    return await Notification.requestPermission();
  } catch {
    return Notification.permission;
  }
}

export function canUseDesktopNotifications(): boolean {
  return typeof Notification !== "undefined";
}

/**
 * Показать уведомление, если окно в фоне или вкладка чата не активна.
 */
export function showDesktopNotificationIfBackground(
  title: string,
  body: string,
  opts: { tag?: string; chatVisible?: boolean } = {},
): void {
  if (typeof Notification === "undefined") return;
  if (Notification.permission !== "granted") return;
  const hidden = typeof document !== "undefined" && document.visibilityState === "hidden";
  const tabHidden = opts.chatVisible === false;
  if (!hidden && !tabHidden) return;
  try {
    new Notification(title, {
      body: body.slice(0, 500),
      tag: opts.tag || "jm-chat",
      silent: false,
    });
  } catch {
    /* ignore */
  }
}
