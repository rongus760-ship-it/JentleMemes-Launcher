# ТЗ: API новостей, обновлений и автообновления JentleMemes Launcher

> Версия документа: 1.0  
> Дата: 2026-03-14  
> Сайт проекта: `jentlememes.ru`

---

## 1. Обзор

Лаунчер должен при запуске:
1. Загружать **новости** с сервера и показывать их на вкладке "Главная".
2. Проверять наличие **обновлений** для текущей ОС.
3. Если есть новая версия — показывать пользователю диалог обновления и выполнять автообновление с верификацией SHA-1.

---

## 2. Серверная сторона (jentlememes.ru)

### 2.1. Структура файлов на сервере

Все файлы размещаются как **статические JSON/бинарные файлы** на `jentlememes.ru` без необходимости в динамическом бэкенде.

```
https://jentlememes.ru/api/launcher/
├── news.json                    ← Лента новостей
├── version.json                 ← Текущая версия + хеши + URL для каждой ОС
└── releases/
    └── v0.2.0/                  ← Директория для каждого релиза
        ├── jentlememes-launcher-linux-x86_64             ← Бинарник Linux
        ├── jentlememes-launcher-linux-x86_64.sha1        ← SHA-1 хеш (текстовый файл)
        ├── JentleMemes-Launcher-Setup.exe                ← Установщик Windows
        ├── JentleMemes-Launcher-Setup.exe.sha1           ← SHA-1 хеш
        ├── jentlememes-launcher-linux-x86_64.tar.gz      ← (опционально) архив Linux
        └── jentlememes-launcher-linux-x86_64.tar.gz.sha1
```

### 2.2. Настройка веб-сервера

Если сайт работает на **nginx**, добавьте location-блок:

```nginx
location /api/launcher/ {
    alias /var/www/jentlememes.ru/launcher-api/;
    add_header Access-Control-Allow-Origin "*";
    add_header Cache-Control "public, max-age=300";  # Кеш 5 минут
    types {
        application/json json;
        application/octet-stream exe;
    }
    autoindex off;
}
```

Если используется **какой-либо фреймворк** (Next.js, PHP и т.д.), можно просто отдавать файлы из папки `public/api/launcher/` или создать API-маршруты.

---

## 3. Форматы данных

### 3.1. `news.json` — Лента новостей

```json
{
  "news": [
    {
      "id": "2026-03-14-update-02",
      "title": "Обновление 0.2.0 — Темы и кастомные фоны!",
      "body": "Мы добавили 8 встроенных тем, систему кастомных фонов и новый экран настроек.\n\nПодробнее читайте в нашем Discord.",
      "image": "https://jentlememes.ru/api/launcher/images/update-0.2.0-banner.png",
      "date": "2026-03-14T18:00:00Z",
      "url": "https://jentlememes.ru/news/update-0.2.0",
      "tags": ["update", "feature"],
      "pinned": true
    },
    {
      "id": "2026-03-08-site-launch",
      "title": "МЫ ОТКРЫЛИ САЙТ!",
      "body": "Спасибо за поддержку всем кто был рядом при создании",
      "image": "https://jentlememes.ru/api/launcher/images/site-launch.png",
      "date": "2026-03-08T15:48:38Z",
      "url": "https://jentlememes.ru/news/site-launch",
      "tags": ["announcement"],
      "pinned": false
    }
  ]
}
```

| Поле      | Тип      | Обязательно | Описание |
|-----------|----------|-------------|----------|
| `id`      | string   | да          | Уникальный ID новости (формат: `YYYY-MM-DD-slug`) |
| `title`   | string   | да          | Заголовок |
| `body`    | string   | да          | Текст (поддерживается `\n` для переносов) |
| `image`   | string?  | нет         | URL баннера (рекомендация: 1200×630px) |
| `date`    | string   | да          | Дата в формате ISO 8601 |
| `url`     | string?  | нет         | Ссылка "Подробнее" (открывается в браузере) |
| `tags`    | string[] | нет         | Теги: `update`, `feature`, `announcement`, `event`, `bugfix` |
| `pinned`  | boolean  | нет         | Закреплённая новость (всегда первая) |

