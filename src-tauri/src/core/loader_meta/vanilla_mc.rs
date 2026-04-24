use serde_json::Value;

use crate::error::{Error, Result};

use super::http::fetch_text_cached;
use super::version_sort::sort_mc_versions_desc;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const CACHE_KEY: &str = "vanilla:version_manifest_v2.json";

/// Полный `version_manifest_v2.json` (кэш 2 ч).
pub async fn manifest_v2() -> Result<Value> {
    let body = fetch_text_cached(CACHE_KEY, MANIFEST_URL).await?;
    serde_json::from_str(&body).map_err(|e| Error::Custom(format!("Minecraft manifest JSON: {e}")))
}

/// Список id версий из манифеста (пригодится для отдельного режима без Modrinth).
#[allow(dead_code)]
pub async fn release_game_versions(include_snapshots: bool) -> Result<Vec<String>> {
    let manifest = manifest_v2().await?;
    let arr = manifest
        .get("versions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::Custom("manifest: нет versions[]".into()))?;

    let mut out: Vec<String> = arr
        .iter()
        .filter(|v| include_snapshots || v.get("type").and_then(|t| t.as_str()) == Some("release"))
        .filter_map(|v| v.get("id").and_then(|i| i.as_str()).map(|s| s.to_string()))
        .collect();
    sort_mc_versions_desc(&mut out);
    Ok(out)
}
