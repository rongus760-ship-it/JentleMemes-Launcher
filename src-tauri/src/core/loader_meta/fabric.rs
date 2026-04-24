use serde_json::Value;

use crate::error::{Error, Result};

use super::http::fetch_text_cached;
use super::version_sort::sort_mc_versions_desc;

const GAME_URL: &str = "https://meta.fabricmc.net/v2/versions/game";

pub async fn game_versions(include_snapshots: bool) -> Result<Vec<String>> {
    let body = fetch_text_cached("fabric:game", GAME_URL).await?;
    let res: Value = serde_json::from_str(&body)
        .map_err(|e| Error::Custom(format!("Fabric game meta JSON: {e}")))?;
    let arr = res
        .as_array()
        .ok_or_else(|| Error::Custom("Fabric meta: ожидался массив".into()))?;
    let mut out: Vec<String> = arr
        .iter()
        .filter(|v| include_snapshots || v["stable"].as_bool() == Some(true))
        .filter_map(|v| v["version"].as_str().map(|s| s.to_string()))
        .collect();
    sort_mc_versions_desc(&mut out);
    Ok(out)
}