---

### 3.2. `version.json` — Реестр версий и обновлений

```json
{
  "latest": "0.2.0",
  "minimum_supported": "0.1.0",
  "urgent": false,
  "changelog": "### 0.2.0\n- Система тем (8 встроенных + кастомные)\n- Кастомные фоны\n- Удалён звук переключения вкладок\n- Фикс кнопок тайтлбара\n\n### 0.1.0\n- Первый релиз",
  "platforms": {
    "windows-x86_64": {
      "version": "0.2.0",
      "url": "https://jentlememes.ru/api/launcher/releases/v0.2.0/JentleMemes-Launcher-Setup.exe",
      "sha1": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0",
      "size": 6506118,
      "installer": true,
      "notes": "Запустите .exe для обновления. Установщик заменит старую версию."
    },
    "linux-x86_64": {
      "version": "0.2.0",
      "url": "https://jentlememes.ru/api/launcher/releases/v0.2.0/jentlememes-launcher-linux-x86_64",
      "sha1": "b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1",
      "size": 27340800,
      "installer": false,
      "notes": "Замените исполняемый файл и перезапустите."
    }
  }
}
```

| Поле                  | Тип     | Описание |
|-----------------------|---------|----------|
| `latest`              | string  | Последняя версия (semver) |
| `minimum_supported`   | string  | Минимальная поддерживаемая версия. Если текущая версия < этой, принудительное обновление |
| `urgent`              | boolean | Если `true`, обновление обязательное (блокирует работу) |
| `changelog`           | string  | Markdown-формат журнала изменений |
| `platforms`           | object  | Ключи: `windows-x86_64`, `linux-x86_64`, (будущее: `linux-aarch64`, `macos-x86_64`, `macos-aarch64`) |
| `platforms.*.version` | string  | Версия для этой платформы (может отличаться) |
| `platforms.*.url`     | string  | Прямая ссылка на скачивание |
| `platforms.*.sha1`    | string  | SHA-1 хеш файла (40 hex-символов, нижний регистр) |
| `platforms.*.size`    | number  | Размер файла в байтах |
| `platforms.*.installer` | boolean | `true` = .exe установщик, `false` = прямой бинарник |
| `platforms.*.notes`   | string  | Инструкция по обновлению для конкретной ОС |

---

## 4. Алгоритм работы клиента (Rust)

### 4.1. Проверка обновлений

```
┌──────────────────────────────────────────────────────────┐
│ Запуск лаунчера                                          │
├──────────────────────────────────────────────────────────┤
│ 1. Загрузить news.json + version.json параллельно        │
│ 2. Показать новости на HomeTab                           │
│ 3. Сравнить version.json.latest с CURRENT_VERSION        │
│    ├─ latest == current → ничего не делать               │
│    ├─ latest > current, urgent=false → показать баннер   │
│    └─ latest > current, urgent=true  → блокирующий диалог│
│ 4. Если current < minimum_supported → ПРИНУДИТЕЛЬНО      │
│ 5. Пользователь нажал "Обновить":                        │
│    a. Определить platform key                            │
│    b. Скачать файл по url                                │
│    c. Вычислить SHA-1 скачанного файла                   │
│    d. Сравнить с sha1 из version.json                    │
│    e. SHA совпал → применить обновление                  │
│    f. SHA не совпал → показать ошибку, удалить файл      │
└──────────────────────────────────────────────────────────┘
```

### 4.2. Применение обновления по ОС

**Linux:**
1. Скачать новый бинарник в `~/.jentlememes_data/updates/`
2. Проверить SHA-1
3. Заменить текущий бинарник: `rename(new, current_exe)`
4. Выставить `chmod +x`
5. Перезапуск через `std::process::Command::new(current_exe).spawn()`

**Windows (installer=true):**
1. Скачать `.exe` в `%APPDATA%/.jentlememes_data/updates/`
2. Проверить SHA-1
3. Запустить `.exe` установщик: `std::process::Command::new(setup_exe).spawn()`
4. Закрыть лаунчер (`std::process::exit(0)`)

