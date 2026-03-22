//! Сборка modrinth.index.json при экспорте: Modrinth по SHA1, для JentlePack — CurseForge по fingerprint, иначе файл в overrides.

use crate::core::progress_emit::emit_download_progress;
use crate::core::types::DownloadProgress;
use crate::error::{Error, Result};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use walkdir::WalkDir;

const MODRINTH_VERSION_FILES: &str = "https://api.modrinth.com/v2/version_files";

struct PendingFile {
    rel_path: String,
    abs_path: PathBuf,
    sha1_hex: String,
    bytes: Vec<u8>,
}

fn hash_file(path: &Path) -> Result<(String, Vec<u8>)> {
    let bytes = fs::read(path)?;
    let sha1_hex = format!("{:x}", Sha1::digest(&bytes));
    Ok((sha1_hex, bytes))
}

fn modrinth_file_entry(version: &Value, sha1_hex: &str, pack_path: &str) -> Option<Value> {
    let files = version.get("files")?.as_array()?;
    for f in files {
        let h = f.get("hashes")?.get("sha1")?.as_str()?;
        if h.eq_ignore_ascii_case(sha1_hex) {
            let url = f.get("url")?.as_str()?;
            let sha512 = f
                .get("hashes")
                .and_then(|x| x.get("sha512"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let size = f.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
            return Some(json!({
                "path": pack_path,
                "hashes": { "sha1": sha1_hex, "sha512": sha512 },
                "env": { "client": "required", "server": "required" },
                "downloads": [url],
                "fileSize": size
            }));
        }
    }
    None
}

async fn modrinth_lookup_hashes(
    client: &reqwest::Client,
    hashes: &[String],
) -> Result<HashMap<String, Value>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let body = json!({ "hashes": hashes, "algorithm": "sha1" });
    let res = client
        .post(MODRINTH_VERSION_FILES)
        .json(&body)
        .send()
        .await
        .map_err(|e| Error::Custom(format!("Modrinth version_files: {}", e)))?;
    if !res.status().is_success() {
        return Ok(HashMap::new());
    }
    let map: HashMap<String, Value> = res
        .json()
        .await
        .map_err(|e| Error::Custom(format!("Modrinth JSON: {}", e)))?;
    Ok(map)
}

fn cf_file_to_index_entry(file: &Value, pack_path: &str) -> Option<Value> {
    let url = file.get("downloadUrl")?.as_str()?;
    let sha1 = crate::core::curseforge::cf_file_sha1_hex(file)?;
    let size = file.get("fileLength").and_then(|x| x.as_u64()).unwrap_or(0);
    let sha512 = crate::core::curseforge::cf_file_sha512_hex(file).unwrap_or_default();
    Some(json!({
        "path": pack_path,
        "hashes": { "sha1": sha1, "sha512": sha512 },
        "env": { "client": "required", "server": "required" },
        "downloads": [url],
        "fileSize": size
    }))
}

fn collect_pending_indexable(inst_dir: &Path, selected: &[String]) -> Result<Vec<PendingFile>> {
    let mut out = Vec::new();
    for folder in selected {
        if !matches!(folder.as_str(), "mods" | "resourcepacks" | "shaderpacks") {
            continue;
        }
        let root = inst_dir.join(folder);
        if !root.exists() {
            continue;
        }
        let valid_ext: &[&str] = match folder.as_str() {
            "resourcepacks" | "shaderpacks" => &[".zip", ".jar"],
            _ => &[".jar"],
        };
        for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let fname = entry.file_name().to_string_lossy().to_string();
            let is_valid = valid_ext.iter().any(|ext| {
                fname.ends_with(ext) || fname.ends_with(&format!("{}.disabled", ext))
            });
            if !is_valid {
                continue;
            }
            let rel = path
                .strip_prefix(inst_dir)
                .map_err(|e| Error::Custom(e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            let (sha1_hex, bytes) = hash_file(path)?;
            out.push(PendingFile {
                rel_path: rel,
                abs_path: path.to_path_buf(),
                sha1_hex,
                bytes,
            });
        }
    }
    Ok(out)
}

fn collect_override_folder(inst_dir: &Path, folder: &str) -> Result<Vec<(String, PathBuf)>> {
    let mut v = Vec::new();
    let root = inst_dir.join(folder);
    if !root.exists() {
        return Ok(v);
    }
    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = path.strip_prefix(inst_dir).unwrap().to_string_lossy().replace('\\', "/");
        if path.is_file() {
            v.push((name, path.to_path_buf()));
        } else if path.is_dir() {
            let n = name.trim_end_matches('/');
            if !n.is_empty() {
                // zip directories added separately when writing
            }
        }
    }
    Ok(v)
}

/// dependencies + name для modrinth.index.json
fn index_skeleton(inst_dir: &Path, id: &str) -> Result<Value> {
    let config_path = inst_dir.join("instance.json");
    let config: Value = if config_path.exists() {
        serde_json::from_str(&fs::read_to_string(&config_path)?)?
    } else {
        json!({})
    };
    let game_version = config
        .get("game_version")
        .and_then(|v| v.as_str())
        .unwrap_or("1.20.1");
    let loader = config.get("loader").and_then(|v| v.as_str()).unwrap_or("vanilla");
    let loader_version = config
        .get("loader_version")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut deps = serde_json::Map::new();
    deps.insert(
        "minecraft".to_string(),
        Value::String(game_version.to_string()),
    );
    if loader != "vanilla" && !loader_version.is_empty() {
        deps.insert(loader.to_string(), Value::String(loader_version.to_string()));
    }

    Ok(json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0.0",
        "name": config.get("name").and_then(|v| v.as_str()).unwrap_or(id),
        "files": [],
        "dependencies": deps
    }))
}

