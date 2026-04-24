# Миграция на Svelte 5 (runes)

Статус: **не выполнена**. Scaffolding готов, миграция требует отдельного PR с визуальной верификацией (17k LOC `.svelte`-кода по вкладкам).

## Почему

Svelte 5 с `$state` / `$derived` / `$effect` устраняет порочные реактивные цепочки, которые сейчас живут в `[src/App.svelte](../src/App.svelte)` и `[src/tabs/SettingsTab.svelte](../src/tabs/SettingsTab.svelte)`: связка `bgPath ↔ settings_updated ↔ applyTheme` раньше давала бесшумные регрессии («авто-тема откатывалась через 30 секунд»). На рунах такие штуки выражаются линейно и без гонок между реактивными блоками.

## Ход миграции (когда возьмёмся)

1. `npx sv migrate svelte-5` в корне проекта — автомигратор покрывает ~80%: `let` → `$state`, `$:` → `$derived` или `$effect`, `export let` → `$props()`, событийные коллбеки, `<slot>` → snippet.
2. Обновить `@sveltejs/vite-plugin-svelte` и `svelte-check` до версий, совместимых с Svelte 5 (проверить после `sv migrate`).
3. Пройтись по ручным кейсам:
  - `[src/App.svelte](../src/App.svelte)` — реактивные блоки вокруг `bgPath` / `settingsStore` / `applyTheme` превратить в явный `$effect(() => { syncWallpaperUiChrome(...) })`.
  - Сторы в `[src/lib/](../src/lib/)` (settingsStore, downloadProgressStore) — API совместим, но `$`-префикс подписки становится нативным через `$store`.
  - `[src/tabs/ChatTab.svelte](../src/tabs/ChatTab.svelte)`, `[src/tabs/LibraryTab.svelte](../src/tabs/LibraryTab.svelte)`, `[src/tabs/DiscoverTab.svelte](../src/tabs/DiscoverTab.svelte)` — god-компоненты с десятками `$:` блоков. Лучше сначала разбить по Phase 3.2, потом мигрировать отдельно каждый под-компонент.
4. `npm run check` + ручной прогон всех вкладок (Home / News / Library / Discover / Skins / Chat / Settings / Advanced Settings) + тема `auto-bg` + смена фона + установка модпака.
5. Деплой — бандл Svelte 5 меньше и быстрее, ожидаемый выигрыш по первому рендеру ~10-20%.

## Почему не делаем сейчас

- Автомигратор — не серебряная пуля. Часть магии (`$$props`, reactive blocks с `await`, компоненты с двумя экспортами) требует ручной работы.
- Без `npm run tauri dev` невозможно гарантировать, что UI не сломан. Этот PR посвящён фазам 1-3 (производительность запуска, логи, тесты, декомпозиция).
- Гонки настроек, которые Svelte 5 лечит идиоматично, уже устранены инженерно через `[settingsStore](../src/lib/settingsStore.ts)` + новую Rust-команду `patch_settings`.

## Предусловия, выполненные в этом PR

- ✅ Единый стор настроек (Svelte 4, совместим с 5)
- ✅ Единый стор прогресса загрузки
- ✅ Выделение `localImageUrl`, `themeApply` в `src/lib/` (готово к `$effect`)
- ✅ vitest + testing scaffolding (работает и в Svelte 5)