**Windows (installer=false, если в будущем):**
1. Скачать новый `.exe` в `updates/`
2. Создать батник `update.bat` который ждёт завершения процесса, заменяет файл, запускает новый
3. Запустить батник, закрыть лаунчер

---

## 5. Реализация в коде лаунчера

### 5.1. Новый Rust-модуль: `src-tauri/src/core/updater.rs`

```rust
use crate::core::api::HTTP_CLIENT;
use serde::{Deserialize, Serialize};
use sha1::{Sha1, Digest};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

const UPDATE_URL: &str = "https://jentlememes.ru/api/launcher/version.json";
const NEWS_URL: &str = "https://jentlememes.ru/api/launcher/news.json";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION"); // Берёт из Cargo.toml

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub body: String,
    pub image: Option<String>,
    pub date: String,
    pub url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewsResponse {
    pub news: Vec<NewsItem>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformRelease {
    pub version: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    pub installer: bool,
    pub notes: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VersionInfo {
    pub latest: String,
    pub minimum_supported: String,
    pub urgent: bool,
    pub changelog: String,
    pub platforms: std::collections::HashMap<String, PlatformRelease>,
}

// Определить ключ платформы
fn platform_key() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    { "windows-x86_64" }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    { "linux-x86_64" }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    { "linux-aarch64" }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    { "macos-x86_64" }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    { "macos-aarch64" }
}

pub async fn fetch_news() -> Result<Vec<NewsItem>, String> {
    let resp: NewsResponse = HTTP_CLIENT
        .get(NEWS_URL)
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    Ok(resp.news)
}

pub async fn check_update() -> Result<Option<(VersionInfo, PlatformRelease)>, String> {
    let info: VersionInfo = HTTP_CLIENT
        .get(UPDATE_URL)
        .send().await.map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    
    let current = parse_semver(CURRENT_VERSION);
    let latest = parse_semver(&info.latest);
    
    if latest > current {
        if let Some(release) = info.platforms.get(platform_key()) {
            return Ok(Some((info, release.clone())));
        }
    }
    Ok(None)
}

// Скачать файл обновления с прогрессом и вернуть путь
pub async fn download_update(
    release: &PlatformRelease,
    on_progress: impl Fn(u64, u64), // (downloaded, total)
) -> Result<PathBuf, String> {
    let update_dir = crate::config::get_data_dir().join("updates");
    std::fs::create_dir_all(&update_dir).map_err(|e| e.to_string())?;
    
    let filename = release.url.rsplit('/').next().unwrap_or("update");
    let dest = update_dir.join(filename);
    
    let resp = HTTP_CLIENT.get(&release.url)
        .send().await.map_err(|e| e.to_string())?;
    
    let total = resp.content_length().unwrap_or(release.size);
    let mut downloaded: u64 = 0;
    let mut file = tokio::fs::File::create(&dest).await.map_err(|e| e.to_string())?;
    let mut stream = resp.bytes_stream();
    
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().await.map_err(|e| e.to_string())?;
    
    Ok(dest)
}

// Проверить SHA-1
pub fn verify_sha1(path: &PathBuf, expected: &str) -> Result<bool, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha1::new();
    hasher.update(&data);
    let result = format!("{:x}", hasher.finalize());
    Ok(result == expected.to_lowercase())
}

// Простой semver парсер
fn parse_semver(s: &str) -> (u32, u32, u32) {
    let parts: Vec<&str> = s.split('.').collect();
    let major = parts.get(0).and_then(|v| v.parse().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}
```

### 5.2. Tauri-команды: добавить в `commands.rs`