/// `curseforge_fallback`: только для .jentlepack
pub async fn build_export_index_and_overrides(
    app: &AppHandle,
    inst_dir: &Path,
    selected_folders: &[String],
    pack_id: &str,
    curseforge_fallback: bool,
) -> Result<(Value, Vec<(String, PathBuf)>)> {
    let client = reqwest::Client::builder()
        .user_agent("JentleMemesLauncher/1.0")
        .build()
        .map_err(|e| Error::Custom(e.to_string()))?;

    let pending = collect_pending_indexable(inst_dir, selected_folders)?;
    let total = pending.len().max(1);

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: "Экспорт: поиск модов на Modrinth…".into(),
            downloaded: 0,
            total,
            instance_id: None,
            ..Default::default()
        },
    );

    let mut index_files: Vec<Value> = Vec::new();
    let mut override_entries: Vec<(String, PathBuf)> = Vec::new();

    // Пакетами по 64 SHA1 на Modrinth
    let mut resolved: HashMap<String, Value> = HashMap::new();
    for chunk in pending.chunks(64) {
        let hashes: Vec<String> = chunk.iter().map(|p| p.sha1_hex.clone()).collect();
        let batch = modrinth_lookup_hashes(&client, &hashes).await?;
        for (k, v) in batch {
            resolved.insert(k.to_lowercase(), v);
        }
    }

    let mut need_cf: Vec<(usize, u64)> = Vec::new();

    for (i, p) in pending.iter().enumerate() {
        let mut done = false;
        if let Some(ver) = resolved.get(&p.sha1_hex.to_lowercase()) {
            if let Some(entry) = modrinth_file_entry(ver, &p.sha1_hex, &p.rel_path) {
                index_files.push(entry);
                done = true;
            }
        }
        if done {
            continue;
        }
        if curseforge_fallback {
            need_cf.push((i, crate::core::curseforge::cf_fingerprint_bytes(&p.bytes)));
        } else {
            override_entries.push((p.rel_path.clone(), p.abs_path.clone()));
        }
    }

    if !need_cf.is_empty() {
        emit_download_progress(
            app,
            DownloadProgress {
                task_name: "Экспорт: поиск на CurseForge…".into(),
                downloaded: pending.len() / 2,
                total,
                instance_id: None,
                ..Default::default()
            },
        );
        let fps: Vec<u64> = need_cf.iter().map(|(_, fp)| *fp).collect();
        let data = crate::core::curseforge::match_fingerprints(fps).await?;
        let matches = data
            .get("exactMatches")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();

        let mut fp_to_file: HashMap<u64, Value> = HashMap::new();
        for m in matches {
            if let Some(file) = m.get("file").cloned() {
                let fp = file
                    .get("fileFingerprint")
                    .and_then(|x| x.as_u64())
                    .or_else(|| file.get("fileFingerprint").and_then(|x| x.as_i64()).map(|x| x as u64))
                    .unwrap_or(0);
                if fp != 0 {
                    fp_to_file.insert(fp, file);
                }
            }
        }

        for (idx, fp) in need_cf {
            let p = &pending[idx];
            if let Some(cf_file) = fp_to_file.get(&fp) {
                if let Some(entry) = cf_file_to_index_entry(cf_file, &p.rel_path) {
                    index_files.push(entry);
                    continue;
                }
            }
            override_entries.push((p.rel_path.clone(), p.abs_path.clone()));
        }
    }

    let mut index = index_skeleton(inst_dir, pack_id)?;
    index["files"] = json!(index_files);

    // Остальные выбранные папки — целиком в overrides
    for folder in selected_folders {
        if matches!(folder.as_str(), "mods" | "resourcepacks" | "shaderpacks") {
            continue;
        }
        override_entries.extend(collect_override_folder(inst_dir, folder)?);
    }

    // instance.json и .icon.* в overrides (если не в selected как отдельная логика)
    let ij = inst_dir.join("instance.json");
    if ij.exists() {
        override_entries.push(("instance.json".into(), ij));
    }
    for ext in ["png", "jpg", "webp"] {
        let ic = inst_dir.join(format!(".icon.{}", ext));
        if ic.exists() {
            override_entries.push((format!(".icon.{}", ext), ic));
        }
    }

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: "Экспорт пака: готово".into(),
            downloaded: total,
            total,
            instance_id: None,
            ..Default::default()
        },
    );

    Ok((index, override_entries))
}
