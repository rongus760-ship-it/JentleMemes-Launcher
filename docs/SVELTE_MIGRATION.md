# Миграция на Svelte

## Сейчас

- **Весь основной UI лаунчера на Svelte**: `App.svelte`, `Titlebar.svelte`, вкладки **Главная**, **Новости**, **Аккаунты**, **Скины**, **Настройки**, **Сборки** (`LibraryTab.svelte`), **Браузер** (`DiscoverTab.svelte`, `marked` + DOMPurify где нужно).
- Встроенный мод-браузер в сборке: `<DiscoverTab />` из `.svelte` внутри `LibraryTab.svelte` (без дубликата на React).

## Визуал и анимации

- `index.css`: `.jm-app-shell`, `.jm-breathe`, `.jm-toast-glow`, `.jm-nav-pill`, `.jm-reveal`, `.card-hover-subtle`, `.discover-md`, и т.д.
- Переходы Svelte: `fade`, `fly`, `scale` на вкладках, карточках сборок, модалках, панели деталей инстанса (`{#key}` + `fly`).

## Сборка

`npm run dev`, `npm run build`, `npm run check`, `npm run tauri dev`.

## Установщик (отдельный пакет)

Каталог `installer/` может по-прежнему использовать свой React/Vite — не смешивать с корневым `src/`.