```rust
#[tauri::command]
pub async fn fetch_launcher_news() -> Result<Vec<crate::core::updater::NewsItem>, String> {
    crate::core::updater::fetch_news().await
}

#[tauri::command]
pub async fn check_launcher_update() -> Result<serde_json::Value, String> {
    match crate::core::updater::check_update().await? {
        Some((info, release)) => Ok(serde_json::json!({
            "available": true,
            "latest": info.latest,
            "current": env!("CARGO_PKG_VERSION"),
            "urgent": info.urgent,
            "changelog": info.changelog,
            "release": release,
        })),
        None => Ok(serde_json::json!({ "available": false, "current": env!("CARGO_PKG_VERSION") })),
    }
}

#[tauri::command]
pub async fn download_launcher_update(app: tauri::AppHandle) -> Result<String, String> {
    let (info, release) = crate::core::updater::check_update().await?
        .ok_or("No update available")?;
    
    let path = crate::core::updater::download_update(&release, |dl, total| {
        let _ = app.emit("update-progress", serde_json::json!({
            "downloaded": dl, "total": total
        }));
    }).await?;
    
    if !crate::core::updater::verify_sha1(&path, &release.sha1)? {
        let _ = std::fs::remove_file(&path);
        return Err("SHA-1 verification failed — файл повреждён".into());
    }
    
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn apply_launcher_update(file_path: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(&file_path);
    
    #[cfg(target_os = "linux")]
    {
        let current = std::env::current_exe().map_err(|e| e.to_string())?;
        std::fs::copy(&path, &current).map_err(|e| e.to_string())?;
        // chmod +x
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&current, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        // Перезапуск
        std::process::Command::new(&current).spawn().map_err(|e| e.to_string())?;
        std::process::exit(0);
    }
    
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(&path).spawn().map_err(|e| e.to_string())?;
        std::process::exit(0);
    }
}
```

### 5.3. Регистрация команд: `main.rs`

```rust
// Добавить в invoke_handler:
fetch_launcher_news,
check_launcher_update,
download_launcher_update,
apply_launcher_update,
```

### 5.4. Фронтенд: Показ новостей в `HomeTab.tsx`

```typescript
// В useEffect при монтировании:
useEffect(() => {
  invoke("fetch_launcher_news").then((news: any) => {
    setLauncherNews(news);
  }).catch(console.error);
}, []);

// Рендер:
{launcherNews.map(item => (
  <AnimatedCard key={item.id} className="...">
    {item.image && <img src={item.image} className="w-full h-32 object-cover rounded-t-xl" />}
    <div className="p-4">
      <div className="flex items-center gap-2 mb-1">
        <span className="text-xs" style={{ color: "var(--text-secondary)" }}>
          {new Date(item.date).toLocaleDateString("ru")}
        </span>
        {item.tags?.includes("update") && (
          <span className="text-[10px] px-2 py-0.5 bg-jm-accent/20 text-jm-accent rounded-full font-bold">
            UPD
          </span>
        )}
      </div>
      <h3 className="font-bold text-sm">{item.title}</h3>
      <p className="text-xs mt-1" style={{ color: "var(--text-secondary)" }}>{item.body}</p>
    </div>
  </AnimatedCard>
))}
```

### 5.5. Фронтенд: Баннер обновления в `App.tsx`

```typescript
// В useEffect:
invoke("check_launcher_update").then((result: any) => {
  if (result.available) setUpdateInfo(result);
});

// Рендер баннера:
{updateInfo && (
  <motion.div
    initial={{ y: -60, opacity: 0 }}
    animate={{ y: 0, opacity: 1 }}
    className="mx-3 mt-2 p-3 rounded-xl border border-jm-accent bg-jm-accent/10 flex items-center gap-3"
  >
    <span className="text-sm font-bold flex-1">
      Доступно обновление {updateInfo.latest}!
    </span>
    <button onClick={handleUpdate} className="bg-jm-accent text-black px-4 py-1.5 rounded-lg font-bold text-sm">
      Обновить
    </button>
  </motion.div>
)}
```

---

## 6. Генерация SHA-1 при публикации релиза

### Скрипт для создания `.sha1` файлов: `scripts/make-release.sh`

