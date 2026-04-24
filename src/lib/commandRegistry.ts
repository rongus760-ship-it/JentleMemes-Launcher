import { writable } from "svelte/store";
import type { ComponentType } from "svelte";

/**
 * Глобальный реестр команд для CommandPalette (Ctrl+K).
 *
 * Команды регистрируются любым модулем/табом через `registerCommands(...)`.
 * При размонтировании компонента команды нужно снимать через возвращаемый unregister.
 *
 * Паттерн:
 *   const unregister = registerCommands([
 *     { id: "goto.settings", title: "Настройки", group: "Навигация", run: () => setTab("settings") }
 *   ]);
 *   onDestroy(unregister);
 */
export interface Command {
  id: string;
  title: string;
  description?: string;
  group?: string;
  icon?: ComponentType;
  keywords?: string[];
  shortcut?: string;
  run: () => unknown | Promise<unknown>;
}

export const commands = writable<Command[]>([]);

export function registerCommands(list: Command[]): () => void {
  commands.update((prev) => {
    const ids = new Set(list.map((c) => c.id));
    const next = prev.filter((c) => !ids.has(c.id));
    return [...next, ...list];
  });
  return () => {
    commands.update((prev) => {
      const ids = new Set(list.map((c) => c.id));
      return prev.filter((c) => !ids.has(c.id));
    });
  };
}

export const paletteOpen = writable<boolean>(false);

export function togglePalette() {
  paletteOpen.update((v) => !v);
}

export function openPalette() {
  paletteOpen.set(true);
}

export function closePalette() {
  paletteOpen.set(false);
}
