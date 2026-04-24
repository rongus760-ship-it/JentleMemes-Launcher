use crate::config::{get_data_dir, save_pack_source, InstalledFile, InstanceConfig, PackSource};
use crate::core::mods::ModMeta;
use crate::core::progress_emit::emit_download_progress;
use crate::core::task_signals;
use crate::core::types::DownloadProgress;
use crate::error::{Error, Result};
use futures::StreamExt;
use reqwest::Client;
use rfd::FileDialog;
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;
use walkdir::WalkDir;
use zip::{ZipArchive, ZipWriter};

/// Semver / pre-release string shown inside .jentlepack and UI (keep in sync with tauri.conf.json).
pub const LAUNCHER_VERSION_LABEL: &str = "2.0.0";

#[derive(Debug, Serialize, Clone)]
pub struct ImportInstanceResult {
    pub message: String,
    pub instance_id: String,
}

const JENTLEPACK_MANIFEST: &str = "jentlepack.json";

fn detect_icon(inst_dir: &Path) -> String {
    for ext in ["png", "jpg", "webp"] {
        let p = inst_dir.join(format!(".icon.{}", ext));
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    String::new()
}

fn instance_config_from_modrinth_index(
    index: &serde_json::Value,
    id: &str,
    name: &str,
    icon_path: &str,
) -> InstanceConfig {
    let mc_version = index["dependencies"]["minecraft"]
        .as_str()
        .unwrap_or("1.20.1")
        .to_string();
    let mut loader = "vanilla".to_string();
    let mut loader_version = String::new();
    if let Some(v) = index["dependencies"]
        .get("fabric-loader")
        .and_then(|v| v.as_str())
    {
        loader = "fabric".to_string();
        loader_version = v.to_string();
    } else if let Some(v) = index["dependencies"]
        .get("quilt-loader")
        .and_then(|v| v.as_str())
    {
        loader = "quilt".to_string();
        loader_version = v.to_string();
    } else if let Some(v) = index["dependencies"].get("forge").and_then(|v| v.as_str()) {
        loader = "forge".to_string();
        loader_version = v.to_string();
    } else if let Some(v) = index["dependencies"]
        .get("neoforge")
        .and_then(|v| v.as_str())
    {
        loader = "neoforge".to_string();
        loader_version = v.to_string();
    }
    InstanceConfig {
        id: id.to_string(),
        name: name.to_string(),
        game_version: mc_version,
        loader,
        loader_version,
        icon: icon_path.to_string(),
        settings: None,
        playtime: 0,
    }
}

/// Скачивание файлов из modrinth.index.json (как при установке .mrpack).
pub async fn download_index_pack_files(
    app: &AppHandle,
    inst_dir: &Path,
    index: &serde_json::Value,
    progress_instance_id: Option<&str>,
) -> Result<()> {
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
    if total == 0 {
        return Ok(());
    }
    let dc = Arc::new(AtomicUsize::new(0));
    let inst_id = progress_instance_id.unwrap_or("").to_string();
    emit_download_progress(
        app,
        DownloadProgress {
            task_name: "Скачивание файлов из индекса пака".into(),
            downloaded: 0,
            total,
            instance_id: if progress_instance_id.is_some() {
                Some(inst_id.clone())
            } else {
                None
            },
            ..Default::default()
        },
    );

    let client = Client::new();
    let stream = futures::stream::iter(tasks.into_iter().map(|(url, path)| {
        let client = client.clone();
        let dc = dc.clone();
        let app = app.clone();
        let inst_id = inst_id.clone();
        async move {
            if let Some(p) = path.parent() {
                let _ = tokio::fs::create_dir_all(p).await;
            }
            if let Ok(r) = client.get(&url).send().await {
                if let Ok(b) = r.bytes().await {
                    let _ = tokio::fs::write(&path, b).await;
                }
            }
            let cur = dc.fetch_add(1, Ordering::SeqCst) + 1;
            emit_download_progress(
                &app,
                DownloadProgress {
                    task_name: "Скачивание файлов пака".into(),
                    downloaded: cur,
                    total,
                    instance_id: if progress_instance_id.is_some() {
                        Some(inst_id.clone())
                    } else {
                        None
                    },
                    ..Default::default()
                },
            );
        }
    }));
    stream.buffer_unordered(32).collect::<Vec<()>>().await;
    Ok(())
}

/// Импорт: overrides/* → в корень инстанса; иконка пака → .icon.*
/// Имя для UI и папки: `jentlepack.json` → `instance.name`, иначе `modrinth.index.json` → `name`, иначе stem файла.
fn peek_import_pack_display_name(zip_path: &Path, file_stem: &str) -> String {
    let Ok(file) = fs::File::open(zip_path) else {
        return file_stem.to_string();
    };
    let Ok(mut archive) = ZipArchive::new(file) else {
        return file_stem.to_string();
    };
    let mut jentle_name: Option<String> = None;
    let mut index_name: Option<String> = None;
    for i in 0..archive.len() {
        let Ok(mut zf) = archive.by_index(i) else {
            continue;
        };
        let ename = zf.name().to_string().replace('\\', "/");
        if ename.ends_with('/') {
            continue;
        }
        let leaf = ename.rsplit('/').next().unwrap_or(&ename);
        if leaf == JENTLEPACK_MANIFEST && jentle_name.is_none() {
            let mut s = String::new();
            if zf.read_to_string(&mut s).is_ok() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                    if let Some(n) = v
                        .get("instance")
                        .and_then(|i| i.get("name"))
                        .and_then(|x| x.as_str())
                    {
                        let t = n.trim();
                        if !t.is_empty() {
                            jentle_name = Some(t.to_string());
                        }
                    }
                }
            }
        } else if leaf == "modrinth.index.json" && index_name.is_none() {
            let mut s = String::new();
            if zf.read_to_string(&mut s).is_ok() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                    if let Some(n) = v.get("name").and_then(|x| x.as_str()) {
                        let t = n.trim();
                        if !t.is_empty() {
                            index_name = Some(t.to_string());
                        }
                    }
                }
            }
        }
    }
    jentle_name
        .or(index_name)
        .unwrap_or_else(|| file_stem.to_string())
}