```bash
#!/bin/bash
# Использование: ./scripts/make-release.sh v0.2.0
set -e

VERSION="$1"
if [ -z "$VERSION" ]; then echo "Usage: $0 <version>"; exit 1; fi

RELEASE_DIR="release/$VERSION"
mkdir -p "$RELEASE_DIR"

# Копируем артефакты
cp src-tauri/target/release/jentlememes-launcher "$RELEASE_DIR/jentlememes-launcher-linux-x86_64"
cp dist/JentleMemes-Launcher-Setup.exe "$RELEASE_DIR/"

# Генерируем SHA-1
for f in "$RELEASE_DIR"/*; do
    [ -f "$f" ] || continue
    [[ "$f" == *.sha1 ]] && continue
    SHA=$(sha1sum "$f" | awk '{print $1}')
    echo "$SHA" > "$f.sha1"
    SIZE=$(stat -c%s "$f")
    echo "  $f: sha1=$SHA size=$SIZE"
done

# Генерируем version.json
cat > "$RELEASE_DIR/version.json" <<ENDJSON
{
  "latest": "${VERSION#v}",
  "minimum_supported": "0.1.0",
  "urgent": false,
  "changelog": "### ${VERSION#v}\n- ...",
  "platforms": {
    "windows-x86_64": {
      "version": "${VERSION#v}",
      "url": "https://jentlememes.ru/api/launcher/releases/$VERSION/JentleMemes-Launcher-Setup.exe",
      "sha1": "$(cat "$RELEASE_DIR/JentleMemes-Launcher-Setup.exe.sha1")",
      "size": $(stat -c%s "$RELEASE_DIR/JentleMemes-Launcher-Setup.exe"),
      "installer": true,
      "notes": "Запустите установщик для обновления."
    },
    "linux-x86_64": {
      "version": "${VERSION#v}",
      "url": "https://jentlememes.ru/api/launcher/releases/$VERSION/jentlememes-launcher-linux-x86_64",
      "sha1": "$(cat "$RELEASE_DIR/jentlememes-launcher-linux-x86_64.sha1")",
      "size": $(stat -c%s "$RELEASE_DIR/jentlememes-launcher-linux-x86_64"),
      "installer": false,
      "notes": "Замените файл и перезапустите."
    }
  }
}
ENDJSON

echo ""
echo "=== Release $VERSION ready ==="
echo "Upload contents of $RELEASE_DIR/ to:"
echo "  https://jentlememes.ru/api/launcher/releases/$VERSION/"
echo "Copy $RELEASE_DIR/version.json to:"
echo "  https://jentlememes.ru/api/launcher/version.json"
```

---

## 7. Что нужно предоставить для реализации

### 7.1. Данные от вас (владельца проекта)

| # | Что нужно | Зачем | Формат |
|---|-----------|-------|--------|
| 1 | **Доступ к хостингу** jentlememes.ru для размещения статических файлов | Разместить `news.json`, `version.json` и бинарники | SSH/FTP или CMS-панель |
| 2 | **Путь к document root** на сервере (например, `/var/www/jentlememes.ru/`) | Настроить nginx location | Путь на сервере |
| 3 | **Конфигурация nginx** (текущая) или тип хостинга (shared, VPS, Vercel, и т.д.) | Корректно настроить маршруты | Файл `nginx.conf` или название хостинга |
| 4 | **Начальный `news.json`** с 1-3 новостями | Наполнить ленту при запуске | JSON по формату из раздела 3.1 |
| 5 | **Текст changelog** для версии 0.1.0 | Заполнить `version.json` | Markdown-текст |
| 6 | **Решение по номеру версии**: текущая версия 0.1.0 → следующая будет 0.2.0? | Правильная нумерация | Строка semver |
| 7 | **Сертификат HTTPS** для `jentlememes.ru` (уже есть?) | Лаунчер делает запросы по HTTPS | Let's Encrypt или другой |

### 7.2. Документации (не требуются, но полезно)

| # | Документ | Зачем |
|---|----------|-------|
| 1 | Документация по текущему бэкенду сайта (если есть API) | Интеграция с существующей системой новостей |
| 2 | Структура БД (если новости хранятся в базе) | Создать endpoint вместо статического JSON |
| 3 | Информация о CI/CD (GitHub Actions, ручной деплой?) | Автоматизировать публикацию релизов |

### 7.3. Куски кода, которые я буду менять

