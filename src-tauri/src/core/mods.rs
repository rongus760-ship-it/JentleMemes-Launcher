use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use sha1::{Sha1, Digest};
use tauri::AppHandle;
use reqwest::Client;
use crate::config::get_data_dir;
use crate::error::Result;
use crate::core::progress_emit::emit_download_progress;
use crate::core::task_signals;
use crate::core::types::DownloadProgress;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModMeta {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub icon_url: String,
    pub version_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModInfo {
    pub filename: String,
    pub clean_name: String,
    pub enabled: bool,
    pub hash: String,
    pub title: String,
    pub icon_url: String,
    pub version_name: String,
    pub project_id: String,
    pub version_id: String,
}

pub fn get_installed(instance_id: &str) -> Result<Vec<ModInfo>> {
    get_installed_from_folder(instance_id, "mods")
}

fn meta_path_for(instance_id: &str, folder: &str) -> std::path::PathBuf {
    let data_dir = get_data_dir().join("instances").join(instance_id).join(".data");
    let _ = fs::create_dir_all(&data_dir);
    data_dir.join(format!("{}_meta.json", folder))
}

pub fn get_installed_from_folder(instance_id: &str, folder: &str) -> Result<Vec<ModInfo>> {
    let content_dir = get_data_dir().join("instances").join(instance_id).join(folder);
    fs::create_dir_all(&content_dir)?;

    let mut meta_map: HashMap<String, ModMeta> = HashMap::new();
    let new_meta_path = meta_path_for(instance_id, folder);
    let old_meta_path = content_dir.join("mod_meta.json");
    // Migrate from old location
    if old_meta_path.exists() && !new_meta_path.exists() {
        let _ = fs::copy(&old_meta_path, &new_meta_path);
        let _ = fs::remove_file(&old_meta_path);
    }
    if new_meta_path.exists() {
        let content = fs::read_to_string(&new_meta_path)?;
        if let Ok(m) = serde_json::from_str(&content) { meta_map = m; }
    }

    let mut mods = Vec::new();
    let valid_ext = match folder {
        "resourcepacks" | "shaderpacks" => vec![".zip", ".jar"],
        _ => vec![".jar"],
    };
    if let Ok(entries) = fs::read_dir(&content_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let filename = entry.file_name().to_string_lossy().to_string();
                let is_valid = valid_ext.iter().any(|ext| filename.ends_with(ext) || filename.ends_with(&format!("{}.disabled", ext)));
                if is_valid {
                    let enabled = !filename.ends_with(".disabled");
                    let clean_name = filename.replace(".disabled", "");
                    let mut file = fs::File::open(&path)?;
                    let mut hasher = Sha1::new();
                    std::io::copy(&mut file, &mut hasher)?;
                    let hash = format!("{:x}", hasher.finalize());
                    let meta = meta_map.get(&clean_name);
                    mods.push(ModInfo {
                        filename,
                        clean_name: clean_name.clone(),
                        enabled,
                        hash,
                        title: meta.map(|m| m.title.clone()).unwrap_or_else(|| clean_name.replace(".jar", "")),
                        icon_url: meta.map(|m| m.icon_url.clone()).unwrap_or_default(),
                        version_name: meta.map(|m| m.version_name.clone()).unwrap_or_else(|| "Неизвестно".into()),
                        project_id: meta.map(|m| m.project_id.clone()).unwrap_or_default(),
                        version_id: meta.map(|m| m.version_id.clone()).unwrap_or_default(),
                    });
                }
            } else if path.is_dir() && matches!(folder, "resourcepacks" | "shaderpacks") {
                // Распакованные ресурспаки/шейдеры — папка в корне (Iris/Oculus часто так ставят)
                let filename = entry.file_name().to_string_lossy().to_string();
                if filename.starts_with('.') {
                    continue;
                }
                let enabled = !filename.ends_with(".disabled");
                let clean_name = filename.replace(".disabled", "");
                let meta = meta_map.get(&clean_name);
                mods.push(ModInfo {
                    filename: filename.clone(),
                    clean_name: clean_name.clone(),
                    enabled,
                    hash: String::new(),
                    title: meta
                        .map(|m| m.title.clone())
                        .unwrap_or_else(|| clean_name.clone()),
                    icon_url: meta.map(|m| m.icon_url.clone()).unwrap_or_default(),
                    version_name: meta
                        .map(|m| m.version_name.clone())
                        .unwrap_or_else(|| "Папка".into()),
                    project_id: meta.map(|m| m.project_id.clone()).unwrap_or_default(),
                    version_id: meta.map(|m| m.version_id.clone()).unwrap_or_default(),
                });
            }
        }
    }
    Ok(mods)
}

