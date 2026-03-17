//! CurseForge API (https://api.curseforge.com/v1) and hybrid Modrinth+CurseForge.
//! Requires curseforge_api_key in settings (получить: https://console.curseforge.com).
//! Ответы приводятся к формату Modrinth для совместимости с фронтом.

use serde_json::{json, Value};
use std::collections::HashMap;
use crate::config;
use crate::error::Result;
use crate::core::modrinth;

const BASE: &str = "https://api.curseforge.com/v1";
const MINECRAFT_GAME_ID: u32 = 432;
/// Встроенный ключ для CurseForge API (как в Prism Launcher), чтобы юзерам не настраивать вручную.
const BUILTIN_CURSEFORGE_API_KEY: &str = "$2a$10$CHeqArQS/aZuEPZig9D/eePxauUkFVsOyhLakK6skTnLjm8.GDp/K";

fn api_key() -> Option<String> {
    let key = config::load_settings().ok()?.curseforge_api_key.trim().to_string();
    if key.is_empty() { Some(BUILTIN_CURSEFORGE_API_KEY.to_string()) } else { Some(key) }
}

/// ModLoaderType: 0=None, 1=Forge, 2=Cauldron, 3=LiteLoader, 4=Fabric, 5=Quilt
fn mod_loader_type(loader: &str) -> Option<u32> {
    match loader.to_lowercase().as_str() {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(1),
        _ => None,
    }
}