| Файл | Что будет добавлено |
|------|---------------------|
| `src-tauri/src/core/updater.rs` | **Новый файл.** Модуль проверки обновлений, скачивания, SHA-1 верификации (см. раздел 5.1) |
| `src-tauri/src/core/mod.rs` | Добавить `pub mod updater;` |
| `src-tauri/src/commands.rs` | 4 новые команды: `fetch_launcher_news`, `check_launcher_update`, `download_launcher_update`, `apply_launcher_update` (см. раздел 5.2) |
| `src-tauri/src/main.rs` | Регистрация 4 новых команд в `invoke_handler` |
| `src-tauri/Cargo.toml` | Без новых зависимостей — `reqwest`, `sha1`, `tokio`, `serde`, `futures` уже есть |
| `src/tabs/HomeTab.tsx` | Секция "Новости лаунчера" с загрузкой из API (см. раздел 5.4) |
| `src/App.tsx` | Баннер обновления + модальное окно с changelog + прогресс скачивания (см. раздел 5.5) |
| `scripts/make-release.sh` | **Новый файл.** Скрипт генерации релиза с SHA-1 и `version.json` (см. раздел 6) |

---

## 8. Процесс публикации нового релиза (для администратора)

```
1. Обновить версию в:
   - src-tauri/Cargo.toml        → version = "0.2.0"
   - src-tauri/tauri.conf.json   → "version": "0.2.0"
   - package.json                → "version": "0.2.0"

2. Собрать:
   $ npm run tauri build --no-bundle                  # Linux
   $ bash scripts/build-windows.sh                     # Windows

3. Сгенерировать релиз:
   $ bash scripts/make-release.sh v0.2.0

4. Загрузить на сервер:
   $ scp -r release/v0.2.0/ user@jentlememes.ru:/var/www/jentlememes.ru/launcher-api/releases/v0.2.0/
   $ scp release/v0.2.0/version.json user@jentlememes.ru:/var/www/jentlememes.ru/launcher-api/version.json

5. (Опционально) Обновить news.json:
   $ scp news.json user@jentlememes.ru:/var/www/jentlememes.ru/launcher-api/news.json
```

---

## 9. Безопасность

| Мера | Описание |
|------|----------|
| **SHA-1 верификация** | Каждый скачанный файл проверяется перед применением. Если хеш не совпадает — файл удаляется, обновление отменяется |
| **HTTPS** | Все запросы к API идут по HTTPS (защита от MITM) |
| **Minimum supported version** | Позволяет принудительно обновить пользователей с критическими уязвимостями |
| **Права файлов (Linux)** | После замены бинарника выставляется `chmod 755` |
| **Подпись (будущее)** | Можно добавить Ed25519 подпись вместо/помимо SHA-1 для защиты от компрометации сервера |

---

## 10. FAQ / Спорные моменты

**Q: Почему SHA-1, а не SHA-256?**  
A: В проекте уже есть крейт `sha1`. SHA-1 достаточен для проверки целостности (не криптографической подписи). Если хотите SHA-256 — потребуется добавить зависимость `sha2`.

**Q: Нужен ли динамический бэкенд?**  
A: Нет. Все файлы статические. Обновление `news.json` — это просто замена файла на сервере. В будущем можно добавить CMS-интерфейс.

**Q: Как тестировать без продакшен-сервера?**  
A: Запустите локальный HTTP-сервер: `python3 -m http.server 8080` в папке с `news.json` и `version.json`, и временно измените URL в `updater.rs` на `http://localhost:8080/`.

---

## 11. Контрольный чек-лист перед реализацией

- [ ] Подтвердить формат `news.json` (раздел 3.1)
- [ ] Подтвердить формат `version.json` (раздел 3.2)
- [ ] Предоставить доступ к серверу для размещения файлов
- [ ] Предоставить текущую конфигурацию nginx / тип хостинга
- [ ] Определить следующую версию (0.2.0?)
- [ ] Написать 1-3 начальных новости для `news.json`
- [ ] Подтвердить: SHA-1 или SHA-256?
- [ ] Подтвердить: автоматическое обновление или только уведомление?
