use std::fs;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;
use zip::{ZipArchive, ZipWriter};
use rfd::FileDialog;
use walkdir::WalkDir;
use futures::StreamExt;
use reqwest::Client;
use sha1::{Sha1, Digest};
use crate::config::{InstanceConfig, get_data_dir, PackSource, InstalledFile, save_pack_source};
use crate::core::types::DownloadProgress;
use crate::error::{Result, Error};

/// Извлекает project_id и version_id из URL Modrinth CDN
fn parse_modrinth_url(url: &str) -> Option<(String, String)> {
    // https://cdn.modrinth.com/data/PROJECT_ID/versions/VERSION_ID/...
    let rest = url.strip_prefix("https://cdn.modrinth.com/data/")
        .or_else(|| url.strip_prefix("http://cdn.modrinth.com/data/"))?;
    let mut parts = rest.split('/');
    let project_id = parts.next()?.to_string();
    if parts.next()? != "versions" { return None; }
    let version_id = parts.next()?.to_string();
    Some((project_id, version_id))
}

pub async fn install_from_url(app: AppHandle, url: &str, name: &str) -> Result<String> {
    let data_dir = get_data_dir();
    let temp_path = data_dir.join(format!("temp_{}.mrpack", Uuid::new_v4().simple()));
    app.emit("download_progress", DownloadProgress { task_name: "Скачивание архива сборки...".into(), downloaded: 0, total: 1, instance_id: None }).ok();
    
    let client = Client::new();
    let bytes = client.get(url).send().await?.bytes().await?;
    let pack_meta = if let Some((project_id, version_id)) = parse_modrinth_url(url) {
        Some(PackInstallMeta::Modrinth { project_id, version_id })
    } else {
        let sha1_hex = format!("{:x}", Sha1::digest(&bytes));
        Some(PackInstallMeta::Custom { pack_url: url.to_string(), sha1: sha1_hex })
    };
    fs::write(&temp_path, bytes)?;
    let res = install(app, temp_path.to_string_lossy().as_ref(), name, pack_meta).await;
    let _ = fs::remove_file(temp_path);
    res
}

pub(crate) enum PackInstallMeta {
    Modrinth { project_id: String, version_id: String },
    Custom { pack_url: String, sha1: String },
}