pub fn toggle(instance_id: &str, folder: &str, filename: &str, enable: bool) -> Result<()> {
    let dir = get_data_dir().join("instances").join(instance_id).join(folder);
    let old_path = dir.join(filename);
    let new_filename = if enable {
        filename.replace(".disabled", "")
    } else {
        if filename.ends_with(".disabled") { filename.to_string() } else { format!("{}.disabled", filename) }
    };
    let new_path = dir.join(&new_filename);
    if old_path.exists() && old_path != new_path { fs::rename(old_path, new_path)?; }
    Ok(())
}

pub async fn delete(instance_id: &str, folder: &str, filename: &str) -> Result<()> {
    let path = get_data_dir().join("instances").join(instance_id).join(folder).join(filename);
    match tokio::fs::metadata(&path).await {
        Ok(meta) => {
            if meta.is_dir() {
                tokio::fs::remove_dir_all(&path).await?;
            } else {
                tokio::fs::remove_file(&path).await?;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

pub async fn check_updates(hashes: &[String], loader: &str, game_version: &str) -> Result<serde_json::Value> {
    let client = Client::builder().user_agent("JentleMemes/1.0").build()?;
    let payload = serde_json::json!({
        "hashes": hashes,
        "algorithm": "sha1",
        "loaders": [loader],
        "game_versions": [game_version]
    });
    let res = client.post("https://api.modrinth.com/v2/version_files/update").json(&payload).send().await?;
    Ok(res.json().await?)
}

pub async fn install_with_dependencies(app: AppHandle, instance_id: &str, version_id: &str, game_version: &str, loader: &str, download_deps: bool) -> Result<String> {
    let mods_dir = get_data_dir().join("instances").join(instance_id).join("mods");
    fs::create_dir_all(&mods_dir)?;

    let client = Client::builder().user_agent("JentleMemes/1.0").build()?;
    let mut to_download = vec![version_id.to_string()];
    let mut meta_map: HashMap<String, ModMeta> = HashMap::new();
    let meta_path = meta_path_for(instance_id, "mods");
    if meta_path.exists() {
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(m) = serde_json::from_str(&content) { meta_map = m; }
        }
    }

    let existing_project_ids: std::collections::HashSet<String> = meta_map.values().map(|m| m.project_id.clone()).filter(|id| !id.is_empty()).collect();

    let mut current_idx = 0;
    while current_idx < to_download.len() {
        let vid = to_download[current_idx].clone();
        current_idx += 1;
        let v_url = format!("https://api.modrinth.com/v2/version/{}", vid);
        if let Ok(res) = client.get(&v_url).send().await {
            if let Ok(v_data) = res.json::<serde_json::Value>().await {
                if let Some(files) = v_data["files"].as_array() {
                    let file = files.iter().find(|f| f["primary"].as_bool().unwrap_or(false)).unwrap_or(&files[0]);
                    let url = file["url"].as_str().unwrap_or("");
                    let filename = file["filename"].as_str().unwrap_or("");
                    if !url.is_empty() && !filename.is_empty() {
                        let file_path = mods_dir.join(filename);
                        // Remove old version of the same project
                        let pid = v_data["project_id"].as_str().unwrap_or("");
                        if !pid.is_empty() {
                            let old_files: Vec<String> = meta_map.iter()
                                .filter(|(_, m)| m.project_id == pid)
                                .map(|(k, _)| k.clone())
                                .collect();
                            for old_file in old_files {
                                let old_path = mods_dir.join(&old_file);
                                if old_path.exists() && old_file != filename {
                                    let _ = fs::remove_file(&old_path);
                                }
                                meta_map.remove(&old_file);
                            }
                        }
                        let pid_before = v_data["project_id"].as_str().unwrap_or("");
                        let dep_already_installed = !pid_before.is_empty() && current_idx > 1 && (existing_project_ids.contains(pid_before) || meta_map.values().any(|m| m.project_id == pid_before));
                        if dep_already_installed && file_path.exists() { continue; }
                        if !file_path.exists() {
                            emit_download_progress(&app, DownloadProgress {
                                task_name: format!("Скачивание: {}", filename),
                                downloaded: current_idx, total: to_download.len(),
                                instance_id: Some(instance_id.to_string()),
                                ..Default::default()
                            });
                            if let Ok(r) = client.get(url).send().await {
                                if let Ok(b) = r.bytes().await { fs::write(&file_path, b).ok(); }
                            }
                        }
                        let pid = v_data["project_id"].as_str().unwrap_or("");
                        let mut title = filename.to_string();
                        let mut icon_url = "".to_string();
                        if !pid.is_empty() {
                            let p_url = format!("https://api.modrinth.com/v2/project/{}", pid);
                            if let Ok(p_res) = client.get(&p_url).send().await {
                                if let Ok(p_data) = p_res.json::<serde_json::Value>().await {
                                    title = p_data["title"].as_str().unwrap_or(filename).to_string();
                                    icon_url = p_data["icon_url"].as_str().unwrap_or("").to_string();
                                }
                            }
                        }
                        meta_map.insert(filename.to_string(), ModMeta {
                            project_id: pid.to_string(), version_id: vid.clone(),
                            title, icon_url, version_name: v_data["version_number"].as_str().unwrap_or("").to_string(),
                        });
                        if download_deps {
                            if let Some(deps) = v_data["dependencies"].as_array() {
                                for dep in deps {
                                    if dep["dependency_type"].as_str().unwrap_or("") == "required" {
                                        let dep_pid = dep["project_id"].as_str().unwrap_or("");
                                        if !dep_pid.is_empty() && existing_project_ids.contains(dep_pid) {
                                            continue;
                                        }
                                        if let Some(dep_vid) = dep["version_id"].as_str() {
                                            if !to_download.contains(&dep_vid.to_string()) {
                                                to_download.push(dep_vid.to_string());
                                            }
                                        } else if !dep_pid.is_empty() {
                                            let res_url = format!("https://api.modrinth.com/v2/project/{}/version?game_versions=[%22{}%22]&loaders=[%22{}%22]", dep_pid, game_version, loader);
                                            if let Ok(dep_res) = client.get(&res_url).send().await {
                                                if let Ok(dep_v_data) = dep_res.json::<Vec<serde_json::Value>>().await {
                                                    if let Some(first_v) = dep_v_data.get(0) {
                                                        if let Some(resolved_vid) = first_v["id"].as_str() {
                                                            if !to_download.contains(&resolved_vid.to_string()) {
                                                                to_download.push(resolved_vid.to_string());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fs::write(&meta_path, serde_json::to_string_pretty(&meta_map)?)?;
    emit_download_progress(&app, DownloadProgress {
        task_name: "Готово".into(),
        downloaded: 1,
        total: 1,
        instance_id: Some(instance_id.to_string()),
        ..Default::default()
    });
    Ok("Мод и зависимости установлены!".into())
}

/// Опции загрузки/проверки меты контента (фон vs кнопка «обновить»).
#[derive(Clone, Copy)]
pub struct ContentMetaOpts {
    pub silent: bool,
    /// Если задано, прерываемся, когда `task_signals` эпоха перестала совпадать (появился видимый таск).
    pub cancel_after_epoch: Option<u64>,
}

#[inline]
fn meta_aborted(cancel_after_epoch: Option<u64>) -> bool {
    cancel_after_epoch
        .map(task_signals::background_meta_cancelled)
        .unwrap_or(false)
}

/// Scans content files and looks up their metadata via Modrinth hash lookup (с прогрессом в UI).
pub async fn build_metadata(app: &AppHandle, instance_id: &str) -> Result<()> {
    let opts = ContentMetaOpts {
        silent: false,
        cancel_after_epoch: None,
    };
    build_metadata_with_opts(app, instance_id, opts).await
}

/// Фоновая мета: без событий в UI, отмена при любом несilent download_progress.
pub async fn build_metadata_background(app: &AppHandle, instance_id: &str, start_epoch: u64) -> Result<()> {
    let opts = ContentMetaOpts {
        silent: true,
        cancel_after_epoch: Some(start_epoch),
    };
    build_metadata_with_opts(app, instance_id, opts).await
}

async fn build_metadata_with_opts(
    app: &AppHandle,
    instance_id: &str,
    opts: ContentMetaOpts,
) -> Result<()> {
    for folder in &["mods", "resourcepacks", "shaderpacks"] {
        if meta_aborted(opts.cancel_after_epoch) {
            return Ok(());
        }
        let _ = build_metadata_for_folder(app, instance_id, folder, opts).await;
    }
    Ok(())
}

async fn build_metadata_for_folder(
    app: &AppHandle,
    instance_id: &str,
    folder: &str,
    opts: ContentMetaOpts,
) -> Result<()> {
    let content_dir = get_data_dir().join("instances").join(instance_id).join(folder);
    if !content_dir.exists() { return Ok(()); }

    let meta_path = meta_path_for(instance_id, folder);
    let mut meta_map: HashMap<String, ModMeta> = HashMap::new();
    if meta_path.exists() {
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(m) = serde_json::from_str(&content) { meta_map = m; }
        }
    }

    let valid_ext: &[&str] = match folder {
        "resourcepacks" | "shaderpacks" => &[".zip", ".jar"],
        _ => &[".jar"],
    };

    let mut hashes = Vec::new();
    let mut hash_to_file: HashMap<String, String> = HashMap::new();

    if let Ok(entries) = fs::read_dir(&content_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = entry.file_name().to_string_lossy().to_string();
            let is_valid = path.is_file() && valid_ext.iter().any(|ext| fname.ends_with(ext) || fname.ends_with(&format!("{}.disabled", ext)));
            if !is_valid { continue; }
            let clean_name = fname.replace(".disabled", "");
            let needs_lookup = !meta_map.contains_key(&clean_name)
                || meta_map
                    .get(&clean_name)
                    .map(|m| m.project_id.is_empty() && m.version_id.is_empty())
                    .unwrap_or(true);
            if !needs_lookup {
                continue;
            }
            if let Ok(mut f) = fs::File::open(&path) {
                let mut hasher = Sha1::new();
                let _ = std::io::copy(&mut f, &mut hasher);
                let hash = format!("{:x}", hasher.finalize());
                hash_to_file.insert(hash.clone(), clean_name);
                hashes.push(hash);
            }
        }
    }

    if hashes.is_empty() { return Ok(()); }

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    let label = match folder { "resourcepacks" => "ресурспаков", "shaderpacks" => "шейдеров", _ => "модов" };
    if !opts.silent {
        emit_download_progress(app, DownloadProgress {
            task_name: format!("Загрузка метаданных {}...", label),
            downloaded: 0,
            total: hashes.len(),
            instance_id: Some(instance_id.to_string()),
            ..Default::default()
        });
    }

    let client = Client::builder().user_agent("JentleMemes/1.0").build()?;
    let payload = serde_json::json!({ "hashes": hashes, "algorithm": "sha1" });
    let res = client.post("https://api.modrinth.com/v2/version_files")
        .json(&payload).send().await;

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    if let Ok(resp) = res {
        if let Ok(data) = resp.json::<HashMap<String, serde_json::Value>>().await {
            for (hash, version_data) in &data {
                if meta_aborted(opts.cancel_after_epoch) {
                    return Ok(());
                }
                if let Some(fname) = hash_to_file.get(hash) {
                    let pid = version_data["project_id"].as_str().unwrap_or("");
                    let vid = version_data["id"].as_str().unwrap_or("");
                    let ver_name = version_data["version_number"].as_str().unwrap_or("").to_string();

                    let mut title = fname.clone();
                    let mut icon_url = String::new();
                    if !pid.is_empty() {
                        let p_url = format!("https://api.modrinth.com/v2/project/{}", pid);
                        if let Ok(p_res) = client.get(&p_url).send().await {
                            if let Ok(p_data) = p_res.json::<serde_json::Value>().await {
                                title = p_data["title"].as_str().unwrap_or(fname).to_string();
                                icon_url = p_data["icon_url"].as_str().unwrap_or("").to_string();
                            }
                        }
                    }

                    meta_map.insert(fname.clone(), ModMeta {
                        project_id: pid.to_string(),
                        version_id: vid.to_string(),
                        title, icon_url, version_name: ver_name,
                    });
                }
            }
        }
    }

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    fs::write(&meta_path, serde_json::to_string_pretty(&meta_map)?)?;
    if !opts.silent {
        emit_download_progress(app, DownloadProgress {
            task_name: "Метаданные загружены".into(),
            downloaded: 1,
            total: 1,
            instance_id: Some(instance_id.to_string()),
            ..Default::default()
        });
    }
    Ok(())
}

/// Удаляет сохранённые *.json меты контента (перед полной пересборкой с API).
pub fn clear_stored_content_meta(instance_id: &str) -> Result<()> {
    for folder in ["mods", "resourcepacks", "shaderpacks"] {
        let p = meta_path_for(instance_id, folder);
        if p.exists() {
            fs::remove_file(p)?;
        }
    }
    Ok(())
}

/// После JentlePack (с прогрессом в UI). Сейчас везде используется фоновая [`verify_jentlepack_metadata_background`].
#[allow(dead_code)]
pub async fn verify_jentlepack_metadata_against_apis(app: &AppHandle, instance_id: &str) -> Result<()> {
    let opts = ContentMetaOpts {
        silent: false,
        cancel_after_epoch: None,
    };
    verify_jentlepack_with_opts(app, instance_id, opts).await
}

/// Фоновая проверка меты JentlePack (без UI, с отменой при видимом таске).
pub async fn verify_jentlepack_metadata_background(
    app: &AppHandle,
    instance_id: &str,
    start_epoch: u64,
) -> Result<()> {
    let opts = ContentMetaOpts {
        silent: true,
        cancel_after_epoch: Some(start_epoch),
    };
    verify_jentlepack_with_opts(app, instance_id, opts).await
}

async fn verify_jentlepack_with_opts(
    app: &AppHandle,
    instance_id: &str,
    opts: ContentMetaOpts,
) -> Result<()> {
    for folder in &["mods", "resourcepacks", "shaderpacks"] {
        if meta_aborted(opts.cancel_after_epoch) {
            return Ok(());
        }
        verify_folder_metadata_from_modrinth(app, instance_id, folder, opts).await?;
    }
    Ok(())
}

async fn verify_folder_metadata_from_modrinth(
    app: &AppHandle,
    instance_id: &str,
    folder: &str,
    opts: ContentMetaOpts,
) -> Result<()> {
    let content_dir = get_data_dir().join("instances").join(instance_id).join(folder);
    if !content_dir.exists() {
        return Ok(());
    }

    let meta_path = meta_path_for(instance_id, folder);
    let mut meta_map: HashMap<String, ModMeta> = HashMap::new();
    if meta_path.exists() {
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(m) = serde_json::from_str(&content) {
                meta_map = m;
            }
        }
    }

    let valid_ext: &[&str] = match folder {
        "resourcepacks" | "shaderpacks" => &[".zip", ".jar"],
        _ => &[".jar"],
    };

    let mut hashes = Vec::new();
    let mut hash_to_file: HashMap<String, String> = HashMap::new();

    if let Ok(entries) = fs::read_dir(&content_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let fname = entry.file_name().to_string_lossy().to_string();
            let is_valid = path.is_file()
                && valid_ext
                    .iter()
                    .any(|ext| fname.ends_with(ext) || fname.ends_with(&format!("{}.disabled", ext)));
            if !is_valid {
                continue;
            }
            let clean_name = fname.replace(".disabled", "");
            if let Ok(mut f) = fs::File::open(&path) {
                let mut hasher = Sha1::new();
                let _ = std::io::copy(&mut f, &mut hasher);
                let hash = format!("{:x}", hasher.finalize());
                hash_to_file.insert(hash.clone(), clean_name);
                hashes.push(hash);
            }
        }
    }

    if hashes.is_empty() {
        return Ok(());
    }

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    if !opts.silent {
        emit_download_progress(app, DownloadProgress {
            task_name: "Проверка меты (JentlePack)…".into(),
            downloaded: 0,
            total: hashes.len(),
            instance_id: Some(instance_id.to_string()),
            ..Default::default()
        });
    }

    let client = Client::builder().user_agent("JentleMemes/1.0").build()?;
    let payload = serde_json::json!({ "hashes": hashes, "algorithm": "sha1" });
    let res = client
        .post("https://api.modrinth.com/v2/version_files")
        .json(&payload)
        .send()
        .await;

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    if let Ok(resp) = res {
        if let Ok(data) = resp.json::<HashMap<String, serde_json::Value>>().await {
            for (hash, version_data) in &data {
                if meta_aborted(opts.cancel_after_epoch) {
                    return Ok(());
                }
                if let Some(fname) = hash_to_file.get(hash) {
                    let pid = version_data["project_id"].as_str().unwrap_or("");
                    let vid = version_data["id"].as_str().unwrap_or("");
                    let ver_name = version_data["version_number"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();

                    let mut title = fname.clone();
                    let mut icon_url = String::new();
                    if !pid.is_empty() {
                        let p_url = format!("https://api.modrinth.com/v2/project/{}", pid);
                        if let Ok(p_res) = client.get(&p_url).send().await {
                            if let Ok(p_data) = p_res.json::<serde_json::Value>().await {
                                title = p_data["title"].as_str().unwrap_or(fname).to_string();
                                icon_url = p_data["icon_url"].as_str().unwrap_or("").to_string();
                            }
                        }
                    }

                    meta_map.insert(
                        fname.clone(),
                        ModMeta {
                            project_id: pid.to_string(),
                            version_id: vid.to_string(),
                            title,
                            icon_url,
                            version_name: ver_name,
                        },
                    );
                }
            }
        }
    }

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    fs::write(&meta_path, serde_json::to_string_pretty(&meta_map)?)?;
    Ok(())
}