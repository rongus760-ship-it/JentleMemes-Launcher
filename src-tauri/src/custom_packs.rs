//! Загрузка и кэширование кастомных сборок с удалённого URL.
//! При каждом перезапуске лаунчера JSON обновляется.

use std::fs;
use std::sync::Mutex;
use serde_json::Value;
use crate::config::get_data_dir;
use crate::custom_packs_url::get_custom_packs_url;
use crate::error::Result;

static CACHE: Mutex<Option<Value>> = Mutex::new(None);

/// Загружает JSON со сборками с удалённого адреса и кэширует в data_dir.
/// Вызывается при старте лаунчера.
pub async fn fetch_and_cache_packs() -> Result<Value> {
    let url = get_custom_packs_url();
    if url.is_empty() {
        return Ok(serde_json::json!([]));
    }
    let client = reqwest::Client::builder()
        .user_agent("JentleMemesLauncher/1.0")
        .build()?;
    let res = client.get(&url).send().await?;
    let json: Value = res.json().await?;
    let arr = if let Some(a) = json.as_array() {
        Value::Array(a.clone())
    } else if let Some(packs) = json.get("packs").or(json.get("items")) {
        packs.clone()
    } else {
        json.clone()
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
