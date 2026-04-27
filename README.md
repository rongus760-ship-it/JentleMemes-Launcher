# JentleMemes Launcher

Лаунчер для Minecraft на базе **Tauri 2** и **Svelte 4**. Управление сборками, модами, серверами и аккаунтами в одном приложении с кастомными темами, Modrinth/CurseForge и переписанным в 2.0 дизайном и оверлеем.

![version](https://img.shields.io/badge/version-2.0.0-green)

> **JentleMemes 2.0 — «Reshape».** Холодный старт ~13 с (с 22 с в 1.x), три оси кастомизации UI (preset × mode × accent), глобальная палитра команд (Ctrl+K), оверлей с HUD + drag-n-drop + скриншотами + хвостом логов. См. [CHANGELOG.md](CHANGELOG.md) секцию `[2.0.0]`.

## Документация

- [docs/GUIDE.md](docs/GUIDE.md) — пользовательский гайд (RU, ~760 строк).
- [docs/LAUNCH_INTERNALS.md](docs/LAUNCH_INTERNALS.md) — внутренности запуска, FluxCore v3, оптимизации (RU, ~720 строк, диаграммы).
- [docs/UI_GUIDE.md](docs/UI_GUIDE.md) — дизайн-система: три оси, токены, компоненты.
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — карта модулей Rust/Svelte.

## Возможности

- **Главная** — быстрый доступ к сборкам, последним серверам и миру, запуск игры в один клик
- **Новости** — автообновляемые новости проекта (отключаемо в настройках)
- **Библиотека** — создание и управление сборками (Vanilla, Fabric, Forge, NeoForge, Quilt), моды, ресурспаки, шейдеры, датапаки
- **Обзор** — поиск и установка модов/сборок/ресурспаков с **Modrinth**, **CurseForge** и **гибридным** режимом (объединённый поиск без дубликатов)
- **Импорт/Экспорт** — `.zip`, `.mrpack` (Modrinth), `.jentlepack` (собственный формат с полной метой)
- **Аккаунты** — офлайн, Ely.by, вход через Microsoft с локальным JWT-expiry-check (не делаем лишних HTTP)
- **Скины** — пресеты скинов (из файла и по нику), 2D/3D просмотр с корректным рендером, slim/default модели, плащи
- **Оформление** — 3 независимые оси: 5 визуальных пресетов (`blend`/`modrinth`/`discord`/`legacy`/`glass`) × тёмный/светлый/авто × 7 акцентных палитр (+ custom HEX) = 105 сочетаний
- **Палитра команд** — Ctrl+K, нечёткий поиск по всему лаунчеру (fuse.js)
- **In-Game Overlay** — HUD поверх Minecraft: виджеты с drag-n-drop, кнопки Стоп/Скрин/Логи, хвост `latest.log`
- **Обновления** — автоматическая проверка и обновление лаунчера через API
- **Наигранное время** — накопление секунд по каждой сборке (карточка в библиотеке)
- **Discord Rich Presence** — опциональный статус «играет» на время сессии

У пользователя должен быть запущен **Discord-клиент**. Переключатель: **Настройки → Discord Rich Presence**.

## Требования

### Для разработки

- **Node.js** 18+
- **Rust** 1.75+ (для сборки Tauri)
- **Linux:** `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`
- **Windows:** WebView2 (встроен в Windows 10+)

### Системные требования (для пользователей)

- **ОС:** Windows 10+, Linux (glibc 2.31+)
- **RAM:** минимум 4 ГБ (рекомендуется 8+ ГБ для Minecraft)
- **Java:** встроенная установка или ручное указание пути

## Сборка и запуск

```bash
# Установка зависимостей
npm install

# Режим разработки (Vite + Tauri)
npm run tauri dev

# Сборка под текущую ОС (все форматы: deb, AppImage, rpm)
npm run tauri build

# Только конкретный формат
npx tauri build --bundles deb
npx tauri build --bundles appimage

# С авто-установкой linuxdeploy в ~/.local/bin (если пишет "failed to run linuxdeploy")
npm run tauri:build:linux
# или только deb / только AppImage
npm run tauri:build:deb
npm run tauri:build:appimage

# Копирование артефактов с расширениями
npm run package:linux
```

Собранное приложение: `src-tauri/target/release/jentlememes-launcher`
Пакеты: `src-tauri/target/release/bundle/`

**Arch Linux:** сам **Tauri** при `tauri build` не собирает пакет pacman; готовый **`*.pkg.tar.zst`** появляется только после **`makepkg`** по `PKGBUILD` — это нормальный формат, менять `PKGBUILD` под расширение не нужно. Обычно достаточно **AppImage** или **бинарника** из `target/release/`. Если нужен **не pacman-пакет**, а простой архив бинарника под админку «Arch / tar.zst»:

```bash
cd src-tauri/target/release
tar -cvf - jentlememes-launcher | zstd -19 -o "JentleMemesLauncher_${VERSION}.tar.zst"
```

Готовые шаблоны для **AUR** (из исходников и `-bin` из AppImage) и пошаговая инструкция: [`packaging/aur/AUR.md`](packaging/aur/AUR.md).


### Сборка Windows-установщика (из Linux)

```bash
# Зависимости (Arch/CachyOS)
sudo pacman -S lld llvm clang
yay -S nsis
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin

# Сборка всего (installer + standalone app)
./scripts/build-windows.sh

# Только standalone .exe
./scripts/build-windows.sh app

# Только NSIS-установщик
./scripts/build-windows.sh setup
```

Результат: `dist/JentleMemes-Launcher-Setup.exe`, `dist/JentleMemesLauncher_<version>.exe`

## Стек

| Часть | Технологии |
|-------|------------|
| Фронтенд | Svelte 4, TypeScript, Vite 5, Tailwind CSS 4, lucide-svelte |
| Бэкенд | Tauri 2 (Rust), reqwest, serde, sha1/sha2, zip, walkdir, tracing |
| Моды/API | Modrinth API v2, CurseForge API v1 |
| 3D | skinview3d (просмотр скинов) |
| Лог | `tracing` + ротируемый файл в `<data_dir>/logs/launcher.YYYY-MM-DD.log` |

См. также [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) и [`docs/MIGRATION_SVELTE5.md`](docs/MIGRATION_SVELTE5.md).

## Структура проекта

```
jentlememes-launcher/
├── src/                        # Svelte 4 приложение
│   ├── App.svelte              # Главный компонент, splash/onboarding/layout
│   ├── Titlebar.svelte         # Кастомный тайтлбар (кнопки окна, drag)
│   ├── themes.ts               # Темы: встроенные, кастомные, авто-генерация по фону
│   ├── index.css               # CSS-переменные тем, `--jm-panel-opacity`, анимации
│   ├── tabs/                   # Вкладки (HomeTab.svelte, LibraryTab.svelte, ChatTab.svelte, ...)
│   ├── lib/                    # Сторы и утилиты (settingsStore, downloadProgressStore, themeApply, ...)
│   ├── components/             # Переиспользуемые UI-блоки (Card, Toggle, ChromeNavigation, ...)
│   └── utils/                  # instanceIcon и др.
│
├── src-tauri/                  # Tauri бэкенд (Rust)
│   ├── src/
│   │   ├── main.rs             # Точка входа, init_tracing, invoke_handler!
│   │   ├── commands.rs         # IPC-команды (декомпозированы в подмодули)
│   │   ├── commands/
│   │   │   ├── window.rs       # close/minimize/maximize/drag/is_maximized
│   │   │   └── backgrounds.rs  # get/pick/copy/delete + read_local_image_data_url
│   │   ├── error.rs            # Серийизуемый Error + diagnostic_report
│   │   ├── config/mod.rs       # LauncherSettings, профили, pack_source (atomic write)
│   │   └── core/
│   │       ├── fluxcore/       # Конвейер запуска: v3 DAG, chain/lib/java resolvers, LaunchCache
│   │       ├── instance.rs     # CRUD сборок
│   │       ├── mods.rs         # Метаданные, toggle, обновления
│   │       ├── modrinth.rs / curseforge.rs / mrpack.rs
│   │       ├── auth.rs         # Ely.by, Microsoft device-code OAuth
│   │       ├── updater.rs      # Self-update (см. SECURITY.md)
│   │       ├── loader_meta/    # Fabric/Forge/NeoForge/Quilt/LiteLoader/ModLoader
│   │       ├── utils/
│   │       │   ├── atomic_fs.rs     # tmp+fsync+bak+rename
│   │       │   ├── path_guard.rs    # Sandbox-резолв путей в IPC
│   │       │   ├── download.rs      # HTTP с SHA-verify
│   │       │   ├── maven.rs / modlauncher.rs / xml_meta.rs / system.rs
│   │       └── game/
│   │           ├── install.rs       # Mojang manifest, библиотеки, ассеты
│   │           └── launch.rs        # argv / spawn / supervisor
│   ├── Cargo.toml
│   ├── rustfmt.toml            # Форматирование
│   └── tauri.conf.json         # Окно, бандлы, CSP
│
├── .github/workflows/ci.yml    # cargo fmt/clippy/test + svelte-check + build (Linux/Windows)
├── .editorconfig
├── vitest.config.ts            # Юнит-тесты FE-утилит
├── docs/
│   ├── ARCHITECTURE.md
│   ├── MIGRATION_SVELTE5.md
│   ├── fluxcore-v3-blueprint.md
│   └── TZ-LAUNCHER-API.md
├── CHANGELOG.md
├── CONTRIBUTING.md
├── SECURITY.md
└── package.json / vite.config.ts / svelte.config.js / tsconfig.json
```

## Данные пользователя

Все данные хранятся в `~/.jentlememes_data/`:

```
~/.jentlememes_data/
├── settings.json               # Настройки лаунчера (RAM, тема, фон, и т.д.)
├── profiles.json               # Аккаунты и пресеты скинов
├── backgrounds/                # Фоновые изображения
└── instances/                  # Сборки
    └── <instance_id>/
        ├── instance.json       # Конфигурация сборки
        ├── mods/               # Моды (.jar, .jar.disabled)
        ├── config/             # Конфигурация модов
        ├── resourcepacks/      # Ресурспаки
        ├── shaderpacks/        # Шейдеры
        ├── saves/              # Миры
        ├── .data/              # Метаданные (mods_meta.json, etc.)
        └── pack_source.json    # Источник сборки (Modrinth/Custom)
```

## API

Лаунчер взаимодействует с `https://jentlememes.ru/launcher/`:

- `GET /launcher/version.json` — информация о последней версии (URL, размер, SHA-256 по платформам)
- `GET /launcher/news.json` — новости проекта

## Лицензия

Проект приватный. Использование — на усмотрение автора.