/// Search CurseForge. Returns Modrinth-like { hits: [...], total_hits: N }.
pub async fn search(
    query: &str,
    project_type: &str,
    game_version: &str,
    loader: &str,
    _categories: Vec<String>,
    page: usize,
) -> Result<Value> {
    let key = match api_key() {
        Some(k) => k,
        None => return Ok(json!({ "hits": [], "total_hits": 0, "error": "curseforge_no_api_key" })),
    };

    if project_type != "mod" && project_type != "modpack" {
        return Ok(json!({ "hits": [], "total_hits": 0 }));
    }

    let page_size = 20u32;
    let index = (page as u32) * page_size;
    // CurseForge API: GET /v1/mods/search с query-параметрами (не POST)
    let class_id: u32 = if project_type == "modpack" { 4471 } else { 6 };
    // Без версии CurseForge часто возвращает пустой список — подставляем популярную
    let game_ver = if game_version.is_empty() { "1.21.1" } else { game_version };

    let client = reqwest::Client::builder()
        .user_agent("JentleMemesLauncher/1.0")
        .build()?;
    let mut req = client
        .get(format!("{}/mods/search", BASE))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .query(&[("gameId", MINECRAFT_GAME_ID), ("classId", class_id), ("index", index), ("pageSize", page_size)]);
    if !query.is_empty() {
        req = req.query(&[("searchFilter", query)]);
    }
    req = req.query(&[("gameVersion", game_ver)]);
    if let Some(ml) = mod_loader_type(loader) {
        req = req.query(&[("modLoaderType", ml)]);
    }
    let res = req.send().await?;

    let status = res.status();
    if status.as_u16() == 403 {
        return Ok(json!({ "hits": [], "total_hits": 0, "error": "curseforge_forbidden" }));
    }
    if !status.is_success() {
        return Ok(json!({ "hits": [], "total_hits": 0 }));
    }

    let data: Value = res.json().await?;
    let empty: Vec<Value> = vec![];
    let arr = data.get("data").and_then(|d| d.as_array()).unwrap_or(&empty);
    let total = data.get("pagination").and_then(|p| p.get("totalCount")).and_then(|t| t.as_u64()).unwrap_or(0);

    let hits: Vec<Value> = arr
        .iter()
        .map(|m| {
            let id = m.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let summary = m.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let author = m
                .get("authors")
                .and_then(|a| a.as_array())
                .and_then(|a| a.first())
                .and_then(|a| a.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let icon = m
                .get("logo")
                .and_then(|l| l.get("url"))
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();
            json!({
                "project_id": id.to_string(),
                "title": name,
                "description": summary,
                "author": author,
                "icon_url": icon,
                "project_type": project_type,
            })
        })
        .collect();

    Ok(json!({ "hits": hits, "total_hits": total }))
}

/// Get CurseForge mod by id. Returns Modrinth-like: title, body, icon_url, project_type.
pub async fn get_project(id: &str) -> Result<Value> {
    let key = match api_key() {
        Some(k) => k,
        None => return Ok(Value::Null),
    };

    let id: u64 = match id.parse() {
        Ok(i) => i,
        Err(_) => return Ok(Value::Null),
    };

    let client = reqwest::Client::builder()
        .user_agent("JentleMemesLauncher/1.0")
        .build()?;
    let res = client
        .get(format!("{}/mods/{}", BASE, id))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .send()
        .await?;

    if !res.status().is_success() {
        return Ok(Value::Null);
    }

    let data: Value = res.json().await?;
    let m = match data.get("data") {
        Some(d) => d,
        None => return Ok(Value::Null),
    };

    let title = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let body = m.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let icon_url = m
        .get("logo")
        .and_then(|l| l.get("url"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    Ok(json!({
        "title": title,
        "body": body,
        "icon_url": icon_url,
        "project_type": "mod",
    }))
}

/// Get CurseForge mod files. Returns array like Modrinth versions: id, name, game_versions, loaders, files: [{ url, filename, primary }].
pub async fn get_versions(id: &str) -> Result<Value> {
    let key = match api_key() {
        Some(k) => k,
        None => return Ok(json!([])),
    };

    let id: u64 = match id.parse() {
        Ok(i) => i,
        Err(_) => return Ok(json!([])),
    };

    let client = reqwest::Client::builder()
        .user_agent("JentleMemesLauncher/1.0")
        .build()?;
    let res = client
        .get(format!("{}/mods/{}/files", BASE, id))
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .send()
        .await?;

    if !res.status().is_success() {
        return Ok(json!([]));
    }

    let data: Value = res.json().await?;
    let empty: Vec<Value> = vec![];
    let arr = data.get("data").and_then(|d| d.as_array()).unwrap_or(&empty);

    let versions: Vec<Value> = arr
        .iter()
        .map(|f| {
            let fid = f.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let display_name = f.get("displayName").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let filename = f.get("fileName").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let url = f.get("downloadUrl").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let game_versions: Vec<String> = f
                .get("gameVersions")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let loaders: Vec<String> = f
                .get("gameVersions")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| {
                            let s = v.as_str()?;
                            if s.eq_ignore_ascii_case("fabric") || s.eq_ignore_ascii_case("forge") || s.eq_ignore_ascii_case("quilt") || s.eq_ignore_ascii_case("neoforge") {
                                Some(s.to_string())
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "id": fid.to_string(),
                "name": display_name,
                "game_versions": game_versions,
                "loaders": if loaders.is_empty() { vec!["forge".to_string()] } else { loaders },
                "files": [{
                    "url": url,
                    "filename": filename,
                    "primary": true,
                }],
            })
        })
        .collect();

    Ok(serde_json::to_value(versions).unwrap_or(json!([])))
}

/// Hybrid search: Modrinth + CurseForge in parallel, merged by title (no duplicates).
pub async fn search_hybrid(
    query: &str,
    project_type: &str,
    game_version: &str,
    loader: &str,
    categories: Vec<String>,
    page: usize,
) -> Result<Value> {
    let (mr, cf) = tokio::join!(
        modrinth::search(query, project_type, game_version, loader, categories.clone(), page),
        search(query, project_type, game_version, loader, categories, page),
    );
    let mr_json = mr.unwrap_or_else(|_| json!({ "hits": [], "total_hits": 0 }));
    let cf_json = cf.unwrap_or_else(|_| json!({ "hits": [], "total_hits": 0 }));
    let mr_hits: Vec<Value> = mr_json.get("hits").and_then(|h| h.as_array()).cloned().unwrap_or_default();
    let cf_hits: Vec<Value> = cf_json.get("hits").and_then(|h| h.as_array()).cloned().unwrap_or_default();
    let mut by_key: HashMap<String, Value> = HashMap::new();
    for mut h in mr_hits {
        let key = h.get("title").and_then(|t| t.as_str()).unwrap_or("").to_lowercase();
        if !key.is_empty() {
            h["source"] = json!("modrinth");
            h["modrinth_id"] = h.get("project_id").cloned().unwrap_or(Value::Null);
            by_key.insert(key, h);
        }
    }
    for mut h in cf_hits {
        let key = h.get("title").and_then(|t| t.as_str()).unwrap_or("").to_lowercase();
        if !key.is_empty() {
            h["curseforge_id"] = h.get("project_id").cloned().unwrap_or(Value::Null);
            if let Some(entry) = by_key.get_mut(&key) {
                entry["curseforge_id"] = h.get("project_id").cloned().unwrap_or(Value::Null);
            } else {
                h["source"] = json!("curseforge");
                by_key.insert(key, h);
            }
        }
    }
    let merged: Vec<Value> = by_key.into_values().collect();
    let total = merged.len();
    Ok(json!({ "hits": merged, "total_hits": total }))
}

/// Hybrid versions: fetch from both sources; merge and tag with _source.
pub async fn get_hybrid_versions(modrinth_id: Option<String>, curseforge_id: Option<String>) -> Result<Value> {
    let (mr_versions, cf_versions) = if let Some(id) = modrinth_id {
        let cf_id = curseforge_id.clone();
        let (mr, cf) = tokio::join!(
            modrinth::get_versions(&id),
            async move {
                if let Some(cid) = cf_id {
                    get_versions(&cid).await
                } else {
                    Ok(json!([]))
                }
            },
        );
        (
            mr.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default(),
            cf.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default(),
        )
    } else {
        (vec![], vec![])
    };
    let mut out: Vec<Value> = mr_versions
        .into_iter()
        .map(|mut v| {
            v["_source"] = json!("modrinth");
            v
        })
        .collect();
    for v in cf_versions {
        let mut o = v;
        o["_source"] = json!("curseforge");
        out.push(o);
    }
    Ok(serde_json::to_value(out).unwrap_or(json!([])))
}
