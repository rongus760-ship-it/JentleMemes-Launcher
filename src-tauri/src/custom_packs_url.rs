//! URL для списка кастомных сборок.
//! Измените CUSTOM_PACKS_URL на адрес вашего JSON-файла со сборками.
//! При каждом перезапуске лаунчера JSON будет загружаться и обновляться.

const CONFIG_JSON: &str = include_str!("../custom_packs.json");

/// URL списка кастомных сборок (резолвится из custom_packs.json)
pub fn get_custom_packs_url() -> String {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(CONFIG_JSON) {
        if let Some(url) = json.get("url").and_then(|v| v.as_str()) {
            return url.trim().to_string();
        }
    }
    String::new()
}
