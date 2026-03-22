use std::path::Path;
use std::process::Command as StdCommand;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

use crate::config::get_data_dir;
use crate::core::api::HTTP_CLIENT;
use crate::core::types::DownloadProgress;
use crate::error::{Result, Error};

fn get_os() -> &'static str {
    if cfg!(target_os = "linux") { "linux" }
    else if cfg!(target_os = "windows") { "windows" }
    else if cfg!(target_os = "macos") { "mac" }
    else { "linux" }
}

fn get_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") { "x64" }
    else if cfg!(target_arch = "aarch64") { "aarch64" }
    else if cfg!(target_arch = "x86") { "x32" }
    else { "x64" }
}

fn find_java_binary(dir: &Path) -> Option<String> {
    if !dir.exists() { return None; }

    let candidates = if cfg!(windows) {
        vec!["bin/java.exe", "bin/javaw.exe"]
    } else {
        vec!["bin/java"]
    };

    for candidate in &candidates {
        let bin = dir.join(candidate);
        if bin.exists() { return Some(bin.to_string_lossy().to_string()); }
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                if let Some(path) = find_java_binary(&entry.path()) {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Maps Minecraft's required Java major version to the best available Adoptium version.
/// Java 16 is EOL; Adoptium may not have it, so we fall back to 17.
fn adoptium_version(major: u32) -> Vec<u32> {
    match major {
        16 => vec![16, 17],
        v => vec![v],
    }
}

pub async fn ensure_java(app: &AppHandle, major_version: u32) -> Result<String> {
    let data_dir = get_data_dir();
    let java_base = data_dir.join("java");
    let java_dir = java_base.join(format!("java-{}", major_version));

    if let Some(bin) = find_java_binary(&java_dir) {
        return Ok(bin);
    }

    let os = get_os();
    let arch = get_arch();
    let candidates = adoptium_version(major_version);

    let mut download_url = String::new();
    let mut file_name = String::new();
    let mut file_size: u64 = 0;

    for ver in &candidates {
        let api_url = format!(
            "https://api.adoptium.net/v3/assets/latest/{}/hotspot?architecture={}&image_type=jre&os={}",
            ver, arch, os
        );

        let resp = HTTP_CLIENT.get(&api_url).send().await;
        let resp = match resp {
            Ok(r) if r.status().is_success() => r,
            _ => continue,
        };

        let items: Vec<Value> = match resp.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(pkg) = items.first()
            .and_then(|r| r.get("binary"))
            .and_then(|b| b.get("package"))
        {
            if let (Some(link), Some(name)) = (
                pkg.get("link").and_then(|v| v.as_str()),
                pkg.get("name").and_then(|v| v.as_str()),
            ) {
                download_url = link.to_string();
                file_name = name.to_string();
                file_size = pkg.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                break;
            }
        }
    }

    if download_url.is_empty() {
        return Err(Error::Custom(format!(
            "Java {} не найдена в Adoptium для {}/{}. Установите Java {} вручную.",
            major_version, os, arch, major_version
        )));
    }

    std::fs::create_dir_all(&java_base)?;
    let archive_path = java_base.join(&file_name);

    let _ = app.emit("download_progress", DownloadProgress {
        task_name: format!("Скачивание Java {}...", major_version),
        downloaded: 0,
        total: if file_size > 0 { (file_size / 1024 / 1024) as usize } else { 100 },
        instance_id: None,
    });

    // Stream download to file
    let response = HTTP_CLIENT.get(&download_url).send().await?;
    let total_bytes = response.content_length().unwrap_or(file_size);
    let mut stream = response.bytes_stream();
    let mut out_file = tokio::fs::File::create(&archive_path).await
        .map_err(|e| Error::Custom(format!("Не удалось создать файл: {}", e)))?;

    let mut downloaded_bytes: u64 = 0;
    let total_mb = (total_bytes / 1024 / 1024) as usize;

    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| Error::Custom(format!("Ошибка скачивания: {}", e)))?;
        out_file.write_all(&chunk).await
            .map_err(|e| Error::Custom(format!("Ошибка записи: {}", e)))?;
        downloaded_bytes += chunk.len() as u64;

        let dl_mb = (downloaded_bytes / 1024 / 1024) as usize;
        if dl_mb % 5 == 0 || downloaded_bytes == total_bytes {
            let _ = app.emit("download_progress", DownloadProgress {
                task_name: format!("Скачивание Java {}...", major_version),
                downloaded: dl_mb,
                total: total_mb,
                instance_id: None,
            });
        }
    }
    drop(out_file);

    let _ = app.emit("download_progress", DownloadProgress {
        task_name: format!("Распаковка Java {}...", major_version),
        downloaded: 0,
        total: 1,
        instance_id: None,
    });

    std::fs::create_dir_all(&java_dir)?;

    let extract_ok = if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        StdCommand::new("tar")
            .args(["xzf", &archive_path.to_string_lossy(), "-C", &java_dir.to_string_lossy(), "--strip-components=1"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    } else if file_name.ends_with(".zip") {
        extract_zip(&archive_path, &java_dir)
    } else {
        false
    };

    let _ = std::fs::remove_file(&archive_path);

    if !extract_ok {
        let _ = std::fs::remove_dir_all(&java_dir);
        return Err(Error::Custom("Не удалось распаковать Java".into()));
    }

    let _ = app.emit("download_progress", DownloadProgress {
        task_name: format!("Java {} готова!", major_version),
        downloaded: 1,
        total: 1,
        instance_id: None,
    });

    find_java_binary(&java_dir).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&java_dir);
        Error::Custom("Бинарник java не найден после распаковки".into())
    })
}

fn extract_zip(archive: &Path, dest: &Path) -> bool {
    let file = match std::fs::File::open(archive) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut zip = match zip::ZipArchive::new(file) {
        Ok(z) => z,
        Err(_) => return false,
    };

    // Find the common prefix (top-level directory in the zip)
    let prefix = zip.file_names()
        .next()
        .and_then(|name| name.split('/').next())
        .unwrap_or("")
        .to_string();

    for i in 0..zip.len() {
        let mut entry = match zip.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let raw_name = entry.name().to_string();
        let stripped = raw_name.strip_prefix(&format!("{}/", prefix)).unwrap_or(&raw_name);
        if stripped.is_empty() { continue; }
        let out_path = dest.join(stripped);

        if entry.is_dir() {
            let _ = std::fs::create_dir_all(&out_path);
        } else {
            if let Some(parent) = out_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut out_file) = std::fs::File::create(&out_path) {
                let _ = std::io::copy(&mut entry, &mut out_file);
            }
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bin = dest.join("bin").join("java");
        if bin.exists() {
            let _ = std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755));
        }
    }

    true
}
