//! CurseForge API (https://api.curseforge.com/v1) and hybrid Modrinth+CurseForge.
//! Requires curseforge_api_key in settings (получить: https://console.curseforge.com).
//! Ответы приводятся к формату Modrinth для совместимости с фронтом.

use crate::config;
use crate::core::modrinth;
use crate::error::{Error, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

const BASE: &str = "https://api.curseforge.com/v1";
const MINECRAFT_GAME_ID: u32 = 432;
/// Встроенный ключ для CurseForge API (как в Prism Launcher), чтобы юзерам не настраивать вручную.
const BUILTIN_CURSEFORGE_API_KEY: &str =
    "$2a$10$DnVHLcP.vV9mBN8z5n5LP.IYAaRMf1eSwG0VdWdAJpOljveXl.lgC";

/// (основной ключ, запасной встроенный если в настройках указан свой).
/// Запасной нужен: неверный/просроченный ключ в `settings.json` на одном ПК даёт 403, на другом поле пустое — работает встроенный.
fn curseforge_api_keys() -> Option<(String, Option<String>)> {
    let settings = config::load_settings().ok()?;
    let user = settings.curseforge_api_key.trim().to_string();
    if user.is_empty() {
        Some((BUILTIN_CURSEFORGE_API_KEY.to_string(), None))
    } else {
        Some((user, Some(BUILTIN_CURSEFORGE_API_KEY.to_string())))
    }
}

fn cf_http_client() -> reqwest::Client {
    crate::core::api::http_client()
}

fn cf_needs_key_retry(status: u16) -> bool {
    matches!(status, 401 | 403)
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

fn mods_search_request(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    project_type: &str,
    game_version: &str,
    loader: &str,
    page: usize,
    sort: &str,
    sort_desc: bool,
) -> reqwest::RequestBuilder {
    let page_size = 20u32;
    let index = (page as u32) * page_size;
    let class_id: u32 = match project_type {
        "modpack" => 4471,
        "resourcepack" => 12,
        "shader" => 6552,
        _ => 6,
    };
    let mut req = client
        .get(format!("{}/mods/search", BASE))
        .header("x-api-key", api_key)
        .header("Accept", "application/json")
        .query(&[
            ("gameId", MINECRAFT_GAME_ID),
            ("classId", class_id),
            ("index", index),
            ("pageSize", page_size),
        ]);
    if !query.is_empty() {
        req = req.query(&[("searchFilter", query)]);
    }
    if !game_version.is_empty() {
        req = req.query(&[("gameVersion", game_version)]);
        if project_type == "mod" || project_type == "modpack" {
            if let Some(ml) = mod_loader_type(loader) {
                req = req.query(&[("modLoaderType", ml)]);
            }
        }
    }
    let sort_field: u32 = match sort {
        "relevance" | "featured" => 1,
        "popularity" => 2,
        "updated" | "last_updated" => 3,
        "name" => 4,
        "author" => 5,
        "downloads" => 6,
        "rating" => 12,
        _ => 1,
    };
    let order = if sort_desc { "desc" } else { "asc" };
    req.query(&[
        ("sortField", sort_field.to_string()),
        ("sortOrder", order.to_string()),
    ])
}

/// Search CurseForge. Returns Modrinth-like { hits: [...], total_hits: N }.
pub async fn search(
    query: &str,
    project_type: &str,
    game_version: &str,
    loader: &str,
    _categories: Vec<String>,
    page: usize,
    sort: &str,
    sort_desc: bool,
) -> Result<Value> {
    let (primary, fallback) = match curseforge_api_keys() {
        Some(k) => k,
        None => {
            return Ok(json!({ "hits": [], "total_hits": 0, "error": "curseforge_no_api_key" }))
        }
    };

    let client = cf_http_client();
    let mut res = mods_search_request(
        &client,
        &primary,
        query,
        project_type,
        game_version,
        loader,
        page,
        sort,
        sort_desc,
    )
    .send()
    .await?;
    if cf_needs_key_retry(res.status().as_u16()) {
        if let Some(ref fb) = fallback {
            if fb != &primary {
                res = mods_search_request(
                    &client,
                    fb,
                    query,
                    project_type,
                    game_version,
                    loader,
                    page,
                    sort,
                    sort_desc,
                )
                .send()
                .await?;
            }
        }
    }

    let status = res.status();
    if status.as_u16() == 403 {
        return Ok(json!({ "hits": [], "total_hits": 0, "error": "curseforge_forbidden" }));
    }
    if !status.is_success() {
        return Ok(json!({ "hits": [], "total_hits": 0 }));
    }

    let data: Value = res.json().await?;
    let empty: Vec<Value> = vec![];
    let arr = data
        .get("data")
        .and_then(|d| d.as_array())
        .unwrap_or(&empty);
    let total = data
        .get("pagination")
        .and_then(|p| p.get("totalCount"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    let hits: Vec<Value> = arr
        .iter()
        .map(|m| {
            let id = m.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let name = m
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let summary = m
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
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
    let (primary, fallback) = match curseforge_api_keys() {
        Some(k) => k,
        None => return Ok(Value::Null),
    };

    let id: u64 = match id.parse() {
        Ok(i) => i,
        Err(_) => return Ok(Value::Null),
    };

    let client = cf_http_client();
    let mod_url = format!("{}/mods/{}", BASE, id);
    let mut key = primary.clone();
    let mut res = client
        .get(&mod_url)
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .send()
        .await?;
    if cf_needs_key_retry(res.status().as_u16()) {
        if let Some(ref fb) = fallback {
            if fb != &primary {
                key = fb.clone();
                res = client
                    .get(&mod_url)
                    .header("x-api-key", &key)
                    .header("Accept", "application/json")
                    .send()
                    .await?;
            }
        }
    }

    if !res.status().is_success() {
        return Ok(Value::Null);
    }

    let data: Value = res.json().await?;
    let m = match data.get("data") {
        Some(d) => d,
        None => return Ok(Value::Null),
    };

    // CurseForge API:
    // - `name` - заголовок
    // - `summary` - краткое описание
    // - расширенное HTML-описание отдаётся отдельным эндпоинтом `/mods/{modId}/description`
    let title = m
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let summary = m
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut body = summary.clone();

    // Скриншоты: `screenshots[]` содержит thumbnailUrl/url.
    let gallery: Vec<String> = m
        .get("screenshots")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| {
                    let url = s
                        .get("url")
                        .and_then(|v| v.as_str())
                        .or_else(|| s.get("thumbnailUrl").and_then(|v| v.as_str()));
                    url.map(|u| u.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    let icon_url = m
        .get("logo")
        .and_then(|l| l.get("url"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    // Full HTML description (if available)
    let desc_url = format!("{}/mods/{}/description", BASE, id);
    let desc_res = client
        .get(&desc_url)
        .header("x-api-key", &key)
        .header("Accept", "application/json")
        .send()
        .await;
    if let Ok(desc_res) = desc_res {
        if desc_res.status().is_success() {
            if let Ok(desc_json) = desc_res.json::<Value>().await {
                if let Some(desc_data) = desc_json.get("data").and_then(|v| v.as_str()) {
                    body = desc_data.to_string();
                }
            }
        }
    }

    Ok(json!({
        "_source": "curseforge",
        "id": id.to_string(),
        "project_id": id.to_string(),
        "title": title,
        "summary": summary,
        "body": body,
        "gallery": gallery,
        "icon_url": icon_url,
        "project_type": "mod",
    }))
}

/// Get CurseForge mod files. Returns array like Modrinth versions: id, name, game_versions, loaders, files: [{ url, filename, primary }].
pub async fn get_versions(id: &str) -> Result<Value> {
    let (primary, fallback) = match curseforge_api_keys() {
        Some(k) => k,
        None => return Ok(json!([])),
    };

    let id: u64 = match id.parse() {
        Ok(i) => i,
        Err(_) => return Ok(json!([])),
    };

    let client = cf_http_client();
    let files_url = format!("{}/mods/{}/files", BASE, id);
    let mut res = client
        .get(&files_url)
        .header("x-api-key", &primary)
        .header("Accept", "application/json")
        .send()
        .await?;
    if cf_needs_key_retry(res.status().as_u16()) {
        if let Some(ref fb) = fallback {
            if fb != &primary {
                res = client
                    .get(&files_url)
                    .header("x-api-key", fb)
                    .header("Accept", "application/json")
                    .send()
                    .await?;
            }
        }
    }

    if !res.status().is_success() {
        return Ok(json!([]));
    }

    let data: Value = res.json().await?;
    let empty: Vec<Value> = vec![];
    let arr = data
        .get("data")
        .and_then(|d| d.as_array())
        .unwrap_or(&empty);

    let versions: Vec<Value> = arr
        .iter()
        .map(|f| {
            let fid = f.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let display_name = f
                .get("displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let filename = f
                .get("fileName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let url = f
                .get("downloadUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let game_versions: Vec<String> = f
                .get("gameVersions")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            // У файла `gameVersions` — это версии игры, а не модлоадеры.
            // Поэтому loader мы определяем по имени файла (эвристика),
            // чтобы фильтры в UI не "гасили" результаты.
            let lower_hint = format!("{} {}", display_name, filename).to_lowercase();
            let has_neoforge = lower_hint.contains("neoforge") || lower_hint.contains("neo-forge");
            let mut loaders: Vec<String> = Vec::new();
            if lower_hint.contains("fabric") {
                loaders.push("fabric".to_string());
            }
            if has_neoforge {
                loaders.push("neoforge".to_string());
            } else if lower_hint.contains("forge") {
                // Важно: "neoforge" тоже содержит "forge", поэтому выше мы проверили neoforge отдельно.
                loaders.push("forge".to_string());
            }
            if lower_hint.contains("quilt") {
                loaders.push("quilt".to_string());
            }
            if loaders.is_empty() {
                // Если не смогли распознать — не ограничиваем фильтрами.
                loaders = vec![
                    "fabric".to_string(),
                    "forge".to_string(),
                    "neoforge".to_string(),
                    "quilt".to_string(),
                ];
            }
            json!({
                "id": fid.to_string(),
                "name": display_name,
                "game_versions": game_versions,
                "loaders": loaders,
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

async fn cf_send_get(
    client: &reqwest::Client,
    url: &str,
    primary: &str,
    fallback: &Option<String>,
) -> Result<reqwest::Response> {
    let mut res = client
        .get(url)
        .header("x-api-key", primary)
        .header("Accept", "application/json")
        .send()
        .await?;
    if cf_needs_key_retry(res.status().as_u16()) {
        if let Some(ref fb) = fallback {
            if fb != primary {
                res = client
                    .get(url)
                    .header("x-api-key", fb)
                    .header("Accept", "application/json")
                    .send()
                    .await?;
            }
        }
    }
    Ok(res)
}

/// Метаданные одного файла мода/пака (имя, displayName и т.д.).
pub async fn get_mod_file_detail(mod_id: u64, file_id: u64) -> Result<Value> {
    let (primary, fallback) =
        curseforge_api_keys().ok_or_else(|| Error::Custom("Нет ключа CurseForge API".into()))?;
    let client = cf_http_client();
    let file_url = format!("{}/mods/{}/files/{}", BASE, mod_id, file_id);
    let res = cf_send_get(&client, &file_url, &primary, &fallback).await?;
    if !res.status().is_success() {
        return Err(Error::Custom(format!(
            "CurseForge файл {}:{} — HTTP {}",
            mod_id,
            file_id,
            res.status()
        )));
    }
    let full: Value = res.json().await?;
    full.get("data")
        .cloned()
        .ok_or_else(|| Error::Custom("CurseForge: пустой ответ файла".into()))
}

/// Прямая ссылка на CDN (нужна, когда в списке файлов `downloadUrl` пустой).
pub async fn fetch_mod_file_download_url(mod_id: u64, file_id: u64) -> Result<String> {
    let (primary, fallback) =
        curseforge_api_keys().ok_or_else(|| Error::Custom("Нет ключа CurseForge API".into()))?;
    let client = cf_http_client();
    let dl_url = format!("{}/mods/{}/files/{}/download-url", BASE, mod_id, file_id);
    let res = cf_send_get(&client, &dl_url, &primary, &fallback).await?;
    if !res.status().is_success() {
        return Err(Error::Custom(format!(
            "CurseForge download-url {}:{} — HTTP {}",
            mod_id,
            file_id,
            res.status()
        )));
    }
    let full: Value = res.json().await?;
    let s = full
        .get("data")
        .and_then(|d| d.as_str())
        .unwrap_or("")
        .trim();
    if s.is_empty() {
        return Err(Error::Custom(
            "CurseForge: сервер не вернул ссылку скачивания".into(),
        ));
    }
    Ok(s.to_string())
}

/// Hybrid search: Modrinth + CurseForge in parallel, merged by title (no duplicates).
pub async fn search_hybrid(
    query: &str,
    project_type: &str,
    game_version: &str,
    loader: &str,
    categories: Vec<String>,
    page: usize,
    sort: &str,
    sort_desc: bool,
) -> Result<Value> {
    let (mr, cf) = tokio::join!(
        modrinth::search(
            query,
            project_type,
            game_version,
            loader,
            categories.clone(),
            page,
            sort,
            sort_desc,
        ),
        search(
            query,
            project_type,
            game_version,
            loader,
            categories,
            page,
            sort,
            sort_desc,
        ),
    );
    let mr_json = mr.unwrap_or_else(|_| json!({ "hits": [], "total_hits": 0 }));
    let cf_json = cf.unwrap_or_else(|_| json!({ "hits": [], "total_hits": 0 }));
    let mr_hits: Vec<Value> = mr_json
        .get("hits")
        .and_then(|h| h.as_array())
        .cloned()
        .unwrap_or_default();
    let cf_hits: Vec<Value> = cf_json
        .get("hits")
        .and_then(|h| h.as_array())
        .cloned()
        .unwrap_or_default();
    // Дедуплицируем, но сохраняем порядок (сортировку) — сначала Modrinth, потом CurseForge.
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut merged: Vec<Value> = Vec::new();

    for mut h in mr_hits {
        let key = h
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_lowercase();
        if key.is_empty() {
            continue;
        }
        if seen.contains_key(&key) {
            continue;
        }
        seen.insert(key, ());
        h["source"] = json!("modrinth");
        h["modrinth_id"] = h.get("project_id").cloned().unwrap_or(Value::Null);
        merged.push(h);
    }

    for mut h in cf_hits {
        let key = h
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_lowercase();
        if key.is_empty() {
            continue;
        }
        h["curseforge_id"] = h.get("project_id").cloned().unwrap_or(Value::Null);
        if seen.contains_key(&key) {
            // если уже есть — просто обновим поле curseforge_id у найденного элемента
            if let Some(existing_idx) = merged.iter().position(|x| {
                x.get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_lowercase()
                    == key
            }) {
                merged[existing_idx]["curseforge_id"] =
                    h.get("project_id").cloned().unwrap_or(Value::Null);
            }
        } else {
            seen.insert(key, ());
            h["source"] = json!("curseforge");
            merged.push(h);
        }
    }

    let total = merged.len();
    Ok(json!({ "hits": merged, "total_hits": total }))
}

/// Hybrid versions: fetch from both sources; merge and tag with _source.
pub async fn get_hybrid_versions(
    modrinth_id: Option<String>,
    curseforge_id: Option<String>,
) -> Result<Value> {
    let (mr_versions, cf_versions) = if let Some(id) = modrinth_id {
        let cf_id = curseforge_id.clone();
        let (mr, cf) = tokio::join!(modrinth::get_versions(&id), async move {
            if let Some(cid) = cf_id {
                get_versions(&cid).await
            } else {
                Ok(json!([]))
            }
        },);
        (
            mr.ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default(),
            cf.ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default(),
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

/// Fingerprint CurseForge (murmur2, seed 1, без таб/пробел/LF/CR) — как в furse / Prism.
pub fn cf_fingerprint_bytes(bytes: &[u8]) -> u64 {
    let filtered: Vec<u8> = bytes
        .iter()
        .filter(|b| !matches!(**b, 9u8 | 10 | 13 | 32))
        .copied()
        .collect();
    murmur2::murmur2(&filtered, 1) as u64
}

/// HashAlgo: 1 = Sha1 (см. документацию CurseForge API).
pub fn cf_file_sha1_hex(file: &Value) -> Option<String> {
    let arr = file.get("hashes")?.as_array()?;
    for h in arr {
        let algo = h.get("algo")?.as_u64()?;
        if algo == 1 {
            return h.get("value")?.as_str().map(|s| s.to_lowercase());
        }
    }
    None
}

pub fn cf_file_sha512_hex(file: &Value) -> Option<String> {
    let arr = file.get("hashes")?.as_array()?;
    for h in arr {
        let algo = h.get("algo")?.as_u64()?;
        // 4 = Sha512 в ряде версий enum Overwolf; если нет — остаётся пусто в индексе
        if algo == 4 {
            return h.get("value")?.as_str().map(|s| s.to_lowercase());
        }
    }
    None
}

/// Changelog, зависимости и имя файла для карточки версии в UI.
pub async fn get_file_detail_for_ui(mod_id: &str, file_id: &str) -> Result<Value> {
    let mid: u64 = mod_id
        .parse()
        .map_err(|_| Error::Custom("Некорректный CurseForge mod id".into()))?;
    let fid: u64 = file_id
        .parse()
        .map_err(|_| Error::Custom("Некорректный CurseForge file id".into()))?;
    let file = get_mod_file_detail(mid, fid).await?;

    let mut required = Vec::new();
    let mut optional = Vec::new();
    let mut other = Vec::new();

    if let Some(deps) = file.get("dependencies").and_then(|d| d.as_array()) {
        for d in deps {
            let dep_mod = d
                .get("modId")
                .and_then(|v| v.as_u64())
                .or_else(|| d.get("modId").and_then(|v| v.as_i64()).map(|x| x as u64));
            let rel = d.get("relationType").and_then(|v| v.as_u64()).unwrap_or(99);
            let title = if let Some(dm) = dep_mod {
                get_project(&dm.to_string())
                    .await
                    .ok()
                    .and_then(|p| {
                        p.get("title")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| format!("Проект {}", dm))
            } else {
                "?".into()
            };
            let entry = json!({
                "title": title,
                "mod_id": dep_mod,
                "relation_type": rel,
            });
            match rel {
                3 => required.push(entry),
                2 => optional.push(entry),
                _ => other.push(entry),
            }
        }
    }

    let changelog_html = file
        .get("changelog")
        .or_else(|| file.get("changeLog"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let display_name = file
        .get("displayName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let file_name = file
        .get("fileName")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(json!({
        "_source": "curseforge",
        "display_name": display_name,
        "file_name": file_name,
        "changelog_html": changelog_html,
        "game_versions": file.get("gameVersions").cloned().unwrap_or(json!([])),
        "required_dependencies": required,
        "optional_dependencies": optional,
        "other_dependencies": other,
    }))
}

#[derive(Serialize, Clone)]
struct FingerprintRequestBody {
    fingerprints: Vec<u64>,
}

/// POST /v1/fingerprints/432
pub async fn match_fingerprints(fingerprints: Vec<u64>) -> Result<Value> {
    if fingerprints.is_empty() {
        return Ok(json!({ "exactMatches": [] }));
    }
    let (primary, fallback) = match curseforge_api_keys() {
        Some(k) => k,
        None => return Ok(json!({ "exactMatches": [] })),
    };
    let client = cf_http_client();
    let fp_url = format!("{}/fingerprints/{}", BASE, MINECRAFT_GAME_ID);
    let body = FingerprintRequestBody { fingerprints };
    let mut res = client
        .post(&fp_url)
        .header("x-api-key", &primary)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?;
    if cf_needs_key_retry(res.status().as_u16()) {
        if let Some(ref fb) = fallback {
            if fb != &primary {
                res = client
                    .post(&fp_url)
                    .header("x-api-key", fb)
                    .header("Accept", "application/json")
                    .json(&body)
                    .send()
                    .await?;
            }
        }
    }
    if !res.status().is_success() {
        return Ok(json!({ "exactMatches": [] }));
    }
    let full: Value = res.json().await?;
    Ok(full.get("data").cloned().unwrap_or(json!({})))
}
