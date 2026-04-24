# Changelog

Формат: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Проект использует SemVer для пользовательских сборок.

## [2.0.0] — JentleMemes 2.0 — «Reshape»

Мажорный релиз: переработан UI, запуск ускорен до ~13 с, оверлей полностью переписан.

### Added — Phase A (Launch performance, 22 с → 12.9 с)

- **Локальный JWT expiry check** в `refresh_account_for_launch` (`src-tauri/src/commands.rs`): если Microsoft-токен не истёк — функция возвращается мгновенно, без 5 HTTP-запросов. Раньше это было главным виновником 10+ с «тишины» на warm path.
- **Параллелизация** `prepare_launch` и `refresh_account_for_launch` во фронте (`src/tabs/LibraryTab.svelte`) через `Promise.allSettled`. Операции теперь идут одновременно, а не последовательно.
- **G1 GC тюнинг**: `-XX:G1NewSizePercent=20 -XX:G1ReservePercent=20 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=32M`. `-XX:+UnlockExperimentalVMOptions` гарантированно ставится ПЕРЕД экспериментальными G1-флагами (иначе JVM падал с «VM option is experimental»).
- **Log4j fast-init**: `-Dlog4j2.formatMsgNoLookups=true -Dlog4j.skipJansi=true`.
- **Netty reflection**: `-Dio.netty.tryReflectionSetAccessible=true`.
- **Phase tracing**: `⏱ phase=<name> Δ=<ms> total=<ms>` выводится в UI-лог инстанса (`init`, `resolve_chain`, `cache_check`, `java_and_args`, `jvm_spawn`). `first_line_emitted` измеряет время до первой строки stdout Java.

### Added — Phase B (UI 3-axis design system)

- **Трёхосевая архитектура оформления**: `data-preset` × `data-mode` × `class="theme-*"` на `<html>`. Даёт 5 × 2 × 7 = 70 штатных сочетаний (+ кастомный HEX → 105).
- **5 визуальных пресетов**: `blend` (default), `modrinth` (просторный), `discord` (плотный), `legacy` (визуал 1.1.0 с орбами и sheen), `glass` (акрил + blur).
- **Полноценная светлая тема** — не «подкрашенная тёмная», отдельная палитра на каждом акценте.
- **Настоящий 3-layer CSS**: глобальные токены (typography, motion, spacing) → структурные (radius, shadow, density) → цветовые (accent, bg, surface, text).
- Инит `@fontsource/inter` + `@fontsource/jetbrains-mono` для всего приложения.
- Новые UI-компоненты: `Badge`, `Tooltip`, `Dialog`, `ProgressRing`, `EmptyState`, `Skeleton`, `Tabs`.
- Файл `src/styles/preset-legacy.css` — порт визуала 1.1.0 как отдельного пресета.

### Added — Phase C (Command Palette)

- **Ctrl+K палитра команд** (`src/components/CommandPalette.svelte`) с fuzzy-поиском через `fuse.js`. Быстрый переход на любой таб, смена темы, открытие папки данных, запуск проверок обновлений — всё из одного места.
- Реестр команд `src/lib/commandRegistry.ts` — любой таб может зарегистрировать свои команды и снять их при unmount.

### Added — Phase D (Overlay rewrite)

- **Полностью переписанный `IngameOverlay.svelte`** с панелью управления:
  - кнопка **Стоп** (`stop_game_from_overlay`) — убить сессию не открывая лаунчер;
  - кнопка **Скрин** (`take_minecraft_screenshot`) — скриншот через `grim`/`maim`/`scrot`/`import` (Linux) или PowerShell + `System.Drawing` (Windows);
  - кнопка **Логи** — хвост `logs/latest.log` с обновлением раз в 1.5 с и кнопкой копирования.
- **Drag-n-drop виджетов**: хват за иконку GripVertical, порядок сохраняется в `localStorage`.
- **Toast-нотификации** в оверлее для действий управления.

### Added — Phase E (Onboarding + Splash)