pub async fn install(app: AppHandle, file_path: &str, name: &str, pack_meta: Option<PackInstallMeta>) -> Result<String> {
    let base_name = crate::core::instance::sanitize_filename(name);
    let mut folder_name = base_name.clone();
    let mut inst_dir = get_data_dir().join("instances").join(&folder_name);
    let mut counter = 2;
    while inst_dir.exists() {
        folder_name = format!("{}_{}", base_name, counter);
        inst_dir = get_data_dir().join("instances").join(&folder_name);
        counter += 1;
    }
    let id = folder_name.clone();
    fs::create_dir_all(&inst_dir)?;

    let file = fs::File::open(file_path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut index_content = String::new();
    if let Ok(mut index_file) = archive.by_name("modrinth.index.json") {
        index_file.read_to_string(&mut index_content)?;
    } else {
        return Err(Error::Custom("Это не файл .mrpack".into()));
    }

    let index: serde_json::Value = serde_json::from_str(&index_content)?;
    let mc_version = index["dependencies"]["minecraft"].as_str().unwrap_or("1.20.1").to_string();

    let mut loader = "vanilla".to_string();
    let mut loader_version = String::new();
    if let Some(v) = index["dependencies"].get("fabric-loader").and_then(|v| v.as_str()) {
        loader = "fabric".to_string(); loader_version = v.to_string();
    } else if let Some(v) = index["dependencies"].get("quilt-loader").and_then(|v| v.as_str()) {
        loader = "quilt".to_string(); loader_version = v.to_string();
    } else if let Some(v) = index["dependencies"].get("forge").and_then(|v| v.as_str()) {
        loader = "forge".to_string(); loader_version = v.to_string();
    } else if let Some(v) = index["dependencies"].get("neoforge").and_then(|v| v.as_str()) {
        loader = "neoforge".to_string(); loader_version = v.to_string();
    }

    // Extract icon from mrpack if present
    let mut icon_path_str = String::new();
    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            let fname = file.name().to_string();
            if fname == "icon.png" || fname == "icon.jpg" || fname == "icon.webp" {
                let ext = fname.rsplit('.').next().unwrap_or("png");
                let icon_dest = inst_dir.join(format!(".icon.{}", ext));
                if let Ok(mut out) = fs::File::create(&icon_dest) {
                    let _ = std::io::copy(&mut file, &mut out);
                    icon_path_str = icon_dest.to_string_lossy().to_string();
                }
                break;
            }
        }
    }

    let conf = InstanceConfig {
        id: id.clone(), name: name.to_string(), game_version: mc_version, loader,
        loader_version, icon: icon_path_str, settings: None, playtime: 0,
    };
    fs::write(inst_dir.join("instance.json"), serde_json::to_string_pretty(&conf)?)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if let Some(name) = file.enclosed_name() {
            let name_str = name.to_string_lossy().to_string();
            if name_str.starts_with("overrides") {
                let relative_path = name.strip_prefix("overrides").unwrap();
                let outpath = inst_dir.join(relative_path);
                if file.name().ends_with('/') { fs::create_dir_all(&outpath)?; } 
                else {
                    if let Some(p) = outpath.parent() { fs::create_dir_all(p)?; }
                    let mut outfile = fs::File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
        }
    }

    let mut tasks = Vec::new();
    if let Some(files) = index["files"].as_array() {
        for f in files {
            if let (Some(path), Some(downloads)) = (f["path"].as_str(), f["downloads"].as_array()) {
                if let Some(url) = downloads.get(0).and_then(|u| u.as_str()) {
                    tasks.push((url.to_string(), inst_dir.join(path)));
                }
            }
        }
    }

    let total = tasks.len();
    if total > 0 {
        let dc = Arc::new(AtomicUsize::new(0));
        let inst_id = id.clone();
        app.emit("download_progress", DownloadProgress { task_name: "Скачивание модов сборки".into(), downloaded: 0, total, instance_id: Some(inst_id.clone()) }).ok();

        let client = Client::new();
        let stream = futures::stream::iter(tasks.into_iter().map(|(url, path)| {
            let client = client.clone();
            let dc = dc.clone();
            let app = app.clone();
            let inst_id = inst_id.clone();
            async move {
                if let Some(p) = path.parent() { let _ = tokio::fs::create_dir_all(p).await; }
                if let Ok(r) = client.get(&url).send().await {
                    if let Ok(b) = r.bytes().await { let _ = tokio::fs::write(&path, b).await; }
                }
                let cur = dc.fetch_add(1, Ordering::SeqCst) + 1;
                app.emit("download_progress", DownloadProgress { task_name: "Скачивание модов".into(), downloaded: cur, total, instance_id: Some(inst_id.clone()) }).ok();
            }
        }));
        stream.buffer_unordered(32).collect::<Vec<()>>().await;
    }

    // Build mod metadata (icons, titles, versions) from Modrinth API
    if let Err(e) = crate::core::mods::build_metadata(&app, &id).await {
        eprintln!("Не удалось загрузить метаданные модов: {}", e);
    }

    // Сохраняем pack_source для проверки обновлений
    if let Some(meta) = pack_meta {
        let installed_files: Vec<InstalledFile> = index["files"].as_array()
            .map(|arr| arr.iter().filter_map(|f| {
                let path = f["path"].as_str()?;
                let sha1 = f["hashes"].get("sha1").and_then(|v| v.as_str()).unwrap_or("");
                Some(InstalledFile { path: path.to_string(), sha1: sha1.to_string() })
            }).collect())
            .unwrap_or_default();
        let source = match meta {
            PackInstallMeta::Modrinth { project_id, version_id } => PackSource::Modrinth {
                project_id, version_id, version_name: None, installed_files,
            },
            PackInstallMeta::Custom { pack_url, sha1 } => PackSource::Custom {
                pack_url, pack_id: None, installed_mrpack_sha1: sha1, installed_files,
            },
        };
        let _ = save_pack_source(&inst_dir, &source);
    }

    Ok("Сборка успешно установлена!".into())
}

