# Contributing to JentleMemes Launcher

Спасибо за интерес к проекту. Коротко: правим, проверяем, пушим в ветку, открываем PR. Подробности ниже.

## Быстрый старт

```bash
git clone https://github.com/<org>/jentlememes-launcher.git
cd jentlememes-launcher
npm install
npm run tauri dev          # Vite + Tauri dev-сервер
```

## Проверки перед PR

```bash
# Frontend
npm run check               # svelte-check
npm run build               # vite build

# Backend
cd src-tauri
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --bin jentlememes-launcher
```

CI делает то же самое на Linux и Windows (`.github/workflows/ci.yml`).

## Стиль кода

- Rust: `rustfmt` с [`src-tauri/rustfmt.toml`](src-tauri/rustfmt.toml). `cargo clippy -- -D warnings` без исключений.
- TS / Svelte: 2 пробела, Prettier-совместимо, `svelte-check` без ошибок.
- Комментарии в коде объясняют **намерение/компромисс/нетривиальный инвариант**, не «что делает строка». Предпочтение русскому языку для пользовательских сообщений и комментариев; код/имена идентификаторов на английском.
- Не коммитьте файлы с секретами (`.env`, `credentials.json`). CI не хранит секреты в репозитории.

## Коммиты и PR

- Маленькие фокусные коммиты. Один PR = одна цель.
- Заголовок commit/PR: `область: краткое описание` (примеры: `launch: wire LaunchCache warm-path`, `settings: atomic write + .bak`).
- Если правка влияет на пользователя — запись в [`CHANGELOG.md`](CHANGELOG.md) в раздел `[Unreleased]`.

## Backend

- Новые IPC-команды оформлять как `-> CmdResult<T>` (см. [`src-tauri/src/commands.rs`](src-tauri/src/commands.rs)) вместо `Result<_, String>`.
- Писать конфиги через `core::utils::atomic_fs::write_atomic_string`, иначе риск корраптового JSON при крахе процесса.
- Пути из IPC-аргументов проходить через `path_guard::sanitize_path_within(raw, &allowed_root)`.
- Новый long-running код — покрывать тестами в том же файле (`#[cfg(test)] mod tests`).

## Frontend

- Настройки читать/писать через [`src/lib/settingsStore.ts`](src/lib/settingsStore.ts) (`patchSettings`), не через прямые `invoke("save_settings", ...)`.
- Сторы и хелперы — в `src/lib/`. Большие компоненты разрезать на вкладки-подмодули (см. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)).
- Юнит-тесты утилит — vitest (`src/**/*.test.ts`).

## Документация

- Архитектурные изменения фиксировать в [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
- Новые хоткеи, комaнды, опции — в README.
- Известные риски безопасности — в [`SECURITY.md`](SECURITY.md).

## Вопросы

- Issues GitHub — для багов и фич.
- FluxCore архитектура обсуждается в [`docs/fluxcore-v3-blueprint.md`](docs/fluxcore-v3-blueprint.md).
