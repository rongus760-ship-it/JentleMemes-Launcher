# JentleMemes Launcher

Лаунчер для Minecraft на базе Tauri 2 и React. Управление сборками, модами, серверами и аккаунтами в одном приложении с тёмной темой и поддержкой Modrinth и CurseForge.

![version](https://img.shields.io/badge/version-0.1.0-green)

## Возможности

- **Главная** — быстрый доступ к сборкам, последним серверам и миру, запуск игры в один клик
- **Библиотека** — создание и управление сборками (Fabric, Forge, NeoForge, Quilt), моды, ресурспаки, шейдеры, датапаки
- **Обзор** — поиск и установка модов/сборок/ресурспаков с **Modrinth**, **CurseForge** и **гибридным** режимом (объединённый поиск без дубликатов)
- **Установка .mrpack** — импорт и обновление сборок из файлов и по URL
- **Аккаунты** — офлайн, Ely.by, вход через Microsoft (device code)
- **Скины** — пресеты скинов и просмотр 3D
- **Настройки** — RAM, Java, JVM-аргументы, обёртка (mangohud, gamemoderun), темы

Данные лаунчера хранятся локально (сборки, настройки, недавние серверы/мир).

## Требования

- **Node.js** 18+
- **Rust** (для сборки Tauri)
- **Зависимости под Linux:** WebKitGTK (для Tauri), пакеты для сборки Rust

## Сборка и запуск

```bash
# Установка зависимостей
npm install

# Режим разработки (Vite + Tauri)
npm run tauri dev

# Сборка под текущую ОС
npm run tauri build
```

Собранное приложение появится в `src-tauri/target/release/` (или `debug` при `tauri dev`).

### Сборка под Windows (из Linux)

```bash
npm run build:win
```

Нужен настроенный таргет `x86_64-pc-windows-gnu` и скрипт `scripts/fetch-webview2-loader.sh` при необходимости.

## Стек

| Часть      | Технологии |
|-----------|------------|
| Фронтенд  | React 19, TypeScript, Vite 7, Tailwind CSS 4, Framer Motion, Lucide |
| Бэкенд    | Tauri 2 (Rust) |
| Моды/API  | Modrinth API, CurseForge API (v1) |

## Структура проекта

```
jentlememes-launcher/
├── src/                 # React-приложение (вкладки, компоненты)
│   ├── tabs/            # HomeTab, LibraryTab, DiscoverTab, SettingsTab, AccountTab, SkinsTab
│   └── App.tsx
├── src-tauri/           # Tauri (Rust)
│   └── src/
│       ├── commands.rs  # Команды для фронта
│       ├── config/      # Настройки, профили, сборки
│       └── core/        # instance, mods, modrinth, curseforge, mrpack, auth, game, …
├── package.json
└── tauri.conf.json
```

## Лицензия

Проект приватный. Использование — на усмотрение автора.