pub fn export(id: &str, selected_folders: Vec<String>) -> Result<String> {
    let inst_dir = get_data_dir().join("instances").join(id);
    if !inst_dir.exists() { return Err(Error::Custom("Сборка не найдена".into())); }
    
    if let Some(save_path) = FileDialog::new().set_file_name(&format!("{}.zip", id)).add_filter("ZIP Archive", &["zip"]).save_file() {
        let file = fs::File::create(save_path)?;
        let mut zip = ZipWriter::new(file);
        let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut buffer = Vec::new();

        for entry in WalkDir::new(&inst_dir) {
            let entry = entry?;
            let path = entry.path();
            let name = path.strip_prefix(&inst_dir).unwrap().to_string_lossy().replace("\\", "/");

            let should_include = name == "instance.json" || selected_folders.iter().any(|f| name.starts_with(f));
            if !should_include { continue; }

            if path.is_file() {
                zip.start_file(name, options)?;
                let mut f = fs::File::open(path)?;
                f.read_to_end(&mut buffer)?;
                zip.write_all(&buffer)?;
                buffer.clear();
            } else if !name.is_empty() {
                zip.add_directory(name, options)?;
            }
        }
        zip.finish()?;
        return Ok("Сборка экспортирована".into());
    }
    Err(Error::Custom("Экспорт отменен".into()))
}

pub fn export_mrpack(id: &str, selected_folders: Vec<String>) -> Result<String> {
    let inst_dir = get_data_dir().join("instances").join(id);
    if !inst_dir.exists() { return Err(Error::Custom("Сборка не найдена".into())); }

    let config_path = inst_dir.join("instance.json");
    let config: serde_json::Value = if config_path.exists() {
        serde_json::from_str(&fs::read_to_string(&config_path)?)?
    } else {
        serde_json::json!({})
    };

    let game_version = config.get("game_version").and_then(|v| v.as_str()).unwrap_or("1.20.1");
    let loader = config.get("loader").and_then(|v| v.as_str()).unwrap_or("vanilla");
    let loader_version = config.get("loader_version").and_then(|v| v.as_str()).unwrap_or("");

    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".to_string(), serde_json::Value::String(game_version.to_string()));
    if loader != "vanilla" && !loader_version.is_empty() {
        deps.insert(loader.to_string(), serde_json::Value::String(loader_version.to_string()));
    }

    let modrinth_index = serde_json::json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": config.get("name").and_then(|v| v.as_str()).unwrap_or(id),
        "files": [],
        "dependencies": deps
    });

    if let Some(save_path) = FileDialog::new().set_file_name(&format!("{}.mrpack", id)).add_filter("Modrinth Pack", &["mrpack"]).save_file() {
        let file = fs::File::create(save_path)?;
        let mut zip = ZipWriter::new(file);
        let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut buffer = Vec::new();

        zip.start_file("modrinth.index.json", options)?;
        zip.write_all(serde_json::to_string_pretty(&modrinth_index)?.as_bytes())?;

        for entry in WalkDir::new(&inst_dir) {
            let entry = entry?;
            let path = entry.path();
            let name = path.strip_prefix(&inst_dir).unwrap().to_string_lossy().replace("\\", "/");

            let should_include = selected_folders.iter().any(|f| name.starts_with(f));
            if !should_include { continue; }

            let override_name = format!("overrides/{}", name);
            if path.is_file() {
                zip.start_file(&override_name, options)?;
                let mut f = fs::File::open(path)?;
                f.read_to_end(&mut buffer)?;
                zip.write_all(&buffer)?;
                buffer.clear();
            } else if !name.is_empty() {
                zip.add_directory(&override_name, options)?;
            }
        }
        zip.finish()?;
        return Ok("Экспортировано в .mrpack".into());
    }
    Err(Error::Custom("Экспорт отменен".into()))
}

pub fn import() -> Result<String> {
    if let Some(file_path) = FileDialog::new().add_filter("ZIP Archive", &["zip"]).pick_file() {
        let id = format!("inst_{}", Uuid::new_v4().simple());
        let inst_dir = get_data_dir().join("instances").join(&id);
        fs::create_dir_all(&inst_dir)?;

        let file = fs::File::open(file_path)?;
        let mut archive = ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() { Some(path) => inst_dir.join(path), None => continue };
            if file.name().ends_with('/') { fs::create_dir_all(&outpath)?; } 
            else {
                if let Some(p) = outpath.parent() { fs::create_dir_all(p)?; }
                let mut outfile = fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }
        }

        let json_path = inst_dir.join("instance.json");
        if json_path.exists() {
            let content = fs::read_to_string(json_path)?;
            if let Ok(mut conf) = serde_json::from_str::<InstanceConfig>(&content) {
                conf.id = id.clone();
                fs::write(inst_dir.join("instance.json"), serde_json::to_string_pretty(&conf)?)?;
            }
        }
        return Ok("Сборка успешно импортирована!".into());
    }
    Err(Error::Custom("Импорт отменен".into()))
}