/// Уникальное имя папки инстанса: sanitize(display) и при занятости `_2`, `_3`, … (как при установке .mrpack).
fn allocate_import_instance_folder_id(display_name: &str, fallback_stem: &str) -> String {
    let raw = display_name.trim();
    let base_source = if raw.is_empty() { fallback_stem } else { raw };
    let base_name = crate::core::instance::sanitize_filename(base_source);
    let mut folder_name = base_name.clone();
    let mut inst_dir = get_data_dir().join("instances").join(&folder_name);
    let mut counter = 2u32;
    while inst_dir.exists() {
        folder_name = format!("{}_{}", base_name, counter);
        inst_dir = get_data_dir().join("instances").join(&folder_name);
        counter += 1;
    }
    folder_name
}

fn sync_instance_import_display_name(inst_dir: &Path, display_name: &str) -> Result<()> {
    let json_path = inst_dir.join("instance.json");
    if !json_path.exists() {
        return Ok(());
    }
    let d = display_name.trim();
    if d.is_empty() {
        return Ok(());
    }
    let content = fs::read_to_string(&json_path)?;
    let mut conf: InstanceConfig = serde_json::from_str(&content)?;
    conf.name = d.to_string();
    fs::write(&json_path, serde_json::to_string_pretty(&conf)?)?;
    Ok(())
}

fn extract_import_archive(zip_path: &Path, out_dir: &Path) -> Result<bool> {
    let file = fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut has_jentle = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name_norm = file.name().to_string().replace('\\', "/");
        let is_dir = name_norm.ends_with('/');
        let path_part = name_norm.trim_end_matches('/').to_string();
        if path_part.is_empty() {
            continue;
        }

        let outpath: PathBuf = if path_part == "jentlepack.json" {
            has_jentle = true;
            out_dir.join("jentlepack.json")
        } else if path_part == "modrinth.index.json" {
            out_dir.join("modrinth.index.json")
        } else if path_part == "icon.png" || path_part == "icon.jpg" || path_part == "icon.webp" {
            let ext = path_part.rsplit('.').next().unwrap_or("png");
            out_dir.join(format!(".icon.{}", ext))
        } else if let Some(rel) = path_part.strip_prefix("overrides/") {
            out_dir.join(rel)
        } else {
            out_dir.join(&path_part)
        };

        if is_dir {
            fs::create_dir_all(&outpath)?;
            continue;
        }
        if let Some(p) = outpath.parent() {
            fs::create_dir_all(p)?;
        }
        let mut outfile = fs::File::create(&outpath)?;
        std::io::copy(&mut file, &mut outfile)?;
    }
    Ok(has_jentle)
}

fn ensure_instance_config_import(
    inst_dir: &Path,
    new_id: &str,
    pack_name: &str,
    _has_jentle: bool,
) -> Result<()> {
    let inst_path = inst_dir.join("instance.json");
    let idx_path = inst_dir.join("modrinth.index.json");
    if inst_path.exists() {
        patch_instance_id(inst_dir, new_id)?;
        return Ok(());
    }
    if idx_path.exists() {
        let index: serde_json::Value = serde_json::from_str(&fs::read_to_string(&idx_path)?)?;
        let icon = detect_icon(inst_dir);
        let conf = instance_config_from_modrinth_index(&index, new_id, pack_name, &icon);
        fs::write(&inst_path, serde_json::to_string_pretty(&conf)?)?;
        return Ok(());
    }
    let conf = InstanceConfig {
        id: new_id.to_string(),
        name: pack_name.to_string(),
        game_version: "1.20.1".into(),
        loader: "vanilla".into(),
        loader_version: String::new(),
        icon: detect_icon(inst_dir),
        settings: None,
        playtime: 0,
    };
    fs::write(&inst_path, serde_json::to_string_pretty(&conf)?)?;
    Ok(())
}

/// Извлекает project_id и version_id из URL Modrinth CDN
fn parse_modrinth_url(url: &str) -> Option<(String, String)> {
    // https://cdn.modrinth.com/data/PROJECT_ID/versions/VERSION_ID/...
    let rest = url
        .strip_prefix("https://cdn.modrinth.com/data/")
        .or_else(|| url.strip_prefix("http://cdn.modrinth.com/data/"))?;
    let mut parts = rest.split('/');
    let project_id = parts.next()?.to_string();
    if parts.next()? != "versions" {
        return None;
    }
    let version_id = parts.next()?.to_string();
    Some((project_id, version_id))
}

