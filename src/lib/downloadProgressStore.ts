import { writable, derived, type Readable } from "svelte/store";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Единый стор прогресса загрузки. Раньше `App.svelte` и каждая вкладка
 * держали свою копию `progress` и дублировали `listen("download_progress", ...)`.
 * Теперь стор инициализируется один раз (`initDownloadProgress`) и вкладки
 * подписываются через subscribe.
 *
 * Относится к Phase 3.2 (декомпозиция god-компонентов App.svelte/LibraryTab).
 */
export interface DownloadProgressPayload {
  task_name: string;
  downloaded: number;
  total: number;
  instance_id?: string;
  silent?: boolean;
}

const EMPTY: DownloadProgressPayload = { task_name: "", downloaded: 0, total: 0 };

const internal = writable<DownloadProgressPayload>(EMPTY);
const busyInstanceIdInternal = writable<string | null>(null);

let unlisten: UnlistenFn | null = null;
let initPromise: Promise<void> | null = null;

export async function initDownloadProgress(): Promise<void> {
  if (initPromise) return initPromise;
  initPromise = (async () => {
    try {
      unlisten = await listen<DownloadProgressPayload>("download_progress", (e) => {
        const p = e.payload;
        if (p.silent) {
          if (!p.total && !p.downloaded) {
            internal.set(EMPTY);
            busyInstanceIdInternal.set(null);
          }
          return;
        }
        internal.set(p);
        if (p.instance_id) busyInstanceIdInternal.set(p.instance_id);
        if (p.total > 0 && p.downloaded >= p.total) {
          setTimeout(() => busyInstanceIdInternal.set(null), 500);
        }
      });
    } catch (e) {
      console.error("downloadProgressStore: listen failed", e);
    }
  })();
  return initPromise;
}

export function teardownDownloadProgress(): void {
  try {
    unlisten?.();
  } catch {
    /* ignore */
  }
  unlisten = null;
  initPromise = null;
}

export const downloadProgress: Readable<DownloadProgressPayload> = { subscribe: internal.subscribe };

export const busyInstanceId: Readable<string | null> = {
  subscribe: busyInstanceIdInternal.subscribe,
};

/** 0..100 округлённый процент завершения. */
export const downloadPercent: Readable<number> = derived(internal, ($p) =>
  $p.total > 0 ? Math.round(($p.downloaded / $p.total) * 100) : 0,
);

/** true, когда есть активная загрузка (прогресс не 0/0). */
export const showDownload: Readable<boolean> = derived(
  internal,
  ($p) => $p.total > 0 && $p.downloaded < $p.total,
);
