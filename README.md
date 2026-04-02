# JentleMemes Launcher

Лаунчер для Minecraft на базе **Tauri 2** и **React**. Управление сборками, модами, серверами и аккаунтами в одном приложении с поддержкой кастомных тем, Modrinth и CurseForge.

![version](https://img.shields.io/badge/version-1.0.5--beta.1-green)

## Возможности

- **Главная** — быстрый доступ к сборкам, последним серверам и миру, запуск игры в один клик
- **Новости** — автообновляемые новости проекта (отключаемо в настройках)
- **Библиотека** — создание и управление сборками (Vanilla, Fabric, Forge, NeoForge, Quilt), моды, ресурспаки, шейдеры, датапаки
- **Обзор** — поиск и установка модов/сборок/ресурспаков с **Modrinth**, **CurseForge** и **гибридным** режимом (объединённый поиск без дубликатов)
- **Импорт/Экспорт** — `.zip`, `.mrpack` (Modrinth), `.jentlepack` (собственный формат с полной метой)
- **Аккаунты** — офлайн, Ely.by, вход через Microsoft (device code flow)
- **Скины** — пресеты скинов (из файла и по нику), 2D/3D просмотр с корректным рендером, slim/default модели, плащи
- **Темы** — 8 встроенных тем + кастомные (простой и расширенный режим) + авто-тема по фоновому изображению
- **Обновления** — автоматическая проверка и обновление лаунчера через API
- **Наигранное время** — накопление секунд по каждой сборке (карточка в библиотеке)
- **Discord Rich Presence** — опциональный статус «играет» на время сессии (см. ниже)

### Discord Rich Presence (разработчикам)

1. Создайте приложение в [Discord Developer Portal](https://discord.com/developers/applications) → скопируйте **Application ID** (не секрет).
2. Вставьте ID в константу `DISCORD_APPLICATION_ID` в файле [`src-tauri/src/core/discord_presence.rs`](src-tauri/src/core/discord_presence.rs). Пока строка пустая, Rich Presence не подключается к IPC (настройка в лаунчере не навредит).
3. (Опционально) В разделе **Rich Presence → Art Assets** загрузите картинки и укажите их ключи в коде через `.assets(...)` у активности.

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

**Arch Linux:** Tauri не выдаёт готовый `.pkg.tar.zst`. Обычно достаточно **AppImage** или **бинарника** из `target/release/`. Если нужен архив под админку «Arch / tar.zst»:

```bash
cd src-tauri/target/release
tar -cvf - jentlememes-launcher | zstd -19 -o "JentleMemesLauncher_${VERSION}.tar.zst"
```

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
| Фронтенд | React 19, TypeScript, Vite 7, Tailwind CSS 4, Framer Motion, Lucide Icons |
| Бэкенд | Tauri 2 (Rust), reqwest, serde, sha1/sha2, zip, walkdir |
| Моды/API | Modrinth API v2, CurseForge API v1 |
| 3D | skinview3d (просмотр скинов) |

## Структура проекта

```
jentlememes-launcher/
├── src/                        # React-приложение
│   ├── App.tsx                 # Главный компонент, маршрутизация вкладок, тема
│   ├── Titlebar.tsx            # Кастомный тайтлбар (кнопки окна, drag)
│   ├── themes.ts               # Темы: встроенные, кастомные, авто-генерация, цвета из фона
│   ├── index.css               # CSS: переменные тем, анимации, глобальные стили
│   ├── tabs/
│   │   ├── HomeTab.tsx         # Главная: быстрый запуск, серверы, миры
│   │   ├── NewsTab.tsx         # Новости проекта
│   │   ├── LibraryTab.tsx      # Сборки: создание, моды, экспорт/импорт, настройки
│   │   ├── DiscoverTab.tsx     # Браузер модов: Modrinth, CurseForge, гибрид
│   │   ├── SettingsTab.tsx     # Настройки: RAM, Java, темы, фоны, обновления
│   │   ├── SkinsTab.tsx        # Скины: пресеты, 2D/3D просмотр, загрузка файлов
│   │   └── AccountTab.tsx      # Аккаунты: офлайн, Ely.by, Microsoft OAuth
│   └── components/
│       ├── AnimatedSection.tsx  # Компонент анимированных секций
│       └── LoaderIcon.tsx      # Иконки загрузчиков (Fabric, Forge, etc.)
│
├── src-tauri/                  # Tauri бэкенд (Rust)
│   ├── src/
│   │   ├── main.rs             # Точка входа, регистрация команд
│   │   ├── commands.rs         # Tauri-команды (invoke handlers)
│   │   ├── error.rs            # Типы ошибок
│   │   ├── config/
│   │   │   └── mod.rs          # Настройки, профили, конфиг сборок, pack_source
│   │   └── core/
│   │       ├── instance.rs     # Создание/удаление/переименование сборок
│   │       ├── mods.rs         # Сканирование модов, метаданные, toggle, удаление
│   │       ├── modrinth.rs     # Modrinth API: поиск, проекты, версии, установка
│   │       ├── curseforge.rs   # CurseForge API: поиск, проекты, установка
│   │       ├── mrpack.rs       # Импорт/экспорт .mrpack и .zip, обновление сборок
│   │       ├── auth.rs         # Авторизация: офлайн, Ely.by, Microsoft device code
│   │       ├── updater.rs      # Обновления лаунчера (API, скачивание, применение)
│   │       ├── api.rs          # HTTP-клиент, новости API
│   │       └── game/
│   │           ├── mod.rs      # Скачивание файлов, установка загрузчиков
│   │           └── launch.rs   # Запуск Minecraft, мониторинг процесса
│   ├── Cargo.toml
│   ├── tauri.conf.json         # Конфигурация Tauri (окно, бандлы, CSP)
│   └── custom_packs.json       # Встроенные кастомные сборки
│
├── installer/                  # Tauri мини-приложение (красивый установщик для Windows)
│   ├── src/                    # React UI: Welcome, Path, Progress, Finish
│   └── src-tauri/              # Rust: извлечение файлов, реестр, ярлыки
│
├── installer-stub/             # NSIS-обёртка (self-extracting .exe)
│   └── stub.nsi
│
├── scripts/
│   ├── build-windows.sh        # Кросс-компиляция для Windows из Linux
│   └── package-linux-artifacts.sh  # Копирование артефактов с расширениями
│
├── dist-artifacts/             # Артефакты сборки (gitignored)
├── package.json
├── tsconfig.json
├── vite.config.ts
└── tailwind.config.ts
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
