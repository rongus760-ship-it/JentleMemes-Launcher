use crate::config::get_data_dir;
use crate::core::api::http_client;
use crate::core::progress_emit::emit_download_progress;
use crate::core::types::DownloadProgress;
use crate::error::Result;
use serde_json::Value;
use std::fs;
use tauri::AppHandle;

pub async fn search(
    query: &str,
    project_type: &str,
    game_version: &str,
    loader: &str,
    categories: Vec<String>,
    page: usize,
    sort: &str,
    sort_desc: bool,
) -> Result<Value> {
    let limit = 20;
    let offset = page * limit;
    let mut facets = Vec::new();
    facets.push(vec![format!("project_type:{}", project_type)]);
    if !game_version.is_empty() {
        facets.push(vec![format!("versions:{}", game_version)]);
    }
    if !loader.is_empty() && (project_type == "mod" || project_type == "modpack") {
        facets.push(vec![format!("categories:{}", loader)]);
    }
    for cat in categories {
        facets.push(vec![format!("categories:{}", cat)]);
    }

    let client = http_client();
    // Имя/автор: API не сортирует — берём downloads как базу, потом сортируем локально.
    let index = match sort {
        "relevance" => "relevance",
        "popularity" | "popular" => "downloads",
        "downloads" => "downloads",
        "updated" => "updated",
        "rating" => "follows",
        "name" | "author" => "downloads",
        _ => "relevance",
    };

    let mut req = client.get("https://api.modrinth.com/v2/search").query(&[
        ("limit", limit.to_string()),
        ("offset", offset.to_string()),
        ("index", index.to_string()),
        ("facets", serde_json::to_string(&facets)?),
    ]);
    req = req.query(&[("query", query)]);

    let res = req.send().await?;
    let mut val: Value = res.json().await?;

    if let Some(hits) = val.get_mut("hits").and_then(|h| h.as_array_mut()) {
        match sort {
            "name" => hits.sort_by(|a, b| {
                let ta = a
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                let tb = b
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                ta.cmp(&tb)
            }),
            "author" => hits.sort_by(|a, b| {
                let ta = a
                    .get("author")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                let tb = b
                    .get("author")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_lowercase();
                ta.cmp(&tb)
            }),
            _ => {}
        }

        match sort {
            "name" | "author" => {
                if sort_desc {
                    hits.reverse();
                }
            }
            _ => {
                if !sort_desc {
                    hits.reverse();
                }
            }
        }
    }

    Ok(val)
}

pub async fn get_project(id: &str) -> Result<Value> {
    let client = http_client();
    let url = format!("https://api.modrinth.com/v2/project/{}", id);
    let res = client.get(&url).send().await?;
    Ok(res.json().await?)
}

pub async fn get_versions(id: &str) -> Result<Value> {
    let client = http_client();
    let url = format!("https://api.modrinth.com/v2/project/{}/version", id);
    let res = client.get(&url).send().await?;
    Ok(res.json().await?)
}

pub async fn get_version_by_id(version_id: &str) -> Result<Value> {
    let client = http_client();
    let url = format!("https://api.modrinth.com/v2/version/{}", version_id);
    let res = client.get(&url).send().await?;
    if !res.status().is_success() {
        return Err(crate::error::Error::Custom(format!(
            "Modrinth версия: HTTP {}",
            res.status()
        )));
    }
    Ok(res.json().await?)
}

/// Версия для UI: changelog, зависимости с названиями проектов.
pub async fn get_version_details_for_ui(version_id: &str) -> Result<Value> {
    use serde_json::json;
    let v = get_version_by_id(version_id).await?;
    let mut required = Vec::new();
    let mut optional = Vec::new();
    if let Some(deps) = v.get("dependencies").and_then(|d| d.as_array()) {
        for d in deps {
            let dtype = d
                .get("dependency_type")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let pid = d.get("project_id").and_then(|x| x.as_str()).unwrap_or("");
            let mut title = pid.to_string();
            if !pid.is_empty() {
                if let Ok(p) = get_project(pid).await {
                    title = p
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or(pid)
                        .to_string();
                }
            }
            let entry = json!({
                "title": title,
                "project_id": pid,
                "version_id": d.get("version_id"),
                "dependency_type": dtype,
            });
            match dtype {
                "required" => required.push(entry),
                "optional" => optional.push(entry),
                _ => {}
            }
        }
    }
    let changelog = v
        .get("changelog")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    Ok(json!({
        "_source": "modrinth",
        "version_number": v.get("version_number").cloned().unwrap_or(json!(null)),
        "name": v.get("name").cloned().unwrap_or(json!(null)),
        "changelog": changelog,
        "game_versions": v.get("game_versions").cloned().unwrap_or(json!([])),
        "loaders": v.get("loaders").cloned().unwrap_or(json!([])),
        "required_dependencies": required,
        "optional_dependencies": optional,
        "files": v.get("files").cloned().unwrap_or(json!([])),
    }))
}

pub async fn install_file(
    app: Option<&AppHandle>,
    instance_id: &str,
    url: &str,
    filename: &str,
    project_type: &str,
) -> Result<String> {
    let folder = match project_type {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    };
    if let Some(handle) = app {
        emit_download_progress(
            handle,
            DownloadProgress {
                task_name: format!("Скачивание: {}", filename),
                downloaded: 0,
                total: 1,
                instance_id: Some(instance_id.to_string()),
                ..Default::default()
            },
        );
    }
    let target_dir = get_data_dir()
        .join("instances")
        .join(instance_id)
        .join(folder);
    fs::create_dir_all(&target_dir)?;
    let target_path = target_dir.join(filename);
    let bytes = http_client()
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    fs::write(target_path, bytes)?;
    if let Some(handle) = app {
        emit_download_progress(
            handle,
            DownloadProgress {
                task_name: format!("Скачивание: {}", filename),
                downloaded: 1,
                total: 1,
                instance_id: Some(instance_id.to_string()),
                ..Default::default()
            },
        );
    }
    Ok(format!("Успешно установлено в папку {}", folder))
}
