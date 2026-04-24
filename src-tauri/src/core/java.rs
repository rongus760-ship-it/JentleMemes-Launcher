use serde_json::Value;
use std::path::Path;
use std::process::Command as StdCommand;
use tauri::AppHandle;

use crate::config::get_data_dir;
use crate::core::api::http_client;
use crate::core::progress_emit::emit_download_progress;
use crate::core::types::DownloadProgress;
use crate::error::{Error, Result};

fn get_os() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn get_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "x32"
    } else {
        "x64"
    }
}

pub fn find_java_binary_public(dir: &Path) -> Option<String> {
    find_java_binary(dir)
}

fn find_java_binary(dir: &Path) -> Option<String> {
    if !dir.exists() {
        return None;
    }

    let candidates = if cfg!(windows) {
        vec!["bin/java.exe", "bin/javaw.exe"]
    } else {
        vec!["bin/java"]
    };

    for candidate in &candidates {
        let bin = dir.join(candidate);
        if bin.exists() {
            return Some(bin.to_string_lossy().to_string());
        }
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
    let settings = crate::config::load_settings().unwrap_or_default();
    if let Some(sub) = settings
        .java_major_default_subdir
        .get(&major_version.to_string())
    {
        let t = sub.trim();
        if !t.is_empty() && !t.contains("..") {
            let dir = data_dir.join("java").join(t);
            if let Some(bin) = find_java_binary(&dir) {
                return Ok(bin);
            }
        }
    }
    let provider = settings.java_download_provider.to_lowercase();
    match provider.as_str() {
        "zulu" => ensure_java_zulu(app, major_version).await,
        "microsoft" => ensure_java_adoptium(app, major_version, "microsoft").await,
        _ => ensure_java_adoptium(app, major_version, "eclipse").await,
    }
}

async fn ensure_java_zulu(app: &AppHandle, major_version: u32) -> Result<String> {
    let data_dir = get_data_dir();
    let java_base = data_dir.join("java");
    let java_dir = java_base.join(format!("java-{}", major_version));

    if let Some(bin) = find_java_binary(&java_dir) {
        return Ok(bin);
    }

    let z_os = match get_os() {
        "mac" => "macos",
        o => o,
    };
    let z_arch = match get_arch() {
        "x64" => "x86_64",
        "x32" => "i686",
        "aarch64" => "aarch64",
        _ => "x86_64",
    };
    let archive = if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    };

    let meta_url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages/?java_version={}&os={}&arch={}&archive_type={}&java_package_type=jre&latest=true",
        major_version, z_os, z_arch, archive
    );

    let resp = http_client().get(&meta_url).send().await?;
    if !resp.status().is_success() {
        return Err(Error::Custom(format!(
            "Zulu API ошибка {} для Java {}",
            resp.status(),
            major_version
        )));
    }
    let items: Vec<Value> = resp
        .json()
        .await
        .map_err(|e| Error::Custom(e.to_string()))?;

    let mut download_url = String::new();
    let mut file_name = String::new();
    for item in items {
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if cfg!(target_os = "linux") && name.contains("musl") {
            continue;
        }
        if let (Some(url), Some(nm)) = (
            item.get("download_url").and_then(|v| v.as_str()),
            item.get("name").and_then(|v| v.as_str()),
        ) {
            download_url = url.to_string();
            file_name = nm.to_string();
            break;
        }
    }

    if download_url.is_empty() {
        return Err(Error::Custom(format!(
            "Java {} не найдена (Zulu) для {}/{}. Установите вручную.",
            major_version, z_os, z_arch
        )));
    }

    std::fs::create_dir_all(&java_base)?;
    let archive_path = java_base.join(&file_name);

    crate::core::utils::download::download_with_progress(
        &download_url,
        &archive_path,
        app,
        &format!("Скачивание Java {} (Zulu)...", major_version),
    )
    .await?;

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: format!("Распаковка Java {}...", major_version),
            downloaded: 0,
            total: 1,
            instance_id: None,
            ..Default::default()
        },
    );

    extract_archive_blocking(archive_path, java_dir.clone(), "Java (Zulu)").await?;

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: format!("Java {} готова!", major_version),
            downloaded: 1,
            total: 1,
            instance_id: None,
            ..Default::default()
        },
    );

    find_java_binary(&java_dir).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&java_dir);
        Error::Custom("Бинарник java не найден после распаковки".into())
    })
}