- **Анимированный сплеш-экран** (`src/SplashScreen.svelte`): чек-лист фаз с пульсирующим индикатором, 3 анимированных орба, звёздный фон, ротация 8 tips, brand-ring анимация. Уважает `jm-reduce-motion`.
- **+1 шаг в OnboardingWizard**: «Визуал» — выбор визуального пресета с превью-плитками.
- Онбординг теперь сохраняет `visual_preset` в `settings.json`.

### Added — Phase F (Docs)

- `docs/GUIDE.md` (~765 строк, RU) — полный пользовательский гайд: установка, первый запуск, сборки, запуск, оверлей, оформление, настройки, типовые проблемы, структура data-dir.
- `docs/LAUNCH_INTERNALS.md` (~720 строк, RU) — внутреннее устройство запуска: FluxCore v3, CoilCache, параллелизм, JVM-аргументы, путь оптимизации с диаграммами mermaid.
- `docs/UI_GUIDE.md` (~180 строк, RU) — дизайн-система: три оси, структура токенов, компоненты, пример кастомного пресета.
- `README.md` обновлён под 2.0 (фичи, запуск, ссылки на новые доки).

### Backend commands (new)

- `stop_game_from_overlay(instance_id: Option<String>) -> Result<(), String>`
- `take_minecraft_screenshot(instance_id: Option<String>) -> Result<String, String>`
- `tail_game_log(instance_id: String, lines: Option<usize>) -> Result<String, String>`

### Changed

- `src-tauri/src/config/mod.rs` → `LauncherSettings`: добавлено поле `visual_preset: String` с `default = "blend"`.
- `src/lib/themeApply.ts`: добавлены `VisualPreset` тип, `ALL_VISUAL_PRESETS`, `normalizeVisualPreset()`, `applyVisualPreset()`. `applyTheme` теперь гарантирует `data-preset="blend"`, если пресет не задан явно.
- `src/index.css` полностью переписан на 3-layer токены. Старый `@theme` блок и `:root` variables заменены на `html[data-preset=...]` и слой глобальных токенов.
- `src/App.svelte`: интегрирован `CommandPalette`, зарегистрированы app-wide команды, сплеш получает `phases` + `phaseIndex`.
- `src/Titlebar.svelte`: добавлена кнопка «Quick Search (Ctrl+K)» и класс `jm-titlebar` для legacy-специфики.
- `src/components/ui/SectionNav.svelte`: у каждого элемента теперь `data-section={id}` — для прямой навигации из палитры.

### Миграция 1.1.1 → 2.0.0

Полностью **совместимо** с сохранёнными данными (сборки, аккаунты, настройки). При первом старте 2.0:

1. `settings.visual_preset` установится в `"blend"` автоматически.
2. Старые темы `theme-*` продолжат работать как акцентная палитра.
3. Повторный Onboarding **не** запускается — только для новых установок.

Если хотите заново пройти мастер — удалите `settings.onboarding_completed` в `settings.json` или `FORCE_ONBOARDING=1` при запуске.

---

## [Unreleased] — full-stack rehab

### Added — Phase 1 (FluxCore v3 Warm Path)

- Реализован тёплый путь запуска: `LaunchCache` (`<game_dir>/launch_cache.json`) хранит `classpath`, `java_path`, `asset_index`, `chain_hash`, `settings_hash`. При совпадении хешей пропускаются forge-universals resolve и обновление каталога natives.
- `classpath_hint: Option<ClasspathSnapshot>` прокидывается из `fluxcore::v3::runner` в `game::launch::launch`.
- Параллельное вычисление `profile_inputs_digest` и `load_snapshot` через `tokio::join!` + `spawn_blocking`.
- Батчинг лог-эмитов: `LogFlushGuard` буферизует `app.emit("log_<id>", ...)` до 8 строк или 150 мс; финальный flush — через `Drop`.
- Перенос идемпотентного `promote_forge_wrapper_to_bootstrap` в `install::download_game_files`, чтобы launch не делал этот шаг на холодном старте.
- Подключение готовых resolver-ов (`chain_resolver`) из `fluxcore/` в основной конвейер launch.

### Added — Phase 2 (Platform Hygiene)

