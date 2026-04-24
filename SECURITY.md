# Security Policy

Лаунчер запускает сторонний Java-код (моды Minecraft) и скачивает бинарные артефакты по сети. Это делает его нетривиальной поверхностью атаки. Документ фиксирует текущие инварианты и известные риски.

## Отчёт об уязвимости

Приватно — контакт: `security@jentlememes.ru` (или issue с пометкой `security` с минимумом деталей в публичной части).

## Threat model

- **Сеть MITM.** Лаунчер качает моды, JRE, Mojang-манифесты и своё же обновление. Все загрузки идут по `https://`, CSP запрещает `http:` в `connect-src` / `frame-src` (см. [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json)).
- **Вредоносный мод-контент.** Лаунчер верифицирует SHA-1 / SHA-256 по манифесту там, где провайдер его выдаёт (Mojang, Modrinth). CurseForge выдаёт только длину — контент проверяется минимально.
- **Попытка выйти из sandbox через IPC.** Все команды с аргументом-путём проходят через `core::utils::path_guard::sanitize_path_within(raw, &allowed_root)`; попытка `../../etc/passwd` возвращает `Error::Custom("Path escapes sandbox")`. Покрыто тестами.
- **Перезапись чужого конфига.** Конфиги пишутся через `atomic_fs::write_atomic_string` (tmp + fsync + rename), с `<path>.bak` перед каждой записью.
- **Вредоносный авто-updater.** Сейчас `core/updater.rs` — самописный. SHA-256 проверка планируется в follow-up (см. [`CHANGELOG.md`](CHANGELOG.md) → Deferred).

## Sandbox-каталоги

Все пользовательские данные живут в `<data_dir>` (по умолчанию `~/.jentlememes_data/`, см. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)). IPC-команды ограничены этим каталогом или его подкаталогами (backgrounds, instances, java).

## CSP

Текущее значение (`src-tauri/tauri.conf.json`):

```
default-src 'self' 'unsafe-inline';
script-src 'self' 'unsafe-inline';
img-src 'self' asset: http://asset.localhost https://asset.localhost https: data: blob:;
media-src 'self' asset: http://asset.localhost https://asset.localhost blob: mediastream: data: https:;
connect-src 'self' https:;
font-src 'self' data: https:;
style-src 'self' 'unsafe-inline' https:;
frame-src https:;
worker-src 'self' blob' https:;
```

- `unsafe-inline` для `script-src` / `style-src` остаётся ради совместимости с Svelte/Vite-HMR; убирается, когда мигрируем на Svelte 5 и применяем nonce/hash.
- `http:` разрешён только в `img-src` через `http://asset.localhost` (Windows-схема asset-протокола Tauri).

## Не выполнено / в работе

- Сужение `assetProtocol.scope` (`**/*` → `<data_dir>/backgrounds/**`, `<data_dir>/instances/**/icons/**`) — требует per-user path expansion.
- Whitelist доменов для `connect-src` (api.modrinth.com / api.curseforge.com / piston-*.mojang.com / libraries.minecraft.net / authserver.ely.by / sessionserver.mojang.com / skin.ely.by / textures.minecraft.net / jentlememes.ru / api.github.com).
- SHA-256 verification + code signing для обновлений лаунчера.
- Миграция `zip = "0.6"` → `2.x` (0.6 имеет устаревающие CVE).

## Пользователю

- Лаунчер **выполняет Java** с аргументами из JSON-манифестов и установленных модов. Не устанавливайте моды из непроверенных источников.
- Обновления — только через официальный API `jentlememes.ru/launcher/version.json`. Если видите обновление из стороннего источника, сообщите в issue.