async fn ensure_java_adoptium(app: &AppHandle, major_version: u32, vendor: &str) -> Result<String> {
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

    let vendor_attempts: Vec<&str> = if vendor == "microsoft" {
        vec!["microsoft", "eclipse"]
    } else {
        vec![vendor]
    };

    'found: for ver in &candidates {
        for v in &vendor_attempts {
            let api_url = format!(
                "https://api.adoptium.net/v3/assets/latest/{}/hotspot?architecture={}&image_type=jre&os={}&vendor={}",
                ver, arch, os, v
            );

            let resp = http_client().get(&api_url).send().await;
            let resp = match resp {
                Ok(r) if r.status().is_success() => r,
                _ => continue,
            };

            let items: Vec<Value> = match resp.json().await {
                Ok(v) => v,
                Err(_) => continue,
            };

            if let Some(pkg) = items
                .first()
                .and_then(|r| r.get("binary"))
                .and_then(|b| b.get("package"))
            {
                if let (Some(link), Some(name)) = (
                    pkg.get("link").and_then(|v| v.as_str()),
                    pkg.get("name").and_then(|v| v.as_str()),
                ) {
                    download_url = link.to_string();
                    file_name = name.to_string();
                    break 'found;
                }
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

    crate::core::utils::download::download_with_progress(
        &download_url,
        &archive_path,
        app,
        &format!("Скачивание Java {}...", major_version),
    )
    .await?;

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: format!("Распаковка Java {}...", major_version),
            downloaded: 0,
            total: 1,
            instance_id: None,
            ..Default::default()
        },
    );

    extract_archive_blocking(archive_path, java_dir.clone(), "Java (Adoptium)").await?;

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: format!("Java {} готова!", major_version),
            downloaded: 1,
            total: 1,
            instance_id: None,
            ..Default::default()
        },
    );

    find_java_binary(&java_dir).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&java_dir);
        Error::Custom("Бинарник java не найден после распаковки".into())
    })
}

/// Разбор вывода `java -version` (stderr или stdout). Для `1.8.0_392` возвращает `8`.
pub fn parse_java_version_output(text: &str) -> Option<u32> {
    for line in text.lines() {
        let idx = line.find("version \"")?;
        let rest = line.get(idx + "version \"".len()..)?;
        let end = rest.find('"')?;
        let ver = &rest[..end];
        if let Some(tail) = ver.strip_prefix("1.") {
            let major_str = tail.split(|c| c == '.' || c == '_' || c == '-').next()?;
            return major_str.parse().ok();
        }
        let major_str = ver.split(|c| c == '.' || c == '-' || c == '+').next()?;
        return major_str.parse().ok();
    }
    None
}

/// Фактическая major-версия указанного бинарника `java` / `javaw`.
pub async fn detect_java_major(binary: &str) -> Option<u32> {
    let output = tokio::process::Command::new(binary)
        .arg("-version")
        .output()
        .await
        .ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_java_version_output(&stderr).or_else(|| parse_java_version_output(&stdout))
}

