//! Risugami ModLoader: вшитый `data/modloader.json` + слияние zip из каталога данных (без Prism / без Mojang-манифеста для списка версий).

use crate::core::loader_meta::modloader_jar_merge;
use crate::core::loader_meta::version_sort;
use crate::core::utils::download::download_file;
use crate::error::{Error, Result};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use zip::read::ZipArchive;

const EMBEDDED_MANIFEST: &str = include_str!("data/modloader.json");

#[derive(Debug, Deserialize)]
struct Manifest {
    entries: Vec<ManifestEntry>,
    #[serde(rename = "zipNamePattern")]
    zip_name_pattern: String,
    #[serde(rename = "patchDir")]
    patch_dir: String,
}

#[derive(Debug, Deserialize)]
struct ManifestEntry {
    minecraft: String,
    builds: Vec<String>,
    /// Необязательные зеркала для автозагрузки zip/jar (классический ModLoader в корне архива).
    #[serde(rename = "zipDownloadUrls", default)]
    zip_download_urls: HashMap<String, Vec<String>>,
}

static MANIFEST: Lazy<Manifest> =
    Lazy::new(|| serde_json::from_str(EMBEDDED_MANIFEST).expect("modloader.json must parse"));

fn sanitize_seg(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '<' | '>' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect()
}

fn zip_file_name(mc: &str, build: &str) -> String {
    MANIFEST
        .zip_name_pattern
        .replace("{minecraft}", &sanitize_seg(mc))
        .replace("{build}", &sanitize_seg(build))
}

/// Каталог `<data>/modloader_patches/` — сюда кладут zip для слияния в client.jar.
pub fn modloader_patch_zip_path(data_dir: &Path, mc: &str, build: &str) -> PathBuf {
    data_dir
        .join(MANIFEST.patch_dir.trim().trim_matches('/'))
        .join(zip_file_name(mc, build))
}

pub fn is_known_build(mc: &str, build: &str) -> bool {
    if build == "manual" {
        return true;
    }
    MANIFEST
        .entries
        .iter()
        .any(|e| e.minecraft == mc && e.builds.iter().any(|b| b == build))
}

pub fn minecraft_versions(include_snapshots: bool) -> Result<Vec<String>> {
    let mut set = BTreeSet::new();
    for e in &MANIFEST.entries {
        set.insert(e.minecraft.clone());
    }
    let mut out: Vec<String> = set.into_iter().collect();
    version_sort::sort_mc_versions_desc(&mut out);
    let _ = include_snapshots;
    Ok(out)
}

pub fn loader_versions_for_minecraft(game_version: &str) -> Result<Vec<String>> {
    let gv = game_version.trim();
    if gv.is_empty() {
        return Ok(vec![]);
    }
    for e in &MANIFEST.entries {
        if e.minecraft == gv {
            return Ok(e.builds.clone());
        }
    }
    Ok(vec![])
}

fn zip_download_urls_for(mc: &str, build: &str) -> Option<&'static [String]> {
    for e in &MANIFEST.entries {
        if e.minecraft != mc {
            continue;
        }
        let urls = e.zip_download_urls.get(build)?;
        if urls.is_empty() {
            return None;
        }
        return Some(urls.as_slice());
    }
    None
}

fn validate_overlay_zip_or_jar(bytes: &[u8]) -> Result<()> {
    let cursor = Cursor::new(bytes);
    let archive = ZipArchive::new(cursor).map_err(|e| {
        Error::Custom(format!(
            "ModLoader: скачанный файл не является корректным zip/jar: {e}"
        ))
    })?;
    if archive.is_empty() {
        return Err(Error::Custom(
            "ModLoader: скачанный архив пуст (ожидались .class в корне).".into(),
        ));
    }
    Ok(())
}

async fn load_modloader_overlay_bytes(data_dir: &Path, mc: &str, build: &str) -> Result<Vec<u8>> {
    let zip_path = modloader_patch_zip_path(data_dir, mc, build);
    if zip_path.is_file() {
        return fs::read(&zip_path).map_err(|e| Error::Custom(e.to_string()));
    }
    let Some(urls) = zip_download_urls_for(mc, build) else {
        return Err(Error::Custom(format!(
            "ModLoader: нет zip для слияния в client.jar:\n  {}\n\
             Ожидается архив в стиле классического ModLoader (файлы в корне → внутрь client.jar).\n\
             Либо добавьте зеркала «zipDownloadUrls» в манифест для этой пары версия+сборка.",
            zip_path.display()
        )));
    };

    let mut last_err: Option<String> = None;
    for url in urls {
        match download_file(url, None).await {
            Ok(bytes) => {
                let slice = bytes.as_ref();
                if let Err(e) = validate_overlay_zip_or_jar(slice) {
                    last_err = Some(format!("{url}: {e}"));
                    continue;
                }
                if let Some(parent) = zip_path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| Error::Custom(e.to_string()))?;
                }
                tokio::fs::write(&zip_path, slice)
                    .await
                    .map_err(|e| Error::Custom(e.to_string()))?;
                eprintln!(
                    "[JentleMemes] ModLoader: патч сохранён в «{}» (скачано).",
                    zip_path.display()
                );
                return Ok(slice.to_vec());
            }
            Err(e) => {
                last_err = Some(format!("{url}: {e}"));
            }
        }
    }

    Err(Error::Custom(format!(
        "ModLoader: нет локального zip и не удалось скачать патч для «{mc}» / «{build}».\n\
         Ожидаемый путь: {}\n\
         {}",
        zip_path.display(),
        last_err.unwrap_or_else(|| "нет доступных URL.".into())
    )))
}

/// После скачивания vanilla client.jar: вшить ModLoader из zip, если сборка не `manual`.
pub async fn apply_modloader_merge_to_jar(
    data_dir: &Path,
    version_id: &str,
    jar_path: &Path,
) -> Result<()> {
    if !version_id.starts_with("modloader/") {
        return Ok(());
    }
    let parts: Vec<&str> = version_id.split('/').collect();
    if parts.len() != 3 {
        return Ok(());
    }
    let mc = parts[1].trim();
    let build = parts[2].trim();
    if build == "manual" {
        eprintln!(
            "[JentleMemes] ModLoader: «manual» — client.jar не изменяется. Для авто-слияния выберите сборку v4 (или другую из манифеста) и положите zip «{}» в «{}/{}».",
            zip_file_name(mc, "v4"),
            data_dir.display(),
            MANIFEST.patch_dir.trim().trim_matches('/')
        );
        return Ok(());
    }
    if !is_known_build(mc, build) {
        return Err(Error::Custom(format!(
            "ModLoader: неизвестная пара Minecraft «{mc}» + сборка «{build}» для вшитого манифеста."
        )));
    }
    let zip_path = modloader_patch_zip_path(data_dir, mc, build);
    let bytes = load_modloader_overlay_bytes(data_dir, mc, build).await?;
    modloader_jar_merge::merge_zip_into_jar(jar_path, &bytes)?;
    eprintln!(
        "[JentleMemes] ModLoader: в «{}» влит «{}».",
        jar_path.display(),
        zip_path.display()
    );
    Ok(())
}
