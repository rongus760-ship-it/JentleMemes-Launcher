# Формат кастомных сборок

Адрес URL со списком сборок хранится в файле `src-tauri/custom_packs.json`. Его нельзя изменить через интерфейс лаунчера.

## Конфигурация лаунчера

Файл `src-tauri/custom_packs.json`:

```json
{
  "url": "https://example.com/packs.json"
}
```

Перед сборкой замените URL на адрес вашего JSON-файла со списком сборок.

---

## Формат JSON-файла со сборками

URL должен возвращать **JSON-файл** в одном из форматов:

### Вариант 1: массив объектов (рекомендуется)

```json
[
  {
    "id": "pack-1",
    "title": "Название сборки",
    "description": "Краткое описание.",
    "author": "Ваше имя",
    "icon_url": "https://example.com/icon.png",
    "url": "https://cdn.modrinth.com/data/xxx/versions/yyy/pack.mrpack"
  },
  {
    "title": "Вторая сборка",
    "url": "https://example.com/pack2.mrpack"
  }
]
```

### Вариант 2: объект с полем `packs` или `items`

```json
{
  "packs": [
    {
      "title": "Сборка",
      "url": "https://example.com/pack.mrpack"
    }
  ]
}
```

---

## Обязательные поля

| Поле | Тип | Описание |
|------|-----|----------|
| `url` | string | Прямая ссылка на скачивание .mrpack. **Обязательно.** |

Допустимые синонимы: `mrpack_url`, `download_url`.

---

## Опциональные поля

| Поле | Тип | Описание |
|------|-----|----------|
| `title` | string | Название (или `name`) |
| `description` | string | Описание |
| `author` | string | Автор |
| `icon_url` | string | URL иконки (PNG, JPEG) |
| `id` | string | Уникальный идентификатор |
| `sha1` | string | **SHA1-хеш файла .mrpack** — для проверки обновлений. Если указан, лаунчер сравнивает его с установленной версией и предлагает обновление при отличии. Синонимы: `mrpack_sha1`. |

---

## Пример минимального файла

```json
[
  {
    "title": "Моя сборка",
    "url": "https://cdn.modrinth.com/data/ABC123/versions/1.0.0/JentlePack.mrpack"
  }
]
```

---

## Как получить ссылку на .mrpack

1. **Modrinth**: Страница проекта → Versions → нужная версия → кнопка Download. Скопируйте ссылку.
2. **Собственный хостинг**: Положите .mrpack на сервер и укажите прямую ссылку.
3. **GitHub Releases**: Создайте релиз, приложите .mrpack и используйте ссылку на файл.

---

## Пример полного файла

```json
[
  {
    "id": "jentle-main",
    "title": "JentleMemes Main",
    "description": "Основная сборка для сервера JentleMemes.",
    "author": "JentleMemes",
    "icon_url": "https://example.com/icons/jentle.png",
    "url": "https://cdn.modrinth.com/data/xyz/versions/1.0/JentleMemes.mrpack"
  }
]
```