- Структурированное логирование через `tracing` + `tracing-appender` с ежедневной ротацией в `<data_dir>/logs/launcher.YYYY-MM-DD.log`. Уровень управляется `JM_LOG`.
- Новая IPC-команда `patch_settings(delta)` — атомарный JSON-merge на бэкенде, устраняет гонки между вкладками. На фронте — единый `src/lib/settingsStore.ts`.
- Тип `CmdResult<T> = Result<T, Error>` для IPC — возвращает `{ message, detail }` вместо плоской строки; работает через `?` с `io / reqwest / serde_json`.
- `core/utils/atomic_fs.rs` — запись конфигов через `<path>.tmp` + `fsync` + `rename`, снапшот в `<path>.bak`. Применено к `settings.json`, `profiles.json`, `pack_source.json`, `servers.json`, `last_world.json`, `instance.json` (при создании).
- `.github/workflows/ci.yml` — матричный CI (Linux/Windows), `cargo fmt/clippy/test`, `svelte-check`, `vite build`.
- `rustfmt.toml`, `.editorconfig` — унификация стиля.
- Rust-тесты: `core/utils/path_guard` (sandbox), `core/loader_meta/version_sort` (MC-сортировка), `core/utils/atomic_fs` (tmp/bak).
- Frontend-тесты: vitest-конфиг + `src/lib/chromeLayout.test.ts`, `src/lib/mcVersionSort.test.ts`.

### Added — Phase 3 (Decomposition scaffold)

- Начата декомпозиция `commands.rs` (2320 LOC): вынесены `commands/window.rs` и `commands/backgrounds.rs`, установлен паттерн для follow-up PR (settings / instances / mods / auth / game / modrinth / curseforge).
- Выделен `src/lib/downloadProgressStore.ts` — единый стор прогресса загрузки (writable + derived), готов к перехвату из `App.svelte` и `LibraryTab.svelte`.

### Added — Phase 4 (Polish)

- `[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)` — модульная карта Rust/Svelte, IPC-контракт, конвейер FluxCore v3.
- `[docs/MIGRATION_SVELTE5.md](docs/MIGRATION_SVELTE5.md)` — roadmap миграции на Svelte 5 runes (не выполнено в этом PR — требует визуальной верификации UI).
- `[CONTRIBUTING.md](CONTRIBUTING.md)`, `[SECURITY.md](SECURITY.md)` — стандартные стартовые документы.

### Changed

- CSP в `tauri.conf.json` ужесточён: `http:` удалён из `connect-src`, `frame-src`, `font-src`, `media-src`, `worker-src`. Интернал-трафик разрешён только через `https:`.
- README переписан: в стеке указан Svelte 4 (ранее ошибочно упоминался React), обновлено дерево проекта.
- Конфиги в `src-tauri/src/config/mod.rs` пишутся через `atomic_fs::write_atomic_string`.

### Fixed

- Гонка сохранений настроек между `SettingsTab.svelte`, `AdvancedSettingsTab.svelte` и `App.svelte` (проявлялась как «автотема откатывается к дефолтной через 30 секунд»). Решено через единый writer `patch_settings`.
- `extractColorsFromImage` для `data:`/`blob:` URL больше не выставляет `crossOrigin=anonymous`, что ломало декодирование на WebKit и давало fallback-цвет `#86A886`.

### Known issues / Deferred

- Полная декомпозиция `install.rs` (2682 LOC), `launch.rs` (2645 LOC), `mods.rs` (1403 LOC), `mrpack.rs` (1496 LOC) и больших `.svelte` (`ChatTab` 3639, `LibraryTab` 2948, `DiscoverTab` 2368) — follow-up PR.
- Миграция на Svelte 5 — follow-up PR (см. `docs/MIGRATION_SVELTE5.md`).
- Сужение `assetProtocol.scope` под per-user `<data_dir>` — follow-up.
- `zip = "0.6"` → `2.x`, `reqwest = "0.11"` → `0.12` — в отдельном bump-PR (ломающие изменения API).

## [1.1.1] — предыдущий

- Начальная стабильная версия до rehab-фаз.

