//! Загрузка и кэширование кастомных сборок с удалённого URL.
//! При каждом перезапуске лаунчера JSON обновляется.

use crate::config::get_data_dir;
use crate::custom_packs_url::{get_custom_packs_source, CustomPacksSource};
use crate::error::Result;
use serde_json::Value;
use std::fs;
use std::sync::Mutex;

static CACHE: Mutex<Option<Value>> = Mutex::new(None);

fn normalize_packs_json(json: Value) -> Value {
    if let Some(a) = json.as_array() {
        return Value::Array(a.clone());
    }
    if let Some(p) = json
        .get("packs")
        .or(json.get("items"))
        .and_then(|v| v.as_array())
    {
        return Value::Array(p.clone());
    }
    Value::Array(vec![])
}

/// Загружает список сборок (удалённо или из встроенного `custom_packs.json`) и кэширует в data_dir.
/// Вызывается при старте лаунчера.
pub async fn fetch_and_cache_packs() -> Result<Value> {
    let arr = match get_custom_packs_source() {
        CustomPacksSource::Empty => Value::Array(vec![]),
        CustomPacksSource::Inline(v) => normalize_packs_json(v),
        CustomPacksSource::Remote(url) => {
            let client = crate::core::api::http_client();
            let res = client.get(&url).send().await?;
            let json: Value = res.json().await?;
            normalize_packs_json(json)
        }
    };
    let cache_path = get_data_dir().join("custom_packs_cache.json");
    fs::create_dir_all(get_data_dir())?;
    fs::write(&cache_path, serde_json::to_string_pretty(&arr)?)?;
    {
        let mut guard = CACHE.lock().unwrap();
        *guard = Some(arr.clone());
    }
    Ok(arr)
}

/// Возвращает кэшированный список сборок или загружает заново.
pub fn get_cached_packs() -> Value {
    if let Ok(guard) = CACHE.lock() {
        if let Some(ref v) = *guard {
            return v.clone();
        }
    }
    let cache_path = get_data_dir().join("custom_packs_cache.json");
    if cache_path.exists() {
        if let Ok(s) = fs::read_to_string(&cache_path) {
            if let Ok(v) = serde_json::from_str::<Value>(&s) {
                return v;
            }
        }
    }
    serde_json::json!([])
}