async fn extract_archive_blocking(
    archive_path: std::path::PathBuf,
    dest_dir: std::path::PathBuf,
    error_label: &str,
) -> Result<()> {
    let label = error_label.to_string();
    let ap = archive_path.clone();
    let dd = dest_dir.clone();
    let ok = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&dd)?;
        let file_name = ap
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let success = if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
            StdCommand::new("tar")
                .args([
                    "xzf",
                    &ap.to_string_lossy(),
                    "-C",
                    &dd.to_string_lossy(),
                    "--strip-components=1",
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else if file_name.ends_with(".zip") {
            extract_zip(&ap, &dd)
        } else {
            false
        };
        let _ = std::fs::remove_file(&ap);
        if !success {
            let _ = std::fs::remove_dir_all(&dd);
        }
        Ok::<bool, std::io::Error>(success)
    })
    .await
    .map_err(|e| Error::Custom(format!("{label}: join error: {e}")))?
    .map_err(|e| Error::Custom(format!("{label}: {e}")))?;
    if !ok {
        return Err(Error::Custom(format!("Не удалось распаковать {label}")));
    }
    Ok(())
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
    let prefix = zip
        .file_names()
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
        let stripped = raw_name
            .strip_prefix(&format!("{}/", prefix))
            .unwrap_or(&raw_name);
        if stripped.is_empty() {
            continue;
        }
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

// ---------- Выбор конкретной сборки JRE (расширенные настройки) ----------

#[derive(Debug, Clone, serde::Serialize)]
pub struct JavaBuildOption {
    pub id: String,
    pub label: String,
    pub download_url: String,
    pub archive_name: String,
}

fn sanitize_build_folder_id(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "build".into()
    } else {
        out
    }
}

fn is_allowed_java_download_url(url: &str) -> bool {
    let u = url.to_lowercase();
    u.starts_with("https://github.com/")
        || u.starts_with("https://api.adoptium.net/")
        || u.contains("cdn.azul.com")
        || u.starts_with("https://cdn.azul.com/")
}

pub async fn list_java_builds(provider: &str, major: u32) -> Result<Vec<JavaBuildOption>> {
    let p = provider.to_lowercase();
    match p.as_str() {
        "zulu" => list_zulu_build_options(major).await,
        "microsoft" => list_adoptium_feature_builds(major, "microsoft").await,
        _ => list_adoptium_feature_builds(major, "eclipse").await,
    }
}

async fn list_adoptium_feature_builds(major: u32, vendor: &str) -> Result<Vec<JavaBuildOption>> {
    let os = get_os();
    let arch = get_arch();
    let vendor_attempts: Vec<&str> = if vendor == "microsoft" {
        vec!["microsoft", "eclipse"]
    } else {
        vec![vendor]
    };
    let mut arr: Vec<Value> = Vec::new();
    for v in &vendor_attempts {
        let url = format!(
            "https://api.adoptium.net/v3/assets/feature_releases/{}/ga?architecture={}&image_type=jre&os={}&vendor={}",
            major, arch, os, v
        );
        let resp = http_client()
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Custom(format!("Adoptium API: {e}")))?;
        if !resp.status().is_success() {
            continue;
        }
        if let Ok(a) = resp.json::<Vec<Value>>().await {
            if !a.is_empty() {
                arr = a;
                break;
            }
        }
    }
    if arr.is_empty() {
        return Err(Error::Custom(
            "Adoptium API: нет сборок для этой ОС/архитектуры (попробуйте другого поставщика)."
                .into(),
        ));
    }
    let mut out: Vec<JavaBuildOption> = Vec::new();
    for rel in arr {
        let release_name = rel
            .get("release_name")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown");
        let Some(bins) = rel.get("binaries").and_then(|x| x.as_array()) else {
            continue;
        };
        for b in bins {
            if b.get("os").and_then(|x| x.as_str()) != Some(os) {
                continue;
            }
            if b.get("architecture").and_then(|x| x.as_str()) != Some(arch) {
                continue;
            }
            if b.get("image_type").and_then(|x| x.as_str()) != Some("jre") {
                continue;
            }
            let Some(pkg) = b.get("package") else {
                continue;
            };
            let Some(link) = pkg.get("link").and_then(|x| x.as_str()) else {
                continue;
            };
            let Some(name) = pkg.get("name").and_then(|x| x.as_str()) else {
                continue;
            };
            let id = sanitize_build_folder_id(&format!("{}_{}", vendor, release_name));
            let label = format!("{} ({})", release_name, vendor);
            out.push(JavaBuildOption {
                id,
                label,
                download_url: link.to_string(),
                archive_name: name.to_string(),
            });
        }
    }
    Ok(out)
}

