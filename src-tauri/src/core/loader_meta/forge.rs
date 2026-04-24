use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::error::{Error, Result};

use super::http::fetch_text_cached;
use super::version_sort::{
    is_release_style_mc_version, mc_version_precedes, sort_mc_versions_desc,
};
use super::xml_versions::extract_maven_versions_chunked;

const PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const MAVEN_META_URL: &str =
    "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml";

const CACHE_PROMO: &str = "forge:promotions_slim.json";
const CACHE_MAVEN: &str = "forge:maven-metadata.xml";

fn forge_installer_hosted_for_release_mc(mc: &str) -> bool {
    !is_release_style_mc_version(mc) || !mc_version_precedes(mc, "1.5.2")
}

/// `1.20.1-47.2.0` → (`1.20.1`, `47.2.0`). `1.21-51.0.33` → (`1.21`, `51.0.33`).
pub fn split_forge_maven_version(full: &str) -> Option<(String, String)> {
    let mut it = full.rsplitn(2, '-');
    let forge_part = it.next()?.to_string();
    let mc_part = it.next()?.to_string();
    if mc_part.is_empty() || forge_part.is_empty() {
        return None;
    }
    Some((mc_part, forge_part))
}

async fn promotions_json() -> Result<Value> {
    let body = fetch_text_cached(CACHE_PROMO, PROMOTIONS_URL).await?;
    serde_json::from_str(&body).map_err(|e| Error::Custom(format!("Forge promotions JSON: {e}")))
}

pub(super) async fn maven_full_versions() -> Result<Vec<String>> {
    let xml = fetch_text_cached(CACHE_MAVEN, MAVEN_META_URL).await?;
    Ok(extract_maven_versions_chunked(&xml))
}

/// Список версий Minecraft, для которых есть Forge (промо + Maven).
pub async fn minecraft_versions(include_snapshots: bool) -> Result<Vec<String>> {
    let (promo, maven_list) = tokio::join!(promotions_json(), maven_full_versions());
    let promo = promo?;
    let maven_list = maven_list?;

    let mut set: HashSet<String> = HashSet::new();

    if let Some(promos) = promo.get("promos").and_then(|p| p.as_object()) {
        for key in promos.keys() {
            let mut it = key.rsplitn(2, '-');
            let _suffix = it.next();
            if let Some(mc) = it.next() {
                if !mc.is_empty()
                    && forge_installer_hosted_for_release_mc(mc)
                    && (include_snapshots || is_release_style_mc_version(mc))
                {
                    set.insert(mc.to_string());
                }
            }
        }
    }

    for full in maven_list {
        if let Some((mc, _)) = split_forge_maven_version(&full) {
            if forge_installer_hosted_for_release_mc(&mc)
                && (include_snapshots || is_release_style_mc_version(&mc))
            {
                set.insert(mc);
            }
        }
    }

    let mut out: Vec<String> = set.into_iter().collect();
    sort_mc_versions_desc(&mut out);
    Ok(out)
}

/// Версии Forge для конкретной версии Minecraft.
pub async fn loader_versions_for_minecraft(game_version: &str) -> Result<Vec<String>> {
    let gv = game_version.trim();
    if gv.is_empty() {
        return Ok(vec![]);
    }
    if is_release_style_mc_version(gv) && mc_version_precedes(gv, "1.5.2") {
        return Ok(vec![]);
    }

    let (promo, maven_list) = tokio::join!(promotions_json(), maven_full_versions());
    let promo = promo?;
    let maven_list = maven_list?;

    let mut map: HashMap<String, ()> = HashMap::new();

    if let Some(promos) = promo.get("promos").and_then(|p| p.as_object()) {
        for (key, val) in promos {
            let mut it = key.rsplitn(2, '-');
            let _suffix = it.next();
            if let Some(mc) = it.next() {
                if mc == gv {
                    let s = val
                        .as_str()
                        .map(|s| s.to_string())
                        .or_else(|| val.as_u64().map(|n| n.to_string()))
                        .or_else(|| val.as_i64().map(|n| n.to_string()));
                    if let Some(t) = s {
                        let t = t.trim().to_string();
                        if !t.is_empty() {
                            map.insert(t, ());
                        }
                    }
                }
            }
        }
    }

    for full in maven_list {
        if let Some((mc, forge_v)) = split_forge_maven_version(&full) {
            if mc == gv {
                map.insert(forge_v, ());
            }
        }
    }

    let mut out: Vec<String> = map.into_keys().collect();
    sort_mc_versions_desc(&mut out);
    Ok(out)
}
