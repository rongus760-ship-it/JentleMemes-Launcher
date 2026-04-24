//! Сводка списков версий для UI: Fabric, Quilt, Forge, NeoForge, vanilla (Mojang manifest), плюс лоадеры под MC.

use serde_json::Value;

use crate::error::{Error, Result};

use super::fabric;
use super::forge;
use super::http::fetch_text_cached;
use super::liteloader;
use super::modloader_alpha;
use super::neoforge;
use super::quilt;

fn alpha_loaders_enabled() -> bool {
    crate::config::load_settings()
        .map(|s| s.enable_alpha_loaders)
        .unwrap_or(false)
}

pub async fn loader_game_versions(
    loader: &str,
    include_snapshots: Option<bool>,
    include_alpha_beta: Option<bool>,
) -> Result<Vec<String>> {
    let sn = include_snapshots.unwrap_or(false);
    let ab = include_alpha_beta.unwrap_or(false);
    let wide_lists = sn || ab;
    match loader {
        "fabric" => fabric::game_versions(wide_lists).await,
        "quilt" => quilt::game_versions(wide_lists).await,
        "forge" => forge::minecraft_versions(wide_lists).await,
        "neoforge" => neoforge::minecraft_versions(wide_lists).await,
        "liteloader" => {
            if !alpha_loaders_enabled() {
                return Err(Error::Custom(
                    "LiteLoader: включите «Альфа: LiteLoader и ModLoader» в расширенных настройках."
                        .into(),
                ));
            }
            liteloader::minecraft_versions(wide_lists)
        }
        "modloader" => {
            if !alpha_loaders_enabled() {
                return Err(Error::Custom(
                    "ModLoader: включите «Альфа: LiteLoader и ModLoader» в расширенных настройках."
                        .into(),
                ));
            }
            modloader_alpha::minecraft_versions(wide_lists)
        }
        _ => modrinth_game_versions(sn, ab).await,
    }
}

/// Версии игры с Modrinth (теги): `release` всегда; `snapshot` / `alpha` / `beta` — по флагам.
pub async fn modrinth_game_versions(
    include_snapshots: bool,
    include_alpha_beta: bool,
) -> Result<Vec<String>> {
    use crate::core::api::{http_client, json_from_response};
    let res = http_client()
        .get("https://api.modrinth.com/v2/tag/game_version")
        .send()
        .await?;
    let res = json_from_response(res, "Modrinth game_version").await?;
    let arr = res
        .as_array()
        .ok_or_else(|| Error::Custom("Modrinth: ожидался JSON-массив".into()))?;
    let mut out: Vec<String> = arr
        .iter()
        .filter(|v| {
            let ty = v["version_type"].as_str().unwrap_or("");
            ty == "release"
                || (include_snapshots && ty == "snapshot")
                || (include_alpha_beta && (ty == "alpha" || ty == "beta"))
        })
        .filter_map(|v| v["version"].as_str().map(|s| s.to_string()))
        .collect();
    super::version_sort::sort_mc_versions_desc(&mut out);
    Ok(out)
}

pub async fn loader_versions_for_game(loader: &str, game_version: &str) -> Result<Vec<String>> {
    let game_version = game_version.trim();
    if game_version.is_empty() {
        return Ok(vec![]);
    }
    match loader {
        "fabric" => fabric_loaders(game_version, "fabric").await,
        "quilt" => fabric_loaders(game_version, "quilt").await,
        "forge" => forge::loader_versions_for_minecraft(game_version).await,
        "neoforge" => neoforge::loader_versions_for_minecraft(game_version).await,
        "liteloader" => {
            if !alpha_loaders_enabled() {
                return Ok(vec![]);
            }
            liteloader::loader_versions_for_minecraft(game_version)
        }
        "modloader" => {
            if !alpha_loaders_enabled() {
                return Ok(vec![]);
            }
            modloader_alpha::loader_versions_for_minecraft(game_version)
        }
        _ => Ok(vec![]),
    }
}

async fn fabric_loaders(game_version: &str, kind: &str) -> Result<Vec<String>> {
    let enc = urlencoding::encode(game_version);
    let (cache_key, url) = if kind == "fabric" {
        (
            format!("fabric:loader:{enc}"),
            format!("https://meta.fabricmc.net/v2/versions/loader/{enc}"),
        )
    } else {
        (
            format!("quilt:loader:{enc}"),
            format!("https://meta.quiltmc.org/v3/versions/loader/{enc}"),
        )
    };
    let body = fetch_text_cached(&cache_key, &url).await?;
    let res: Value = serde_json::from_str(&body)
        .map_err(|e| Error::Custom(format!("{kind} loader list JSON ({game_version}): {e}")))?;
    let out: Vec<String> = match res.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v["loader"]["version"]
                    .as_str()
                    .map(|s| s.trim().to_string())
            })
            .collect(),
        None => vec![],
    };
    Ok(out)
}