async fn list_zulu_build_options(major: u32) -> Result<Vec<JavaBuildOption>> {
    let z_os = match get_os() {
        "mac" => "macos",
        o => o,
    };
    let z_arch = match get_arch() {
        "x64" => "x86_64",
        "x32" => "i686",
        "aarch64" => "aarch64",
        _ => "x86_64",
    };
    let archive = if cfg!(target_os = "windows") {
        "zip"
    } else {
        "tar.gz"
    };
    let url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages/?java_version={}&os={}&arch={}&archive_type={}&java_package_type=jre&page_size=80",
        major, z_os, z_arch, archive
    );
    let resp = http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| Error::Custom(format!("Zulu API: {e}")))?;
    if !resp.status().is_success() {
        return Err(Error::Custom(format!("Zulu API: HTTP {}", resp.status())));
    }
    let items: Vec<Value> = resp
        .json()
        .await
        .map_err(|e| Error::Custom(e.to_string()))?;
    let mut out: Vec<JavaBuildOption> = Vec::new();
    for item in items {
        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if cfg!(target_os = "linux") && name.contains("musl") {
            continue;
        }
        if name.contains("crac") || name.contains("-fx-") {
            continue;
        }
        let Some(download_url) = item.get("download_url").and_then(|v| v.as_str()) else {
            continue;
        };
        let id = sanitize_build_folder_id(&format!("zulu_{}", name.replace('.', "_")));
        let label = if let Some(jv) = item.get("java_version") {
            format!("Zulu {jv} — {name}")
        } else {
            format!("Zulu {name}")
        };
        out.push(JavaBuildOption {
            id,
            label,
            download_url: download_url.to_string(),
            archive_name: name.to_string(),
        });
    }
    Ok(out)
}

/// Скачивает выбранную сборку в `java/runtimes/{id}/`, возвращает путь к `java`.
pub async fn download_java_build(
    app: &AppHandle,
    build_id: &str,
    download_url: &str,
    archive_name: &str,
) -> Result<String> {
    let id = sanitize_build_folder_id(build_id);
    if id.contains("..") {
        return Err(Error::Custom("Некорректный id сборки".into()));
    }
    if !is_allowed_java_download_url(download_url) {
        return Err(Error::Custom(
            "URL скачивания не из доверенного источника".into(),
        ));
    }
    let data_dir = get_data_dir();
    let java_base = data_dir.join("java");
    let dest_dir = java_base.join("runtimes").join(&id);
    if let Some(bin) = find_java_binary(&dest_dir) {
        return Ok(bin);
    }
    std::fs::create_dir_all(&java_base).map_err(|e| Error::Custom(e.to_string()))?;
    let archive_path = java_base.join(archive_name);

    crate::core::utils::download::download_with_progress(
        download_url,
        &archive_path,
        app,
        &format!("Скачивание JRE ({})...", id),
    )
    .await?;

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: format!("Распаковка JRE ({})...", id),
            downloaded: 0,
            total: 1,
            instance_id: None,
            ..Default::default()
        },
    );
    extract_archive_blocking(archive_path, dest_dir.clone(), "JRE").await?;
    let bin = find_java_binary(&dest_dir).ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&dest_dir);
        Error::Custom("После распаковки не найден java".into())
    })?;
    emit_download_progress(
        app,
        DownloadProgress {
            task_name: format!("JRE {} готова", id),
            downloaded: 1,
            total: 1,
            instance_id: None,
            ..Default::default()
        },
    );
    Ok(bin)
}
