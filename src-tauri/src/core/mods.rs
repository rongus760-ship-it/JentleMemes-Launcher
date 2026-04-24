use crate::config::get_data_dir;
use crate::core::api::http_client;
use crate::core::progress_emit::emit_download_progress;
use crate::core::task_signals;
use crate::core::types::DownloadProgress;
use crate::error::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs;
use tauri::AppHandle;

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
    get_installed_from_folder(instance_id, "mods", true)
}

fn meta_path_for(instance_id: &str, folder: &str) -> std::path::PathBuf {
    let data_dir = get_data_dir()
        .join("instances")
        .join(instance_id)
        .join(".data");
    let _ = fs::create_dir_all(&data_dir);
    data_dir.join(format!("{}_meta.json", folder))
}

/// `include_hashes`: false — быстрый список для UI (без чтения всего файла для SHA1).
pub fn get_installed_from_folder(
    instance_id: &str,
    folder: &str,
    include_hashes: bool,
) -> Result<Vec<ModInfo>> {
    let content_dir = get_data_dir()
        .join("instances")
        .join(instance_id)
        .join(folder);
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
        if let Ok(m) = serde_json::from_str(&content) {
            meta_map = m;
        }
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
                let is_valid = valid_ext.iter().any(|ext| {
                    filename.ends_with(ext) || filename.ends_with(&format!("{}.disabled", ext))
                });
                if is_valid {
                    let enabled = !filename.ends_with(".disabled");
                    let clean_name = filename.replace(".disabled", "");
                    let hash = if include_hashes {
                        let mut file = fs::File::open(&path)?;
                        let mut hasher = Sha1::new();
                        std::io::copy(&mut file, &mut hasher)?;
                        format!("{:x}", hasher.finalize())
                    } else {
                        String::new()
                    };
                    let meta = meta_map.get(&clean_name);
                    mods.push(ModInfo {
                        filename,
                        clean_name: clean_name.clone(),
                        enabled,
                        hash,
                        title: meta
                            .map(|m| m.title.clone())
                            .unwrap_or_else(|| clean_name.replace(".jar", "")),
                        icon_url: meta.map(|m| m.icon_url.clone()).unwrap_or_default(),
                        version_name: meta
                            .map(|m| m.version_name.clone())
                            .unwrap_or_else(|| "Неизвестно".into()),
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
    let dir = get_data_dir()
        .join("instances")
        .join(instance_id)
        .join(folder);
    let old_path = dir.join(filename);
    let new_filename = if enable {
        filename.replace(".disabled", "")
    } else {
        if filename.ends_with(".disabled") {
            filename.to_string()
        } else {
            format!("{}.disabled", filename)
        }
    };
    let new_path = dir.join(&new_filename);
    if old_path.exists() && old_path != new_path {
        fs::rename(old_path, new_path)?;
    }
    Ok(())
}

pub async fn delete(instance_id: &str, folder: &str, filename: &str) -> Result<()> {
    let path = get_data_dir()
        .join("instances")
        .join(instance_id)
        .join(folder)
        .join(filename);
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

pub async fn check_updates(
    hashes: &[String],
    loader: &str,
    game_version: &str,
) -> Result<serde_json::Value> {
    let client = http_client();
    let payload = serde_json::json!({
        "hashes": hashes,
        "algorithm": "sha1",
        "loaders": [loader],
        "game_versions": [game_version]
    });
    let res = client
        .post("https://api.modrinth.com/v2/version_files/update")
        .json(&payload)
        .send()
        .await?;
    Ok(res.json().await?)
}

pub async fn install_with_dependencies(
    app: AppHandle,
    instance_id: &str,
    version_id: &str,
    game_version: &str,
    loader: &str,
    download_deps: bool,
) -> Result<String> {
    let mods_dir = get_data_dir()
        .join("instances")
        .join(instance_id)
        .join("mods");
    fs::create_dir_all(&mods_dir)?;

    let client = http_client();
    let mut to_download = vec![version_id.to_string()];
    let mut meta_map: HashMap<String, ModMeta> = HashMap::new();
    let meta_path = meta_path_for(instance_id, "mods");
    if meta_path.exists() {
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(m) = serde_json::from_str(&content) {
                meta_map = m;
            }
        }
    }

    let existing_project_ids: std::collections::HashSet<String> = meta_map
        .values()
        .map(|m| m.project_id.clone())
        .filter(|id| !id.is_empty())
        .collect();

    let mut current_idx = 0;
    while current_idx < to_download.len() {
        let vid = to_download[current_idx].clone();
        current_idx += 1;
        let v_url = format!("https://api.modrinth.com/v2/version/{}", vid);
        if let Ok(res) = client.get(&v_url).send().await {
            if let Ok(v_data) = res.json::<serde_json::Value>().await {
                if let Some(files) = v_data["files"].as_array() {
                    let file = files
                        .iter()
                        .find(|f| f["primary"].as_bool().unwrap_or(false))
                        .unwrap_or(&files[0]);
                    let url = file["url"].as_str().unwrap_or("");
                    let filename = file["filename"].as_str().unwrap_or("");
                    if !url.is_empty() && !filename.is_empty() {
                        let file_path = mods_dir.join(filename);
                        // Remove old version of the same project
                        let pid = v_data["project_id"].as_str().unwrap_or("");
                        if !pid.is_empty() {
                            let old_files: Vec<String> = meta_map
                                .iter()
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
                        let dep_already_installed = !pid_before.is_empty()
                            && current_idx > 1
                            && (existing_project_ids.contains(pid_before)
                                || meta_map.values().any(|m| m.project_id == pid_before));
                        if dep_already_installed && file_path.exists() {
                            continue;
                        }
                        if !file_path.exists() {
                            emit_download_progress(
                                &app,
                                DownloadProgress {
                                    task_name: format!("Скачивание: {}", filename),
                                    downloaded: current_idx,
                                    total: to_download.len(),
                                    instance_id: Some(instance_id.to_string()),
                                    ..Default::default()
                                },
                            );
                            if let Ok(r) = client.get(url).send().await {
                                if let Ok(b) = r.bytes().await {
                                    fs::write(&file_path, b).ok();
                                }
                            }
                        }
                        let pid = v_data["project_id"].as_str().unwrap_or("");
                        let mut title = filename.to_string();
                        let mut icon_url = "".to_string();
                        if !pid.is_empty() {
                            let p_url = format!("https://api.modrinth.com/v2/project/{}", pid);
                            if let Ok(p_res) = client.get(&p_url).send().await {
                                if let Ok(p_data) = p_res.json::<serde_json::Value>().await {
                                    title =
                                        p_data["title"].as_str().unwrap_or(filename).to_string();
                                    icon_url =
                                        p_data["icon_url"].as_str().unwrap_or("").to_string();
                                }
                            }
                        }
                        meta_map.insert(
                            filename.to_string(),
                            ModMeta {
                                project_id: pid.to_string(),
                                version_id: vid.clone(),
                                title,
                                icon_url,
                                version_name: v_data["version_number"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string(),
                            },
                        );
                        if download_deps {
                            if let Some(deps) = v_data["dependencies"].as_array() {
                                for dep in deps {
                                    if dep["dependency_type"].as_str().unwrap_or("") == "required" {
                                        let dep_pid = dep["project_id"].as_str().unwrap_or("");
                                        if !dep_pid.is_empty()
                                            && existing_project_ids.contains(dep_pid)
                                        {
                                            continue;
                                        }
                                        if let Some(dep_vid) = dep["version_id"].as_str() {
                                            if !to_download.contains(&dep_vid.to_string()) {
                                                to_download.push(dep_vid.to_string());
                                            }
                                        } else if !dep_pid.is_empty() {
                                            let res_url = format!("https://api.modrinth.com/v2/project/{}/version?game_versions=[%22{}%22]&loaders=[%22{}%22]", dep_pid, game_version, loader);
                                            if let Ok(dep_res) = client.get(&res_url).send().await {
                                                if let Ok(dep_v_data) =
                                                    dep_res.json::<Vec<serde_json::Value>>().await
                                                {
                                                    if let Some(first_v) = dep_v_data.get(0) {
                                                        if let Some(resolved_vid) =
                                                            first_v["id"].as_str()
                                                        {
                                                            if !to_download
                                                                .contains(&resolved_vid.to_string())
                                                            {
                                                                to_download
                                                                    .push(resolved_vid.to_string());
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
    emit_download_progress(
        &app,
        DownloadProgress {
            task_name: "Готово".into(),
            downloaded: 1,
            total: 1,
            instance_id: Some(instance_id.to_string()),
            ..Default::default()
        },
    );
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

/// Размер чанка для POST /version_files (слишком большой список Modrinth отклоняет или обрезает).
const MODRINTH_HASH_CHUNK: usize = 64;

/// Один файл с CurseForge (мод / ресурспак / шейдер): ссылка через API, мета в `*_meta.json`.
pub async fn install_from_curseforge(
    app: &AppHandle,
    instance_id: &str,
    curseforge_project_id: &str,
    curseforge_file_id: &str,
    filename_hint: Option<&str>,
    project_type: &str,
) -> Result<String> {
    let folder = match project_type {
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        _ => "mods",
    };
    let mod_id: u64 = curseforge_project_id
        .parse()
        .map_err(|_| crate::error::Error::Custom("Некорректный CurseForge project id".into()))?;
    let file_id: u64 = curseforge_file_id
        .parse()
        .map_err(|_| crate::error::Error::Custom("Некорректный CurseForge file id".into()))?;

    let file_detail = crate::core::curseforge::get_mod_file_detail(mod_id, file_id)
        .await
        .ok();
    let mut filename = filename_hint.unwrap_or("").trim().to_string();
    if filename.is_empty() {
        filename = file_detail
            .as_ref()
            .and_then(|d| d.get("fileName"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
    }
    if filename.is_empty() {
        return Err(crate::error::Error::Custom(
            "Не удалось определить имя файла CurseForge".into(),
        ));
    }

    let download_url =
        crate::core::curseforge::fetch_mod_file_download_url(mod_id, file_id).await?;

    let target_dir = get_data_dir()
        .join("instances")
        .join(instance_id)
        .join(folder);
    fs::create_dir_all(&target_dir)?;

    let meta_path = meta_path_for(instance_id, folder);
    let mut meta_map: HashMap<String, ModMeta> = HashMap::new();
    if meta_path.exists() {
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(m) = serde_json::from_str(&content) {
                meta_map = m;
            }
        }
    }

    let pid = curseforge_project_id.to_string();
    let old_files: Vec<String> = meta_map
        .iter()
        .filter(|(_, m)| m.project_id == pid)
        .map(|(k, _)| k.clone())
        .collect();
    for old_file in old_files {
        let old_path = target_dir.join(&old_file);
        if old_path.exists() && old_file != filename {
            let _ = fs::remove_file(&old_path);
        }
        meta_map.remove(&old_file);
    }

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: format!("Скачивание: {}", filename),
            downloaded: 0,
            total: 1,
            instance_id: Some(instance_id.to_string()),
            ..Default::default()
        },
    );

    let client = http_client();
    let res = client.get(&download_url).send().await?;
    if !res.status().is_success() {
        return Err(crate::error::Error::Custom(format!(
            "Скачивание файла: HTTP {}",
            res.status()
        )));
    }
    let bytes = res.bytes().await?;
    fs::write(target_dir.join(&filename), bytes)?;

    let mut title = filename.clone();
    let mut icon_url = String::new();
    if let Ok(proj) = crate::core::curseforge::get_project(curseforge_project_id).await {
        if let Some(t) = proj.get("title").and_then(|v| v.as_str()) {
            if !t.is_empty() {
                title = t.to_string();
            }
        }
        if let Some(ic) = proj.get("icon_url").and_then(|v| v.as_str()) {
            icon_url = ic.to_string();
        }
    }

    let mut version_name = file_detail
        .as_ref()
        .and_then(|d| d.get("displayName"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if version_name.is_empty() {
        version_name = filename.clone();
    }

    meta_map.insert(
        filename.clone(),
        ModMeta {
            project_id: pid,
            version_id: curseforge_file_id.to_string(),
            title,
            icon_url,
            version_name,
        },
    );
    fs::write(&meta_path, serde_json::to_string_pretty(&meta_map)?)?;

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: format!("Скачивание: {}", filename),
            downloaded: 1,
            total: 1,
            instance_id: Some(instance_id.to_string()),
            ..Default::default()
        },
    );
    Ok(format!("Установлено в {}", folder))
}

/// Поля версии из JSON Modrinth (объект Version или обёртка с полем `version`).
fn modrinth_version_fields(version_data: &serde_json::Value) -> (String, String, String) {
    let mut pid = version_data
        .get("project_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut vid = version_data
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut ver_name = version_data
        .get("version_number")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if pid.is_empty() || vid.is_empty() || ver_name.is_empty() {
        if let Some(inner) = version_data.get("version") {
            if pid.is_empty() {
                pid = inner
                    .get("project_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            if vid.is_empty() {
                vid = inner
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            if ver_name.is_empty() {
                ver_name = inner
                    .get("version_number")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }
        }
    }
    (pid, vid, ver_name)
}

async fn modrinth_fetch_project_title_icon(
    client: &Client,
    pid: &str,
    fname: &str,
) -> (String, String) {
    if pid.is_empty() {
        return (fname.to_string(), String::new());
    }
    let p_url = format!("https://api.modrinth.com/v2/project/{}", pid);
    if let Ok(p_res) = client.get(&p_url).send().await {
        if let Ok(p_data) = p_res.json::<serde_json::Value>().await {
            let title = p_data["title"].as_str().unwrap_or(fname).to_string();
            let icon_url = p_data["icon_url"].as_str().unwrap_or("").to_string();
            return (title, icon_url);
        }
    }
    (fname.to_string(), String::new())
}

async fn modrinth_version_file_by_hash(client: &Client, hash: &str) -> Option<serde_json::Value> {
    let url = format!(
        "https://api.modrinth.com/v2/version_file/{}?algorithm=sha1",
        hash
    );
    let res = client.get(&url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let val = res.json::<serde_json::Value>().await.ok()?;
    // Часто приходит объект файла с полем `version` (id версии) без project_id на верхнем уровне.
    if val
        .get("project_id")
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return Some(val);
    }
    if let Some(vid) = val
        .get("version")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        let vurl = format!("https://api.modrinth.com/v2/version/{}", vid);
        let r2 = client.get(&vurl).send().await.ok()?;
        if r2.status().is_success() {
            return r2.json::<serde_json::Value>().await.ok();
        }
    }
    Some(val)
}

async fn apply_modrinth_hash_map(
    client: &Client,
    meta_map: &mut HashMap<String, ModMeta>,
    hash_to_file: &HashMap<String, String>,
    data: &HashMap<String, serde_json::Value>,
    cancel_after_epoch: Option<u64>,
) {
    for (hash, version_data) in data {
        if meta_aborted(cancel_after_epoch) {
            return;
        }
        let Some(fname) = hash_to_file.get(hash) else {
            continue;
        };
        let (pid, vid, ver_name) = modrinth_version_fields(version_data);
        if pid.is_empty() && vid.is_empty() {
            continue;
        }
        let (title, icon_url) = modrinth_fetch_project_title_icon(client, &pid, fname).await;
        meta_map.insert(
            fname.clone(),
            ModMeta {
                project_id: pid,
                version_id: vid,
                title,
                icon_url,
                version_name: ver_name,
            },
        );
    }
}

fn content_disk_path_for_clean_name(
    content_dir: &std::path::Path,
    clean_name: &str,
) -> Option<std::path::PathBuf> {
    let p = content_dir.join(clean_name);
    if p.is_file() {
        return Some(p);
    }
    let pd = content_dir.join(format!("{}.disabled", clean_name));
    if pd.is_file() {
        return Some(pd);
    }
    None
}

const CF_FINGERPRINT_CHUNK: usize = 40;

/// Если Modrinth не знает файл (часто у модов только на CurseForge), пробуем отпечаток CurseForge.
async fn enrich_missing_meta_via_curseforge_fingerprints(
    app: &AppHandle,
    instance_id: &str,
    folder: &str,
    content_dir: &std::path::Path,
    hash_to_file: &HashMap<String, String>,
    meta_map: &mut HashMap<String, ModMeta>,
    opts: ContentMetaOpts,
) -> Result<()> {
    let label = match folder {
        "resourcepacks" => "ресурспаков",
        "shaderpacks" => "шейдеров",
        _ => "модов",
    };
    let mut unresolved: Vec<(String, u64)> = Vec::new();
    for clean_name in hash_to_file.values() {
        let still_empty = meta_map
            .get(clean_name)
            .map(|m| m.project_id.is_empty() && m.version_id.is_empty())
            .unwrap_or(true);
        if !still_empty {
            continue;
        }
        let Some(path) = content_disk_path_for_clean_name(content_dir, clean_name) else {
            continue;
        };
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let fp = crate::core::curseforge::cf_fingerprint_bytes(&bytes);
        unresolved.push((clean_name.clone(), fp));
    }
    if unresolved.is_empty() {
        return Ok(());
    }
    let total_fp = unresolved.len();
    let mut done = 0usize;
    for chunk in unresolved.chunks(CF_FINGERPRINT_CHUNK) {
        if meta_aborted(opts.cancel_after_epoch) {
            return Ok(());
        }
        let fps: Vec<u64> = chunk.iter().map(|(_, fp)| *fp).collect();
        let data = crate::core::curseforge::match_fingerprints(fps).await?;
        let matches = data
            .get("exactMatches")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        let mut fp_to_file: HashMap<u64, serde_json::Value> = HashMap::new();
        for m in matches {
            if let Some(file) = m.get("file") {
                let fp = file
                    .get("fileFingerprint")
                    .and_then(|x| x.as_u64())
                    .or_else(|| {
                        file.get("fileFingerprint")
                            .and_then(|x| x.as_i64())
                            .map(|x| x as u64)
                    })
                    .unwrap_or(0);
                if fp != 0 {
                    fp_to_file.insert(fp, file.clone());
                }
            }
        }
        for (clean_name, fp) in chunk {
            if meta_aborted(opts.cancel_after_epoch) {
                return Ok(());
            }
            let Some(cf_file) = fp_to_file.get(fp) else {
                continue;
            };
            let mod_id = cf_file
                .get("modId")
                .and_then(|v| v.as_u64())
                .map(|x| x.to_string())
                .or_else(|| {
                    cf_file
                        .get("modId")
                        .and_then(|v| v.as_i64())
                        .map(|x| x.to_string())
                })
                .unwrap_or_default();
            let file_id = cf_file
                .get("id")
                .and_then(|v| v.as_u64())
                .map(|x| x.to_string())
                .or_else(|| {
                    cf_file
                        .get("id")
                        .and_then(|v| v.as_i64())
                        .map(|x| x.to_string())
                })
                .unwrap_or_default();
            if mod_id.is_empty() || file_id.is_empty() {
                continue;
            }
            let ver_name = cf_file
                .get("displayName")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let mut title = clean_name.clone();
            let mut icon_url = String::new();
            if let Ok(proj) = crate::core::curseforge::get_project(&mod_id).await {
                if let Some(t) = proj.get("title").and_then(|v| v.as_str()) {
                    if !t.is_empty() {
                        title = t.to_string();
                    }
                }
                if let Some(ic) = proj.get("icon_url").and_then(|v| v.as_str()) {
                    icon_url = ic.to_string();
                }
            }
            meta_map.insert(
                clean_name.clone(),
                ModMeta {
                    project_id: mod_id,
                    version_id: file_id,
                    title,
                    icon_url,
                    version_name: if ver_name.is_empty() {
                        clean_name.clone()
                    } else {
                        ver_name
                    },
                },
            );
        }
        done += chunk.len();
        if !opts.silent {
            emit_download_progress(
                app,
                DownloadProgress {
                    task_name: format!("Метаданные {label} (CurseForge)…"),
                    downloaded: done.min(total_fp),
                    total: total_fp,
                    instance_id: Some(instance_id.to_string()),
                    ..Default::default()
                },
            );
        }
    }
    Ok(())
}

async fn resolve_mod_asset_download(
    client: &Client,
    download_url: Option<String>,
    modrinth_version_id: Option<String>,
    curseforge_project_id: Option<String>,
    curseforge_file_id: Option<String>,
) -> Result<(String, String)> {
    if let Some(u) = download_url {
        let t = u.trim();
        if !t.is_empty() {
            let name = t
                .rsplit('/')
                .next()
                .unwrap_or("mod.jar")
                .split('?')
                .next()
                .unwrap_or("mod.jar")
                .to_string();
            return Ok((t.to_string(), name));
        }
    }
    if let Some(vid) = modrinth_version_id {
        let v = vid.trim();
        if !v.is_empty() {
            let vurl = format!("https://api.modrinth.com/v2/version/{}", v);
            let res = client.get(&vurl).send().await?;
            if !res.status().is_success() {
                return Err(crate::error::Error::Custom(format!(
                    "Modrinth версия: HTTP {}",
                    res.status()
                )));
            }
            let val: serde_json::Value = res.json().await?;
            let files = val["files"].as_array().ok_or_else(|| {
                crate::error::Error::Custom("Modrinth: нет файлов у версии".into())
            })?;
            let file = files
                .iter()
                .find(|f| f["primary"].as_bool().unwrap_or(false))
                .unwrap_or(&files[0]);
            let url = file["url"].as_str().unwrap_or("").to_string();
            let fname = file["filename"].as_str().unwrap_or("").to_string();
            if url.is_empty() {
                return Err(crate::error::Error::Custom(
                    "Modrinth: пустой URL файла".into(),
                ));
            }
            return Ok((url, fname));
        }
    }
    let pid = curseforge_project_id.as_deref().unwrap_or("").trim();
    let fid = curseforge_file_id.as_deref().unwrap_or("").trim();
    if !pid.is_empty() && !fid.is_empty() {
        let mid: u64 = pid.parse().map_err(|_| {
            crate::error::Error::Custom("Некорректный CurseForge project id".into())
        })?;
        let file_id: u64 = fid
            .parse()
            .map_err(|_| crate::error::Error::Custom("Некорректный CurseForge file id".into()))?;
        let url = crate::core::curseforge::fetch_mod_file_download_url(mid, file_id).await?;
        let fname = crate::core::curseforge::get_mod_file_detail(mid, file_id)
            .await
            .ok()
            .and_then(|d| {
                d.get("fileName")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "mod.jar".into());
        return Ok((url, fname));
    }
    Err(crate::error::Error::Custom(
        "Укажите ссылку, версию Modrinth или пару CurseForge project/file id".into(),
    ))
}

/// Сохранить jar/zip мода в выбранную пользователем папку (не в сборку).
pub async fn download_mod_file_to_user_folder(
    download_url: Option<String>,
    modrinth_version_id: Option<String>,
    curseforge_project_id: Option<String>,
    curseforge_file_id: Option<String>,
    filename_hint: Option<String>,
) -> Result<String> {
    let folder = rfd::FileDialog::new()
        .set_title("Выберите папку для сохранения файла")
        .pick_folder()
        .ok_or_else(|| crate::error::Error::Custom("Папка не выбрана".into()))?;

    let client = http_client();
    let (url, mut filename) = resolve_mod_asset_download(
        &client,
        download_url,
        modrinth_version_id,
        curseforge_project_id,
        curseforge_file_id,
    )
    .await?;

    if let Some(h) = filename_hint.as_deref() {
        let t = h.trim();
        if !t.is_empty() {
            filename = t.to_string();
        }
    }
    if filename.is_empty() {
        filename = "mod.jar".into();
    }

    let res = client.get(&url).send().await?;
    if !res.status().is_success() {
        return Err(crate::error::Error::Custom(format!(
            "Скачивание: HTTP {}",
            res.status()
        )));
    }
    let bytes = res.bytes().await?;
    let dest = folder.join(&filename);
    fs::write(&dest, bytes)?;
    Ok(dest.to_string_lossy().to_string())
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
pub async fn build_metadata_background(
    app: &AppHandle,
    instance_id: &str,
    start_epoch: u64,
) -> Result<()> {
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
    let content_dir = get_data_dir()
        .join("instances")
        .join(instance_id)
        .join(folder);
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
                && valid_ext.iter().any(|ext| {
                    fname.ends_with(ext) || fname.ends_with(&format!("{}.disabled", ext))
                });
            if !is_valid {
                continue;
            }
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

    if hashes.is_empty() {
        return Ok(());
    }

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    let label = match folder {
        "resourcepacks" => "ресурспаков",
        "shaderpacks" => "шейдеров",
        _ => "модов",
    };
    if !opts.silent {
        emit_download_progress(
            app,
            DownloadProgress {
                task_name: format!("Загрузка метаданных {}...", label),
                downloaded: 0,
                total: hashes.len(),
                instance_id: Some(instance_id.to_string()),
                ..Default::default()
            },
        );
    }

    let client = http_client();

    let unique_hashes: Vec<String> = hashes
        .iter()
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let mut downloaded = 0usize;
    for chunk in unique_hashes.chunks(MODRINTH_HASH_CHUNK) {
        if meta_aborted(opts.cancel_after_epoch) {
            return Ok(());
        }
        let payload = serde_json::json!({ "hashes": chunk, "algorithm": "sha1" });
        if let Ok(resp) = client
            .post("https://api.modrinth.com/v2/version_files")
            .json(&payload)
            .send()
            .await
        {
            if let Ok(data) = resp.json::<HashMap<String, serde_json::Value>>().await {
                apply_modrinth_hash_map(
                    &client,
                    &mut meta_map,
                    &hash_to_file,
                    &data,
                    opts.cancel_after_epoch,
                )
                .await;
            }
        }
        downloaded = downloaded.saturating_add(chunk.len());
        if !opts.silent {
            emit_download_progress(
                app,
                DownloadProgress {
                    task_name: format!("Загрузка метаданных {}...", label),
                    downloaded,
                    total: unique_hashes.len(),
                    instance_id: Some(instance_id.to_string()),
                    ..Default::default()
                },
            );
        }
    }

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    // Одиночный запрос по хешу — если батч ничего не вернул (редкие случаи / лимиты).
    for hash in &unique_hashes {
        if meta_aborted(opts.cancel_after_epoch) {
            return Ok(());
        }
        let Some(fname) = hash_to_file.get(hash) else {
            continue;
        };
        let still_empty = meta_map
            .get(fname)
            .map(|m| m.project_id.is_empty() && m.version_id.is_empty())
            .unwrap_or(true);
        if !still_empty {
            continue;
        }
        if let Some(v) = modrinth_version_file_by_hash(&client, hash).await {
            let mut one = HashMap::new();
            one.insert(hash.clone(), v);
            apply_modrinth_hash_map(
                &client,
                &mut meta_map,
                &hash_to_file,
                &one,
                opts.cancel_after_epoch,
            )
            .await;
        }
    }

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    enrich_missing_meta_via_curseforge_fingerprints(
        app,
        instance_id,
        folder,
        &content_dir,
        &hash_to_file,
        &mut meta_map,
        opts,
    )
    .await?;

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    fs::write(&meta_path, serde_json::to_string_pretty(&meta_map)?)?;
    if !opts.silent {
        emit_download_progress(
            app,
            DownloadProgress {
                task_name: "Метаданные загружены".into(),
                downloaded: 1,
                total: 1,
                instance_id: Some(instance_id.to_string()),
                ..Default::default()
            },
        );
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
pub async fn verify_jentlepack_metadata_against_apis(
    app: &AppHandle,
    instance_id: &str,
) -> Result<()> {
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
    let content_dir = get_data_dir()
        .join("instances")
        .join(instance_id)
        .join(folder);
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
                && valid_ext.iter().any(|ext| {
                    fname.ends_with(ext) || fname.ends_with(&format!("{}.disabled", ext))
                });
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
        emit_download_progress(
            app,
            DownloadProgress {
                task_name: "Проверка меты (JentlePack)…".into(),
                downloaded: 0,
                total: hashes.len(),
                instance_id: Some(instance_id.to_string()),
                ..Default::default()
            },
        );
    }

    let client = http_client();
    let unique_hashes: Vec<String> = hashes
        .iter()
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let mut downloaded = 0usize;
    for chunk in unique_hashes.chunks(MODRINTH_HASH_CHUNK) {
        if meta_aborted(opts.cancel_after_epoch) {
            return Ok(());
        }
        let payload = serde_json::json!({ "hashes": chunk, "algorithm": "sha1" });
        if let Ok(resp) = client
            .post("https://api.modrinth.com/v2/version_files")
            .json(&payload)
            .send()
            .await
        {
            if let Ok(data) = resp.json::<HashMap<String, serde_json::Value>>().await {
                apply_modrinth_hash_map(
                    &client,
                    &mut meta_map,
                    &hash_to_file,
                    &data,
                    opts.cancel_after_epoch,
                )
                .await;
            }
        }
        downloaded = downloaded.saturating_add(chunk.len());
        if !opts.silent {
            emit_download_progress(
                app,
                DownloadProgress {
                    task_name: "Проверка меты (JentlePack)…".into(),
                    downloaded,
                    total: unique_hashes.len(),
                    instance_id: Some(instance_id.to_string()),
                    ..Default::default()
                },
            );
        }
    }

    for hash in &unique_hashes {
        if meta_aborted(opts.cancel_after_epoch) {
            return Ok(());
        }
        let Some(fname) = hash_to_file.get(hash) else {
            continue;
        };
        let still_empty = meta_map
            .get(fname)
            .map(|m| m.project_id.is_empty() && m.version_id.is_empty())
            .unwrap_or(true);
        if !still_empty {
            continue;
        }
        if let Some(v) = modrinth_version_file_by_hash(&client, hash).await {
            let mut one = HashMap::new();
            one.insert(hash.clone(), v);
            apply_modrinth_hash_map(
                &client,
                &mut meta_map,
                &hash_to_file,
                &one,
                opts.cancel_after_epoch,
            )
            .await;
        }
    }

    if meta_aborted(opts.cancel_after_epoch) {
        return Ok(());
    }

    fs::write(&meta_path, serde_json::to_string_pretty(&meta_map)?)?;
    Ok(())
}