pub async fn install_from_url(
    app: AppHandle,
    url: &str,
    name: &str,
    modrinth_project_id: Option<String>,
    modrinth_version_id: Option<String>,
    curseforge_project_id: Option<String>,
    curseforge_file_id: Option<String>,
) -> Result<String> {
    let data_dir = get_data_dir();
    let temp_path = data_dir.join(format!("temp_{}.mrpack", Uuid::new_v4().simple()));
    emit_download_progress(
        &app,
        DownloadProgress {
            task_name: "Скачивание архива сборки...".into(),
            downloaded: 0,
            total: 1,
            instance_id: None,
            ..Default::default()
        },
    );

    let client = Client::new();
    let mut download_url = url.trim().to_string();
    if download_url.is_empty() {
        if let (Some(p), Some(f)) = (curseforge_project_id.as_ref(), curseforge_file_id.as_ref()) {
            if !p.is_empty() && !f.is_empty() {
                let mid: u64 = p.parse().map_err(|_| {
                    Error::Custom("Некорректный CurseForge project id для сборки".into())
                })?;
                let fid: u64 = f.parse().map_err(|_| {
                    Error::Custom("Некорректный CurseForge file id для сборки".into())
                })?;
                download_url =
                    crate::core::curseforge::fetch_mod_file_download_url(mid, fid).await?;
            }
        }
    }
    if download_url.is_empty() {
        return Err(Error::Custom("Нет URL для скачивания сборки".into()));
    }
    let bytes = client.get(&download_url).send().await?.bytes().await?;
    let pack_meta =
        if let (Some(p), Some(v)) = (modrinth_project_id.as_ref(), modrinth_version_id.as_ref()) {
            if !p.is_empty() && !v.is_empty() {
                Some(PackInstallMeta::Modrinth {
                    project_id: p.clone(),
                    version_id: v.clone(),
                })
            } else {
                None
            }
        } else {
            None
        };
    let pack_meta = pack_meta.or_else(|| {
        if let (Some(p), Some(f)) = (curseforge_project_id.as_ref(), curseforge_file_id.as_ref()) {
            if !p.is_empty() && !f.is_empty() {
                return Some(PackInstallMeta::Curseforge {
                    project_id: p.clone(),
                    file_id: f.clone(),
                });
            }
        }
        None
    });
    let pack_meta = pack_meta.or_else(|| {
        parse_modrinth_url(url).map(|(project_id, version_id)| PackInstallMeta::Modrinth {
            project_id,
            version_id,
        })
    });
    let pack_meta = match pack_meta {
        Some(m) => Some(m),
        None => {
            let sha1_hex = format!("{:x}", Sha1::digest(&bytes));
            Some(PackInstallMeta::Custom {
                pack_url: url.to_string(),
                sha1: sha1_hex,
            })
        }
    };
    fs::write(&temp_path, bytes)?;
    let res = install(
        app.clone(),
        temp_path.to_string_lossy().as_ref(),
        name,
        pack_meta,
    )
    .await;
    let _ = fs::remove_file(temp_path);
    crate::core::progress_emit::emit_install_progress_cleared(&app);
    if res.is_ok() {
        let _ = app.emit("instances_changed", ());
    }
    res
}

pub(crate) enum PackInstallMeta {
    Modrinth {
        project_id: String,
        version_id: String,
    },
    Curseforge {
        project_id: String,
        file_id: String,
    },
    Custom {
        pack_url: String,
        sha1: String,
    },
}

