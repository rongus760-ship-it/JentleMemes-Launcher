//! Источник списка кастомных сборок: удалённый URL или встроенный JSON в `custom_packs.json`.

use serde_json::Value;

const CONFIG_JSON: &str = include_str!("../custom_packs.json");

#[derive(Debug)]
pub enum CustomPacksSource {
    /// Нет ни URL, ни встроенного списка.
    Empty,
    /// Скачать JSON по адресу (корень — массив или `{ "packs" | "items" }`).
    Remote(String),
    /// Список зашит в бинарник: корень — массив или объект с `packs` / `items` (без `url`).
    Inline(Value),
}

/// Разбор `custom_packs.json`: массив сборок, `{ "url" }`, либо `{ "packs": [...] }` без URL.
pub fn get_custom_packs_source() -> CustomPacksSource {
    let Ok(json) = serde_json::from_str::<Value>(CONFIG_JSON) else {
        return CustomPacksSource::Empty;
    };
    if let Some(a) = json.as_array() {
        return CustomPacksSource::Inline(Value::Array(a.clone()));
    }
    let url = json
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);
    if let Some(u) = url {
        return CustomPacksSource::Remote(u);
    }
    if let Some(p) = json.get("packs").or(json.get("items")) {
        return CustomPacksSource::Inline(p.clone());
    }
    CustomPacksSource::Empty
}
