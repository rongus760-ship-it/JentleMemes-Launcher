use std::collections::HashSet;

use crate::error::Result;

use super::http::fetch_text_cached;
use super::version_sort::{is_release_style_mc_version, sort_mc_versions_desc};
use super::xml_versions::extract_maven_versions_chunked;

const MAVEN_META_URL: &str =
    "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";
const CACHE_MAVEN: &str = "neoforge:maven-metadata.xml";

/// Старый формат: `21.4.14` → `1.21.4`. Новый (Minecraft 26+): первые три числовые сегмента — версия игры
/// (`26.1.0.10-beta` → `26.1.0`; `26.1.0.0-alpha.2+snapshot-1` → после отсечения суффиксов — `26.1.0.0` → `26.1.0`).
pub fn neoforge_artifact_to_minecraft(ver: &str) -> Option<String> {
    let base = ver.split('+').next()?.split('_').next()?.trim();
    let before_dash = base.split('-').next()?.trim();
    let segments: Vec<&str> = before_dash
        .split('.')
        .map_while(|p| {
            if p.chars().all(|c| c.is_ascii_digit()) {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    if segments.is_empty() {
        return None;
    }
    let major: u32 = segments[0].parse().ok()?;
    if major >= 26 {
        return match segments.len() {
            1 => Some(segments[0].to_string()),
            2 => Some(format!("{}.{}.0", segments[0], segments[1])),
            _ => Some(format!("{}.{}.{}", segments[0], segments[1], segments[2])),
        };
    }
    let b = segments.get(1)?;
    if !b.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("1.{}.{}", segments[0], b))
}

/// Сопоставление выбранной в UI версии MC и артефакта NeoForge (`26.1` ↔ `26.1.0.x`, `1.21.4` ↔ `21.4.x`).
pub fn neoforge_artifact_matches_game(game_version: &str, artifact: &str) -> bool {
    let Some(derived) = neoforge_artifact_to_minecraft(artifact) else {
        return false;
    };
    let gv = game_version.trim();
    if derived == gv {
        return true;
    }
    if derived.starts_with(&format!("{gv}.")) {
        return true;
    }
    if gv.starts_with(&format!("{derived}.")) {
        return true;
    }
    false
}

async fn artifact_versions() -> Result<Vec<String>> {
    let xml = fetch_text_cached(CACHE_MAVEN, MAVEN_META_URL).await?;
    Ok(extract_maven_versions_chunked(&xml))
}

pub async fn minecraft_versions(include_snapshots: bool) -> Result<Vec<String>> {
    let list = artifact_versions().await?;
    let mut set: HashSet<String> = HashSet::new();
    for v in list {
        if let Some(mc) = neoforge_artifact_to_minecraft(&v) {
            if include_snapshots || is_release_style_mc_version(&mc) {
                set.insert(mc);
            }
        }
    }
    let mut out: Vec<String> = set.into_iter().collect();
    sort_mc_versions_desc(&mut out);
    Ok(out)
}

pub async fn loader_versions_for_minecraft(game_version: &str) -> Result<Vec<String>> {
    let gv = game_version.trim();
    if gv.is_empty() {
        return Ok(vec![]);
    }
    let list = artifact_versions().await?;
    let mut out: Vec<String> = list
        .into_iter()
        .filter(|art| neoforge_artifact_matches_game(gv, art))
        .collect();
    sort_mc_versions_desc(&mut out);
    Ok(out)
}