pub async fn install(
    app: AppHandle,
    file_path: &str,
    name: &str,
    pack_meta: Option<PackInstallMeta>,
) -> Result<String> {
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
    let mc_version = index["dependencies"]["minecraft"]
        .as_str()
        .unwrap_or("1.20.1")
        .to_string();

    let mut loader = "vanilla".to_string();
    let mut loader_version = String::new();
    if let Some(v) = index["dependencies"]
        .get("fabric-loader")
        .and_then(|v| v.as_str())
    {
        loader = "fabric".to_string();
        loader_version = v.to_string();
    } else if let Some(v) = index["dependencies"]
        .get("quilt-loader")
        .and_then(|v| v.as_str())
    {
        loader = "quilt".to_string();
        loader_version = v.to_string();
    } else if let Some(v) = index["dependencies"].get("forge").and_then(|v| v.as_str()) {
        loader = "forge".to_string();
        loader_version = v.to_string();
    } else if let Some(v) = index["dependencies"]
        .get("neoforge")
        .and_then(|v| v.as_str())
    {
        loader = "neoforge".to_string();
        loader_version = v.to_string();
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
        id: id.clone(),
        name: name.to_string(),
        game_version: mc_version,
        loader,
        loader_version,
        icon: icon_path_str,
        settings: None,
        playtime: 0,
    };
    fs::write(
        inst_dir.join("instance.json"),
        serde_json::to_string_pretty(&conf)?,
    )?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        if let Some(name) = file.enclosed_name() {
            let name_str = name.to_string_lossy().to_string();
            if name_str.starts_with("overrides") {
                let relative_path = name.strip_prefix("overrides").unwrap();
                let outpath = inst_dir.join(relative_path);
                if file.name().ends_with('/') {
                    fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(p) = outpath.parent() {
                        fs::create_dir_all(p)?;
                    }
                    let mut outfile = fs::File::create(&outpath)?;
                    std::io::copy(&mut file, &mut outfile)?;
                }
            }
        }
    }

    download_index_pack_files(&app, &inst_dir, &index, Some(&id)).await?;

    let _ = crate::core::mods::clear_stored_content_meta(&id);
    let app_meta = app.clone();
    let id_meta = id.clone();
    tauri::async_runtime::spawn(async move {
        let epoch = task_signals::current_foreground_epoch();
        if let Err(e) =
            crate::core::mods::build_metadata_background(&app_meta, &id_meta, epoch).await
        {
            eprintln!("Мета после установки пака: {}", e);
        }
    });

    // Сохраняем pack_source для проверки обновлений
    if let Some(meta) = pack_meta {
        let installed_files: Vec<InstalledFile> = index["files"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| {
                        let path = f["path"].as_str()?;
                        let sha1 = f["hashes"]
                            .get("sha1")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        Some(InstalledFile {
                            path: path.to_string(),
                            sha1: sha1.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let source = match meta {
            PackInstallMeta::Modrinth {
                project_id,
                version_id,
            } => PackSource::Modrinth {
                project_id,
                version_id,
                version_name: None,
                installed_files,
            },
            PackInstallMeta::Curseforge {
                project_id,
                file_id,
            } => PackSource::Curseforge {
                project_id,
                file_id,
                version_name: None,
                installed_files,
            },
            PackInstallMeta::Custom { pack_url, sha1 } => PackSource::Custom {
                pack_url,
                pack_id: None,
                installed_mrpack_sha1: sha1,
                installed_files,
            },
        };
        let _ = save_pack_source(&inst_dir, &source);
    }

    Ok("Сборка успешно установлена!".into())
}

pub fn export(id: &str, selected_folders: Vec<String>) -> Result<String> {
    let inst_dir = get_data_dir().join("instances").join(id);
    if !inst_dir.exists() {
        return Err(Error::Custom("Сборка не найдена".into()));
    }

    if let Some(save_path) = FileDialog::new()
        .set_file_name(&format!("{}.zip", id))
        .add_filter("ZIP Archive", &["zip"])
        .save_file()
    {
        let file = fs::File::create(save_path)?;
        let mut zip = ZipWriter::new(file);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut buffer = Vec::new();

        for entry in WalkDir::new(&inst_dir) {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .strip_prefix(&inst_dir)
                .unwrap()
                .to_string_lossy()
                .replace("\\", "/");

            let should_include =
                name == "instance.json" || selected_folders.iter().any(|f| name.starts_with(f));
            if !should_include {
                continue;
            }
            if crate::core::pack_export::export_skip_path(&name) {
                continue;
            }

            if path.is_file() {
                zip.start_file(name.clone(), options)?;
                if name == "instance.json" {
                    let raw = fs::read_to_string(path)?;
                    let sanitized =
                        crate::core::pack_export::export_instance_json_for_transfer(&raw).map_err(
                            |e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
                        )?;
                    zip.write_all(sanitized.as_bytes())?;
                } else {
                    let mut f = fs::File::open(path)?;
                    f.read_to_end(&mut buffer)?;
                    zip.write_all(&buffer)?;
                    buffer.clear();
                }
            } else if !name.is_empty() {
                zip.add_directory(name, options)?;
            }
        }
        zip.finish()?;
        return Ok("Сборка экспортирована".into());
    }
    Err(Error::Custom("Экспорт отменен".into()))
}

pub async fn export_mrpack_async(
    app: &AppHandle,
    id: &str,
    selected_folders: Vec<String>,
) -> Result<String> {
    let inst_dir = get_data_dir().join("instances").join(id);
    if !inst_dir.exists() {
        return Err(Error::Custom("Сборка не найдена".into()));
    }
    let (index, overrides) = crate::core::pack_export::build_export_index_and_overrides(
        app,
        &inst_dir,
        &selected_folders,
        id,
        false,
    )
    .await?;
    let n_indexed = index["files"].as_array().map(|a| a.len()).unwrap_or(0);
    let safe_name = crate::core::instance::sanitize_filename(id);
    let safe_clone = safe_name.clone();
    let save_path = tokio::task::spawn_blocking(move || {
        FileDialog::new()
            .set_file_name(&format!("{}.mrpack", safe_clone))
            .add_filter("Modrinth Pack", &["mrpack"])
            .save_file()
    })
    .await
    .map_err(|e| Error::Custom(format!("{}", e)))?;

    let Some(save_path) = save_path else {
        return Err(Error::Custom("Экспорт отменен".into()));
    };

    let file = fs::File::create(save_path)?;
    let mut zip = ZipWriter::new(file);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = Vec::new();

    zip.start_file("modrinth.index.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;

    for ext in ["png", "jpg", "webp"] {
        let ic = inst_dir.join(format!(".icon.{}", ext));
        if ic.exists() {
            zip.start_file(format!("icon.{}", ext), options)?;
            let mut f = fs::File::open(&ic)?;
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
            break;
        }
    }

    for (rel, path) in &overrides {
        if path.is_file() {
            let zip_name = format!("overrides/{}", rel.replace('\\', "/"));
            zip.start_file(&zip_name, options)?;
            let mut f = fs::File::open(path)?;
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        }
    }
    zip.finish()?;
    Ok(format!(
        ".mrpack: {} в индексе Modrinth, {} в overrides",
        n_indexed,
        overrides.len()
    ))
}

fn read_optional_json(path: &Path) -> Option<serde_json::Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn load_folder_meta_map(inst_dir: &Path, folder: &str) -> HashMap<String, ModMeta> {
    let p = inst_dir.join(".data").join(format!("{}_meta.json", folder));
    if !p.exists() {
        return HashMap::new();
    }
    fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn build_jentlepack_manifest(
    inst_dir: &Path,
    _id: &str,
    selected_folders: &[String],
) -> Result<serde_json::Value> {
    let config_path = inst_dir.join("instance.json");
    let instance: serde_json::Value = if config_path.exists() {
        let raw = fs::read_to_string(&config_path)?;
        let sanitized = crate::core::pack_export::export_instance_json_for_transfer(&raw)?;
        serde_json::from_str(&sanitized)?
    } else {
        serde_json::json!({})
    };

    let mut content_meta = serde_json::Map::new();
    for folder in &["mods", "resourcepacks", "shaderpacks"] {
        let map = load_folder_meta_map(inst_dir, folder);
        content_meta.insert(folder.to_string(), serde_json::to_value(&map)?);
    }

    let servers = read_optional_json(&inst_dir.join("servers.json"));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut manifest = serde_json::json!({
        "formatVersion": 2,
        "kind": "jentlepack",
        "exportedAtUnix": now,
        "launcherVersion": LAUNCHER_VERSION_LABEL,
        "instance": instance,
        "contentMeta": serde_json::Value::Object(content_meta),
        "selectedFolders": selected_folders,
    });
    if let Some(s) = servers {
        manifest["servers"] = s;
    }
    Ok(manifest)
}

fn apply_jentlepack_manifest(
    inst_dir: &Path,
    new_instance_id: &str,
    manifest: &serde_json::Value,
) -> Result<()> {
    let data_dir = inst_dir.join(".data");
    fs::create_dir_all(&data_dir)?;

    if let Some(cm) = manifest.get("contentMeta") {
        for folder in &["mods", "resourcepacks", "shaderpacks"] {
            if let Some(v) = cm.get(*folder) {
                if let Ok(map) = serde_json::from_value::<HashMap<String, ModMeta>>(v.clone()) {
                    let path = data_dir.join(format!("{}_meta.json", folder));
                    if map.is_empty() {
                        let _ = fs::remove_file(&path);
                    } else {
                        fs::write(&path, serde_json::to_string_pretty(&map)?)?;
                    }
                }
            }
        }
    }

    if let Some(ps) = manifest.get("packSource") {
        if let Ok(source) = serde_json::from_value::<PackSource>(ps.clone()) {
            save_pack_source(inst_dir, &source)?;
        }
    }

    if let Some(servers) = manifest.get("servers") {
        fs::write(
            inst_dir.join("servers.json"),
            serde_json::to_string_pretty(servers)?,
        )?;
    }
    if let Some(lw) = manifest.get("lastWorld") {
        fs::write(
            inst_dir.join("last_world.json"),
            serde_json::to_string_pretty(lw)?,
        )?;
    }

    if let Some(inst) = manifest.get("instance") {
        let mut conf: InstanceConfig = serde_json::from_value(inst.clone())?;
        conf.id = new_instance_id.to_string();
        fs::write(
            inst_dir.join("instance.json"),
            serde_json::to_string_pretty(&conf)?,
        )?;
    }
    Ok(())
}

fn patch_instance_id(inst_dir: &Path, new_id: &str) -> Result<()> {
    let json_path = inst_dir.join("instance.json");
    if !json_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&json_path)?;
    if let Ok(mut conf) = serde_json::from_str::<InstanceConfig>(&content) {
        conf.id = new_id.to_string();
        fs::write(&json_path, serde_json::to_string_pretty(&conf)?)?;
    }
    Ok(())
}

/// Пишет .jentlepack в указанный путь (без диалога). Возвращает (строка лога, число indexed, overrides).
async fn write_jentlepack_zip_to_path(
    app: &AppHandle,
    id: &str,
    selected_folders: &[String],
    save_path: &Path,
) -> Result<(String, usize, usize)> {
    let inst_dir = get_data_dir().join("instances").join(id);
    if !inst_dir.exists() {
        return Err(Error::Custom("Сборка не найдена".into()));
    }
    let mut manifest = build_jentlepack_manifest(&inst_dir, id, selected_folders)?;
    manifest["modrinthIndexed"] = serde_json::json!(true);
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;

    let (index, overrides) = crate::core::pack_export::build_export_index_and_overrides(
        app,
        &inst_dir,
        selected_folders,
        id,
        true,
    )
    .await?;
    let n_indexed = index["files"].as_array().map(|a| a.len()).unwrap_or(0);

    let file = fs::File::create(save_path)?;
    let mut zip = ZipWriter::new(file);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut buffer = Vec::new();

    zip.start_file(JENTLEPACK_MANIFEST, options)?;
    zip.write_all(&manifest_bytes)?;

    zip.start_file("modrinth.index.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&index)?.as_bytes())?;

    for ext in ["png", "jpg", "webp"] {
        let ic = inst_dir.join(format!(".icon.{}", ext));
        if ic.exists() {
            zip.start_file(format!("icon.{}", ext), options)?;
            let mut f = fs::File::open(&ic)?;
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
            break;
        }
    }

    for (rel, path) in &overrides {
        if path.is_file() {
            let zip_name = format!("overrides/{}", rel.replace('\\', "/"));
            zip.start_file(&zip_name, options)?;
            let mut f = fs::File::open(path)?;
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        }
    }
    zip.finish()?;
    let msg = format!(
        ".jentlepack: {} в индексе (Modrinth+CF), {} в overrides, мета в jentlepack.json",
        n_indexed,
        overrides.len()
    );
    Ok((msg, n_indexed, overrides.len()))
}

pub async fn export_jentlepack_async(
    app: &AppHandle,
    id: &str,
    selected_folders: Vec<String>,
) -> Result<String> {
    let safe_name = crate::core::instance::sanitize_filename(id);
    let safe_clone = safe_name.clone();
    let save_path = tokio::task::spawn_blocking(move || {
        FileDialog::new()
            .set_file_name(&format!("{}.jentlepack", safe_clone))
            .add_filter("JentlePack", &["jentlepack"])
            .save_file()
    })
    .await
    .map_err(|e| Error::Custom(format!("{}", e)))?;

    let Some(save_path) = save_path else {
        return Err(Error::Custom("Экспорт отменен".into()));
    };

    let (msg, _, _) = write_jentlepack_zip_to_path(app, id, &selected_folders, &save_path).await?;
    Ok(msg)
}

/// Экспорт .jentlepack во временный файл под `data/tmp/` (для чата / автозагрузки без диалога).
pub async fn export_jentlepack_to_temp_path(
    app: &AppHandle,
    id: &str,
    selected_folders: Vec<String>,
) -> Result<String> {
    let tmp_dir = get_data_dir().join("tmp");
    fs::create_dir_all(&tmp_dir).map_err(|e| Error::Custom(e.to_string()))?;
    let out = tmp_dir.join(format!("jm_chat_{}.jentlepack", Uuid::new_v4()));
    let (_msg, _, _) = write_jentlepack_zip_to_path(app, id, &selected_folders, &out).await?;
    out.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Custom("Путь к tmp некорректен".into()))
}

async fn import_packed_archive_from_path(
    app: &AppHandle,
    file_path: PathBuf,
) -> Result<ImportInstanceResult> {
    if !file_path.is_file() {
        return Err(Error::Custom("Файл сборки не найден".into()));
    }

    let pack_stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported")
        .to_string();

    let friendly_name = peek_import_pack_display_name(&file_path, &pack_stem);
    let id = allocate_import_instance_folder_id(&friendly_name, &pack_stem);
    let inst_dir = get_data_dir().join("instances").join(&id);
    fs::create_dir_all(&inst_dir)?;

    let zip_path = file_path.clone();
    let inst_dir_clone = inst_dir.clone();
    let has_jentle =
        tokio::task::spawn_blocking(move || extract_import_archive(&zip_path, &inst_dir_clone))
            .await
            .map_err(|e| Error::Custom(format!("Импорт: {}", e)))??;

    // Старые экспорты тащили pack_source с чужого ПК; jentlepack.json при наличии задаст источник заново.
    let _ = fs::remove_file(inst_dir.join("pack_source.json"));

    let manifest_path = inst_dir.join(JENTLEPACK_MANIFEST);
    if has_jentle {
        let manifest_str = fs::read_to_string(&manifest_path)?;
        let manifest: serde_json::Value = serde_json::from_str(&manifest_str)
            .map_err(|e| Error::Custom(format!("Некорректный jentlepack.json: {}", e)))?;
        if manifest.get("kind").and_then(|k| k.as_str()) != Some("jentlepack") {
            return Err(Error::Custom(
                "jentlepack.json: поле kind должно быть \"jentlepack\"".into(),
            ));
        }
        apply_jentlepack_manifest(&inst_dir, &id, &manifest)?;
        let _ = fs::remove_file(&manifest_path);
    }

    let idx_path = inst_dir.join("modrinth.index.json");
    if idx_path.exists() {
        let index: serde_json::Value = serde_json::from_str(&fs::read_to_string(&idx_path)?)?;
        download_index_pack_files(app, &inst_dir, &index, Some(&id)).await?;
    }

    ensure_instance_config_import(&inst_dir, &id, &friendly_name, has_jentle)?;
    sync_instance_import_display_name(&inst_dir, &friendly_name)?;

    let app_bg = app.clone();
    let id_bg = id.clone();
    let hj = has_jentle;
    tauri::async_runtime::spawn(async move {
        let epoch = task_signals::current_foreground_epoch();
        let res = if hj {
            crate::core::mods::verify_jentlepack_metadata_background(&app_bg, &id_bg, epoch).await
        } else {
            if let Err(e) = crate::core::mods::clear_stored_content_meta(&id_bg) {
                eprintln!("Сброс меты: {}", e);
            }
            crate::core::mods::build_metadata_background(&app_bg, &id_bg, epoch).await
        };
        if let Err(e) = res {
            eprintln!("Фоновая мета после импорта: {}", e);
        }
    });

    Ok(ImportInstanceResult {
        message: if has_jentle {
            "Импорт JentlePack: индекс скачан; проверка меты в фоне".into()
        } else if idx_path.exists() {
            "Импорт пака: файлы по индексу; мета в фоне".into()
        } else {
            "Импорт ZIP: мета в фоне".into()
        },
        instance_id: id,
    })
}

pub async fn import_instance_packed(app: &AppHandle) -> Result<ImportInstanceResult> {
    let file_path = match FileDialog::new()
        .add_filter("Сборки", &["zip", "mrpack", "jentlepack"])
        .add_filter("ZIP Archive", &["zip"])
        .add_filter("Modrinth Pack", &["mrpack"])
        .add_filter("JentlePack", &["jentlepack"])
        .pick_file()
    {
        Some(p) => p,
        None => return Err(Error::Custom("Импорт отменен".into())),
    };
    import_packed_archive_from_path(app, file_path).await
}

pub async fn import_jentlepack_from_url(
    app: &AppHandle,
    url: String,
) -> Result<ImportInstanceResult> {
    let u = url.trim();
    if u.is_empty() {
        return Err(Error::Custom("Пустой URL".into()));
    }
    let tmp_dir = get_data_dir().join("tmp");
    fs::create_dir_all(&tmp_dir).map_err(|e| Error::Custom(e.to_string()))?;
    let tmp_file = tmp_dir.join(format!("jm_dl_{}.jentlepack", Uuid::new_v4()));
    let res = crate::core::api::http_client()
        .get(u)
        .send()
        .await
        .map_err(|e| Error::Custom(format!("Скачивание: {e}")))?;
    if !res.status().is_success() {
        return Err(Error::Custom(format!("HTTP {}", res.status())));
    }
    let bytes = res
        .bytes()
        .await
        .map_err(|e| Error::Custom(format!("Тело ответа: {e}")))?;
    fs::write(&tmp_file, bytes).map_err(|e| Error::Custom(e.to_string()))?;
    let out = import_packed_archive_from_path(app, tmp_file.clone()).await;
    let _ = fs::remove_file(&tmp_file);
    out
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
        PackSource::Modrinth {
            project_id,
            version_id,
            version_name,
            ..
        } => {
            let versions: serde_json::Value =
                crate::core::modrinth::get_versions(&project_id).await?;
            let versions_arr = versions
                .as_array()
                .ok_or_else(|| Error::Custom("Неверный ответ API".into()))?;
            let latest = versions_arr
                .first()
                .and_then(|v| v.get("id"))
                .and_then(|v| v.as_str());
            let latest_name = versions_arr
                .first()
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str());
            let latest_files = versions_arr
                .first()
                .and_then(|v| v.get("files"))
                .and_then(|v| v.as_array());
            let download_url = latest_files
                .and_then(|arr| {
                    arr.iter().find(|f| {
                        f.get("filename")
                            .and_then(|n| n.as_str())
                            .map_or(false, |n| n.ends_with(".mrpack"))
                    })
                })
                .and_then(|f| f.get("url"))
                .and_then(|u| u.as_str());

            let has_update = latest.map(|l| l != version_id).unwrap_or(false);
            Ok(serde_json::json!({
                "has_update": has_update,
                "current_version": version_name.as_deref().unwrap_or(&version_id),
                "latest_version": latest_name.unwrap_or(latest.unwrap_or("")),
                "latest_version_id": latest.unwrap_or(""),
                "source": "modrinth",
                "update_url": download_url,
            }))
        }
        PackSource::Curseforge {
            project_id,
            file_id,
            version_name,
            ..
        } => {
            let versions: serde_json::Value =
                crate::core::curseforge::get_versions(project_id.as_str()).await?;
            let versions_arr = versions
                .as_array()
                .ok_or_else(|| Error::Custom("Неверный ответ CurseForge".into()))?;
            let latest = versions_arr.first();
            let latest_id = latest.and_then(|v| v.get("id")).and_then(|v| v.as_str());
            let latest_name = latest.and_then(|v| v.get("name")).and_then(|v| v.as_str());
            let latest_files = latest
                .and_then(|v| v.get("files"))
                .and_then(|v| v.as_array());
            let download_url = latest_files
                .and_then(|arr| {
                    arr.iter().find(|f| {
                        f.get("filename")
                            .and_then(|n| n.as_str())
                            .map_or(false, |n| n.ends_with(".mrpack"))
                    })
                })
                .and_then(|f| f.get("url"))
                .and_then(|u| u.as_str());

            let has_update = latest_id.map(|l| l != file_id.as_str()).unwrap_or(false);
            Ok(serde_json::json!({
                "has_update": has_update,
                "current_version": version_name.as_deref().unwrap_or(file_id.as_str()),
                "latest_version": latest_name.unwrap_or(latest_id.unwrap_or("")),
                "latest_version_id": latest_id.unwrap_or(""),
                "source": "curseforge",
                "update_url": download_url,
            }))
        }
        PackSource::Custom {
            pack_url,
            installed_mrpack_sha1,
            ..
        } => {
            let packs = crate::custom_packs::get_cached_packs();
            let arr = packs
                .as_array()
                .or_else(|| packs.get("packs").and_then(|p| p.as_array()));
            let packs_list: Vec<&serde_json::Value> =
                arr.map(|a| a.iter().collect()).unwrap_or_default();
            let pack = packs_list.into_iter().find(|p| {
                let u = p
                    .get("url")
                    .or(p.get("mrpack_url"))
                    .or(p.get("download_url"))
                    .and_then(|v| v.as_str());
                u.map(|url| url == pack_url).unwrap_or(false)
            });

            let (remote_sha1, remote_url) = if let Some(p) = pack {
                let sha1 = p
                    .get("sha1")
                    .or(p.get("mrpack_sha1"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let url = p
                    .get("url")
                    .or(p.get("mrpack_url"))
                    .or(p.get("download_url"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                (sha1, url)
            } else {
                (String::new(), pack_url.clone())
            };

            let has_update = !remote_sha1.is_empty() && remote_sha1 != installed_mrpack_sha1;
            Ok(serde_json::json!({
                "has_update": has_update,
                "current_sha1": installed_mrpack_sha1,
                "latest_sha1": remote_sha1,
                "latest_version_id": "",
                "source": "custom",
                "update_url": if has_update { remote_url } else { String::new() },
            }))
        }
    }
}

/// Обновляет сборку (только изменённые файлы по SHA1).
/// `new_pack_version_ref` — id версии Modrinth или file_id CurseForge (если URL не парсится в Modrinth).
pub async fn update_modpack(
    app: AppHandle,
    instance_id: &str,
    update_url: &str,
    new_pack_version_ref: Option<&str>,
) -> Result<String> {
    let inst_dir = get_data_dir().join("instances").join(instance_id);
    if !inst_dir.exists() {
        return Err(Error::Custom("Инстанс не найден".into()));
    }
    let source = crate::config::load_pack_source(&inst_dir)?
        .ok_or_else(|| Error::Custom("Нет информации об источнике".into()))?;

    let old_files = match &source {
        PackSource::Modrinth {
            installed_files, ..
        }
        | PackSource::Curseforge {
            installed_files, ..
        }
        | PackSource::Custom {
            installed_files, ..
        } => installed_files
            .iter()
            .map(|f| (f.path.clone(), f.sha1.clone()))
            .collect::<std::collections::HashMap<_, _>>(),
    };

    emit_download_progress(
        &app,
        DownloadProgress {
            task_name: "Скачивание обновления...".into(),
            downloaded: 0,
            total: 1,
            instance_id: Some(instance_id.to_string()),
            ..Default::default()
        },
    );
    let bytes = Client::new().get(update_url).send().await?.bytes().await?;

    let data_dir = get_data_dir();
    let temp_path = data_dir.join(format!("temp_update_{}.mrpack", Uuid::new_v4().simple()));
    fs::write(&temp_path, bytes.as_ref())?;

    let file = fs::File::open(&temp_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut index_content = String::new();
    archive
        .by_name("modrinth.index.json")?
        .read_to_string(&mut index_content)?;
    let index: serde_json::Value = serde_json::from_str(&index_content)?;
    drop(archive);

    let new_files: Vec<(String, String, String)> = index["files"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|f| {
                    let path = f["path"].as_str()?;
                    let sha1 = f["hashes"]
                        .get("sha1")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let url = f["downloads"].as_array()?.get(0)?.as_str()?;
                    Some((path.to_string(), sha1.to_string(), url.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut to_download = Vec::new();
    let to_remove: Vec<String> = old_files
        .keys()
        .filter(|p| !new_files.iter().any(|(np, _, _)| np == *p))
        .cloned()
        .collect();

    for (path, new_sha1, url) in &new_files {
        let old_sha1 = old_files.get(path).map(|s| s.as_str()).unwrap_or("");
        if old_sha1 == *new_sha1 {
            continue;
        }
        let full = inst_dir.join(path);
        // Не затираем файл, если он уже есть, но не был в старом индексе пака (пользователь добавил).
        if old_sha1.is_empty() && full.exists() {
            continue;
        }
        // Файл из пака, но на диске другой SHA — пользователь менял вручную; не перезаписываем при смене версии пака.
        if !old_sha1.is_empty() && full.exists() {
            if let Ok(bytes) = fs::read(&full) {
                let disk_sha = format!("{:x}", Sha1::digest(&bytes));
                if disk_sha != old_sha1 {
                    continue;
                }
            }
        }
        to_download.push((path.clone(), url.clone()));
    }

    for path in &to_remove {
        let full = inst_dir.join(path);
        if full.exists() {
            let _ = fs::remove_file(&full);
        }
    }

    let total = to_download.len();
    if total > 0 {
        let dc = Arc::new(AtomicUsize::new(0));
        let inst_id = instance_id.to_string();
        emit_download_progress(
            &app,
            DownloadProgress {
                task_name: "Скачивание обновлённых модов".into(),
                downloaded: 0,
                total,
                instance_id: Some(inst_id.clone()),
                ..Default::default()
            },
        );
        let client = Client::new();
        let stream = futures::stream::iter(to_download.into_iter().map(|(path, url)| {
            let client = client.clone();
            let dc = dc.clone();
            let app = app.clone();
            let inst_dir = inst_dir.clone();
            let inst_id = inst_id.clone();
            async move {
                let full_path = inst_dir.join(&path);
                if let Some(p) = full_path.parent() {
                    let _ = tokio::fs::create_dir_all(p).await;
                }
                if let Ok(r) = client.get(&url).send().await {
                    if let Ok(b) = r.bytes().await {
                        let _ = tokio::fs::write(&full_path, b).await;
                    }
                }
                let cur = dc.fetch_add(1, Ordering::SeqCst) + 1;
                emit_download_progress(
                    &app,
                    DownloadProgress {
                        task_name: "Скачивание модов".into(),
                        downloaded: cur,
                        total,
                        instance_id: Some(inst_id.clone()),
                        ..Default::default()
                    },
                );
            }
        }));
        stream.buffer_unordered(32).collect::<Vec<()>>().await;
    }

    // Фактические SHA на диске (учёт пропущенных загрузок и правок пользователя)
    let installed_files: Vec<InstalledFile> = new_files
        .iter()
        .filter_map(|(path, _, _)| {
            let full = inst_dir.join(path);
            if !full.exists() {
                return None;
            }
            let bytes = fs::read(&full).ok()?;
            Some(InstalledFile {
                path: path.clone(),
                sha1: format!("{:x}", Sha1::digest(&bytes)),
            })
        })
        .collect();
    let new_source = match &source {
        PackSource::Modrinth { project_id, .. } => {
            let new_version_id = new_pack_version_ref
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .or_else(|| parse_modrinth_url(update_url).map(|(_, v)| v))
                .unwrap_or_default();
            PackSource::Modrinth {
                project_id: project_id.clone(),
                version_id: new_version_id,
                version_name: None,
                installed_files,
            }
        }
        PackSource::Curseforge {
            project_id,
            file_id,
            ..
        } => {
            let new_fid = new_pack_version_ref
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| file_id.clone());
            PackSource::Curseforge {
                project_id: project_id.clone(),
                file_id: new_fid,
                version_name: None,
                installed_files,
            }
        }
        PackSource::Custom {
            pack_url, pack_id, ..
        } => {
            let new_sha1 = format!("{:x}", Sha1::digest(fs::read(&temp_path)?));
            PackSource::Custom {
                pack_url: pack_url.clone(),
                pack_id: pack_id.clone(),
                installed_mrpack_sha1: new_sha1,
                installed_files,
            }
        }
    };
    let _ = fs::remove_file(temp_path);
    crate::config::save_pack_source(&inst_dir, &new_source)?;

    let app_meta = app.clone();
    let id_meta = instance_id.to_string();
    tauri::async_runtime::spawn(async move {
        let epoch = task_signals::current_foreground_epoch();
        if let Err(e) =
            crate::core::mods::build_metadata_background(&app_meta, &id_meta, epoch).await
        {
            eprintln!("Мета после обновления пака: {}", e);
        }
    });
    Ok("Сборка обновлена".into())
}
