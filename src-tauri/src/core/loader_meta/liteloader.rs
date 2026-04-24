//! LiteLoader — вшитый `data/liteloader.json` (без Prism Meta и без сетевого индекса).

use crate::core::loader_meta::version_sort;
use crate::error::{Error, Result};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

const EMBEDDED: &str = include_str!("data/liteloader.json");

#[derive(Debug, Deserialize)]
struct Root {
    #[serde(rename = "mcToLoaders")]
    mc_to_loaders: BTreeMap<String, Vec<String>>,
    patches: BTreeMap<String, Value>,
}

static ROOT: Lazy<Root> =
    Lazy::new(|| serde_json::from_str(EMBEDDED).expect("liteloader.json must parse"));

fn looks_like_release_mc(id: &str) -> bool {
    let t = id.trim();
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '.')
}

pub fn patch_for_loader_version(loader_version: &str) -> Result<Value> {
    let lv = loader_version.trim();
    let p = ROOT
        .patches
        .get(lv)
        .cloned()
        .ok_or_else(|| {
            Error::Custom(format!(
                "LiteLoader: нет вшитого патча для «{lv}». Добавьте запись в loader_meta/data/liteloader.json."
            ))
        })?;
    Ok(p)
}

pub fn minecraft_versions(include_snapshots: bool) -> Result<Vec<String>> {
    let mut set = std::collections::BTreeSet::new();
    for mc in ROOT.mc_to_loaders.keys() {
        if !include_snapshots && !looks_like_release_mc(mc) {
            continue;
        }
        set.insert(mc.clone());
    }
    let mut out: Vec<String> = set.into_iter().collect();
    version_sort::sort_mc_versions_desc(&mut out);
    Ok(out)
}

pub fn loader_versions_for_minecraft(game_version: &str) -> Result<Vec<String>> {
    let gv = game_version.trim();
    if gv.is_empty() {
        return Ok(vec![]);
    }
    Ok(ROOT.mc_to_loaders.get(gv).cloned().unwrap_or_default())
}
