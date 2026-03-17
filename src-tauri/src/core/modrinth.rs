use serde_json::Value;
use reqwest::Client;
use std::fs;
use tauri::{AppHandle, Emitter};
use crate::config::get_data_dir;
use crate::core::types::DownloadProgress;
use crate::error::Result;

pub async fn search(
    query: &str, project_type: &str, game_version: &str, 
    loader: &str, categories: Vec<String>, page: usize
) -> Result<Value> {
    let limit = 20;
    let offset = page * limit;
    let mut facets = Vec::new();
    facets.push(vec![format!("project_type:{}", project_type)]);
    if !game_version.is_empty() { facets.push(vec![format!("versions:{}", game_version)]); }
    if !loader.is_empty() && (project_type == "mod" || project_type == "modpack") { facets.push(vec![format!("categories:{}", loader)]); }
    for cat in categories { facets.push(vec![format!("categories:{}", cat)]); }

    let client = Client::builder().user_agent("JentleMemesLauncher/1.0").build()?;
    let mut req = client.get("https://api.modrinth.com/v2/search")
        .query(&[("limit", limit.to_string()), ("offset", offset.to_string()), ("facets", serde_json::to_string(&facets)?)]);
    if !query.is_empty() { req = req.query(&[("query", query)]); }
    
    let res = req.send().await?;
    Ok(res.json().await?)
}

pub async fn get_project(id: &str) -> Result<Value> {
    let client = Client::builder().user_agent("JentleMemes/1.0").build()?;
    let url = format!("https://api.modrinth.com/v2/project/{}", id);
    let res = client.get(&url).send().await?;
    Ok(res.json().await?)
}

pub async fn get_versions(id: &str) -> Result<Value> {
    let client = Client::builder().user_agent("JentleMemes/1.0").build()?;
    let url = format!("https://api.modrinth.com/v2/project/{}/version", id);
    let res = client.get(&url).send().await?;
    Ok(res.json().await?)
}

pub async fn install_file(app: Option<&AppHandle>, instance_id: &str, url: &str, filename: &str, project_type: &str) -> Result<String> {
    let folder = match project_type {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    };
    if let Some(handle) = app {
        let _ = handle.emit("download_progress", DownloadProgress {
            task_name: format!("Скачивание: {}", filename),
            downloaded: 0,
            total: 1,
            instance_id: Some(instance_id.to_string()),
        });
    }
    let target_dir = get_data_dir().join("instances").join(instance_id).join(folder);
    fs::create_dir_all(&target_dir)?;
    let target_path = target_dir.join(filename);
    let bytes = reqwest::get(url).await?.bytes().await?;
    fs::write(target_path, bytes)?;
    if let Some(handle) = app {
        let _ = handle.emit("download_progress", DownloadProgress {
            task_name: format!("Скачивание: {}", filename),
            downloaded: 1,
            total: 1,
            instance_id: Some(instance_id.to_string()),
        });
    }
    Ok(format!("Успешно установлено в папку {}", folder))
}