/// Проверяет наличие обновления сборки. Возвращает JSON:
/// { has_update, current_version?, latest_version?, source: "modrinth"|"custom", update_url? }
pub async fn check_modpack_update(instance_id: &str) -> Result<serde_json::Value> {
    let inst_dir = get_data_dir().join("instances").join(instance_id);
    if !inst_dir.exists() {
        return Err(Error::Custom("Инстанс не найден".into()));
    }
    let source = match crate::config::load_pack_source(&inst_dir)? {
        Some(s) => s,
        None => {
            return Ok(serde_json::json!({
                "has_update": false,
                "source": "unknown",
                "reason": "Нет информации об источнике сборки"
            }));
        }
    };

    match source {
        PackSource::Modrinth { project_id, version_id, version_name, .. } => {
            let versions: serde_json::Value = crate::core::modrinth::get_versions(&project_id).await?;
            let versions_arr = versions.as_array().ok_or_else(|| Error::Custom("Неверный ответ API".into()))?;
            let latest = versions_arr.first().and_then(|v| v.get("id")).and_then(|v| v.as_str());
            let latest_name = versions_arr.first().and_then(|v| v.get("name")).and_then(|v| v.as_str());
            let latest_files = versions_arr.first().and_then(|v| v.get("files")).and_then(|v| v.as_array());
            let download_url = latest_files.and_then(|arr| arr.iter().find(|f| {
                f.get("filename").and_then(|n| n.as_str()).map_or(false, |n| n.ends_with(".mrpack"))
            })).and_then(|f| f.get("url")).and_then(|u| u.as_str());

            let has_update = latest.map(|l| l != version_id).unwrap_or(false);
            Ok(serde_json::json!({
                "has_update": has_update,
                "current_version": version_name.as_deref().unwrap_or(&version_id),
                "latest_version": latest_name.unwrap_or(latest.unwrap_or("")),
                "source": "modrinth",
                "update_url": download_url,
            }))
        }
        PackSource::Custom { pack_url, installed_mrpack_sha1, .. } => {
            let packs = crate::custom_packs::get_cached_packs();
            let arr = packs.as_array().or_else(|| packs.get("packs").and_then(|p| p.as_array()));
            let packs_list: Vec<&serde_json::Value> = arr.map(|a| a.iter().collect()).unwrap_or_default();
            let pack = packs_list.into_iter().find(|p| {
                let u = p.get("url").or(p.get("mrpack_url")).or(p.get("download_url")).and_then(|v| v.as_str());
                u.map(|url| url == pack_url).unwrap_or(false)
            });

            let (remote_sha1, remote_url) = if let Some(p) = pack {
                let sha1 = p.get("sha1").or(p.get("mrpack_sha1")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let url = p.get("url").or(p.get("mrpack_url")).or(p.get("download_url")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                (sha1, url)
            } else {
                (String::new(), pack_url.clone())
            };

            let has_update = !remote_sha1.is_empty() && remote_sha1 != installed_mrpack_sha1;
            Ok(serde_json::json!({
                "has_update": has_update,
                "current_sha1": installed_mrpack_sha1,
                "latest_sha1": remote_sha1,
                "source": "custom",
                "update_url": if has_update { remote_url } else { String::new() },
            }))
        }
    }
}

/// Обновляет сборку (только изменённые файлы по SHA1)
pub async fn update_modpack(app: AppHandle, instance_id: &str, update_url: &str) -> Result<String> {
    let inst_dir = get_data_dir().join("instances").join(instance_id);
    if !inst_dir.exists() {
        return Err(Error::Custom("Инстанс не найден".into()));
    }
    let source = crate::config::load_pack_source(&inst_dir)?.ok_or_else(|| Error::Custom("Нет информации об источнике".into()))?;

    let old_files = match &source {
        PackSource::Modrinth { installed_files, .. } | PackSource::Custom { installed_files, .. } => {
            installed_files.iter().map(|f| (f.path.clone(), f.sha1.clone())).collect::<std::collections::HashMap<_, _>>()
        }
    };

    app.emit("download_progress", DownloadProgress { task_name: "Скачивание обновления...".into(), downloaded: 0, total: 1, instance_id: Some(instance_id.to_string()) }).ok();
    let bytes = Client::new().get(update_url).send().await?.bytes().await?;

    let data_dir = get_data_dir();
    let temp_path = data_dir.join(format!("temp_update_{}.mrpack", Uuid::new_v4().simple()));
    fs::write(&temp_path, bytes.as_ref())?;

    let file = fs::File::open(&temp_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut index_content = String::new();
    archive.by_name("modrinth.index.json")?.read_to_string(&mut index_content)?;
    let index: serde_json::Value = serde_json::from_str(&index_content)?;
    drop(archive);

    let new_files: Vec<(String, String, String)> = index["files"].as_array()
        .map(|arr| arr.iter().filter_map(|f| {
            let path = f["path"].as_str()?;
            let sha1 = f["hashes"].get("sha1").and_then(|v| v.as_str()).unwrap_or("");
            let url = f["downloads"].as_array()?.get(0)?.as_str()?;
            Some((path.to_string(), sha1.to_string(), url.to_string()))
        }).collect())
        .unwrap_or_default();

    let mut to_download = Vec::new();
    let to_remove: Vec<String> = old_files.keys().filter(|p| !new_files.iter().any(|(np, _, _)| np == *p)).cloned().collect();

    for (path, new_sha1, url) in &new_files {
        let old_sha1 = old_files.get(path).map(|s| s.as_str()).unwrap_or("");
        if old_sha1 != *new_sha1 {
            to_download.push((path.clone(), url.clone()));
        }
    }

    for path in &to_remove {
        let full = inst_dir.join(path);
        if full.exists() { let _ = fs::remove_file(&full); }
    }

    let total = to_download.len();
    if total > 0 {
        let dc = Arc::new(AtomicUsize::new(0));
        let inst_id = instance_id.to_string();
        app.emit("download_progress", DownloadProgress { task_name: "Скачивание обновлённых модов".into(), downloaded: 0, total, instance_id: Some(inst_id.clone()) }).ok();
        let client = Client::new();
        let stream = futures::stream::iter(to_download.into_iter().map(|(path, url)| {
            let client = client.clone();
            let dc = dc.clone();
            let app = app.clone();
            let inst_dir = inst_dir.clone();
            let inst_id = inst_id.clone();
            async move {
                let full_path = inst_dir.join(&path);
                if let Some(p) = full_path.parent() { let _ = tokio::fs::create_dir_all(p).await; }
                if let Ok(r) = client.get(&url).send().await {
                    if let Ok(b) = r.bytes().await { let _ = tokio::fs::write(&full_path, b).await; }
                }
                let cur = dc.fetch_add(1, Ordering::SeqCst) + 1;
                app.emit("download_progress", DownloadProgress { task_name: "Скачивание модов".into(), downloaded: cur, total, instance_id: Some(inst_id.clone()) }).ok();
            }
        }));
        stream.buffer_unordered(32).collect::<Vec<()>>().await;
    }

    let installed_files: Vec<InstalledFile> = new_files.iter().map(|(path, sha1, _)| InstalledFile { path: path.clone(), sha1: sha1.clone() }).collect();
    let new_source = match &source {
        PackSource::Modrinth { project_id, .. } => {
            let (_, new_version_id) = parse_modrinth_url(update_url).unwrap_or((project_id.clone(), String::new()));
            PackSource::Modrinth {
                project_id: project_id.clone(),
                version_id: new_version_id,
                version_name: None,
                installed_files,
            }
        }
        PackSource::Custom { pack_url, pack_id, .. } => {
            let new_sha1 = format!("{:x}", Sha1::digest(fs::read(&temp_path)?));
            PackSource::Custom { pack_url: pack_url.clone(), pack_id: pack_id.clone(), installed_mrpack_sha1: new_sha1, installed_files }
        }
    };
    let _ = fs::remove_file(temp_path);
    crate::config::save_pack_source(&inst_dir, &new_source)?;

    if let Err(e) = crate::core::mods::build_metadata(&app, instance_id).await {
        eprintln!("Не удалось обновить метаданные модов: {}", e);
    }
    Ok("Сборка обновлена".into())
}