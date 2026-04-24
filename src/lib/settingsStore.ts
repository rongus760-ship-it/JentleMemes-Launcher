import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Единый стор настроек лаунчера.
 *
 * Проблема, которую решает: разные вкладки (SettingsTab, AdvancedSettingsTab, App.svelte)
 * раньше самостоятельно делали `load_settings` → мутация объекта → `save_settings`.
 * Параллельные записи давали гонку: если сохранение одной вкладки перекрывало изменение другой,
 * тема «откатывалась» к дефолту (баг «зелёная тема через 30 секунд»).
 *
 * Теперь единственный writer в backend — команда `patch_settings`, которая атомарно мержит
 * дельту в сохранённый JSON и эмитит событие `settings_updated`. Стор подписывается на событие
 * и подтягивает актуальные настройки после чужих изменений.
 */
export type Settings = Record<string, unknown>;

const internal = writable<Settings>({});

let ready: Promise<void> | null = null;
let listenerInstalled = false;

async function ensureInitialLoad(): Promise<void> {
  if (ready) return ready;
  ready = (async () => {
    try {
      const s = await invoke<Settings>("load_settings");
      internal.set(s || {});
    } catch (e) {
      console.error("settingsStore: load_settings failed", e);
      internal.set({});
    }
  })();
  return ready;
}

async function installChangeListener(): Promise<void> {
  if (listenerInstalled) return;
  listenerInstalled = true;
  try {
    await listen("settings_updated", async () => {
      try {
        const s = await invoke<Settings>("load_settings");
        internal.set(s || {});
      } catch {
        /* ignore */
      }
    });
  } catch (e) {
    console.warn("settingsStore: listen(settings_updated) failed", e);
  }
}

/** Подписка — ждите первую загрузку перед чтением значений. */
export const settings = {
  subscribe: internal.subscribe,
};

/** Убедиться, что стор проинициализирован (вызвать однажды из App.svelte onMount). */
export async function initSettingsStore(): Promise<void> {
  await ensureInitialLoad();
  await installChangeListener();
}

/**
 * Атомарно обновить часть настроек. Возвращает полный merged-объект.
 * Пример: `await patchSettings({ theme: "purple-dark", background: "" })`.
 */
export async function patchSettings(delta: Partial<Settings>): Promise<Settings> {
  const merged = await invoke<Settings>("patch_settings", { delta });
  internal.set(merged);
  return merged;
}

/** Получить текущий snapshot стора синхронно (после `initSettingsStore`). */
export function getSettingsSnapshot(): Settings {
  return get(internal);
}
