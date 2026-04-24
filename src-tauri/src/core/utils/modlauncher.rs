//! NeoForge / ForgeWrapper: `cpw.mods:modlauncher` на Maven. Версия `net.neoforged.fancymodloader:loader`
//! в `version.json` часто **не совпадает** с номером modlauncher (например loader `10.0.32`, а в репозитории есть только `10.0.10`).

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use crate::core::types::VersionInfo;
use crate::core::utils::download::download_file;
use crate::core::utils::maven::maven_to_path;
use crate::error::{Error, Result};

/// Maven metadata для подбора версии, если номер из `fancymodloader:loader` не опубликован как modlauncher.
pub const MODLAUNCHER_METADATA_URL: &str =
    "https://maven.neoforged.net/releases/cpw/mods/modlauncher/maven-metadata.xml";

fn modlauncher_jar_url(version: &str) -> String {
    format!(
        "https://maven.neoforged.net/releases/cpw/mods/modlauncher/{v}/modlauncher-{v}.jar",
        v = version.trim()
    )
}

fn version_tokens(s: &str) -> Vec<u32> {
    s.split('.')
        .filter_map(|seg| {
            let num: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
            num.parse().ok()
        })
        .collect()
}

fn cmp_maven_dot_version(a: &str, b: &str) -> Ordering {
    let ta = version_tokens(a);
    let tb = version_tokens(b);
    let n = ta.len().max(tb.len());
    for i in 0..n {
        let va = *ta.get(i).unwrap_or(&0);
        let vb = *tb.get(i).unwrap_or(&0);
        match va.cmp(&vb) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    Ordering::Equal
}

fn parse_metadata_all_versions(xml: &str) -> Vec<String> {
    super::xml_meta::parse_maven_metadata(xml).versions
}

fn parse_metadata_release(xml: &str) -> Option<String> {
    super::xml_meta::parse_maven_metadata(xml).release
}

/// Если `hint` нет среди опубликованных версий — ближайшая по линии `major.minor.*`, иначе `<release>`.
pub fn pick_published_modlauncher_version(hint: &str, metadata_xml: &str) -> String {
    let all = parse_metadata_all_versions(metadata_xml);
    if all.iter().any(|v| v == hint) {
        return hint.to_string();
    }
    let tokens = version_tokens(hint);
    let major = *tokens.first().unwrap_or(&0);
    let minor = *tokens.get(1).unwrap_or(&0);

    let mut same_mm: Vec<String> = all
        .iter()
        .filter(|v| {
            let t = version_tokens(v);
            t.get(0) == Some(&major) && t.get(1) == Some(&minor)
        })
        .cloned()
        .collect();
    same_mm.sort_by(|a, b| cmp_maven_dot_version(b, a));

    if let Some(v) = same_mm.into_iter().next() {
        return v;
    }

    let mut same_major: Vec<String> = all
        .iter()
        .filter(|v| version_tokens(v).first() == Some(&major))
        .cloned()
        .collect();
    same_major.sort_by(|a, b| cmp_maven_dot_version(b, a));
    if let Some(v) = same_major.into_iter().next() {
        return v;
    }

    parse_metadata_release(metadata_xml).unwrap_or_else(|| "11.0.5".to_string())
}

/// Цепочка профилей: явный `cpw.mods:modlauncher`, иначе версия `fancymodloader:loader` как подсказка.
pub fn modlauncher_version_from_neoforge_chain(all_v_infos: &[VersionInfo]) -> Option<String> {
    for v_info in all_v_infos {
        for lib in v_info.libraries.iter().chain(v_info.maven_files.iter()) {
            let n = lib.name.trim();
            if let Some(rest) = n.strip_prefix("cpw.mods:modlauncher:") {
                let ver = rest.split('@').next().unwrap_or("").trim();
                if !ver.is_empty() {
                    return Some(ver.to_string());
                }
            }
        }
    }
    for v_info in all_v_infos {
        for lib in v_info.libraries.iter().chain(v_info.maven_files.iter()) {
            let parts: Vec<&str> = lib.name.split(':').collect();
            if parts.len() >= 3
                && parts[0] == "net.neoforged.fancymodloader"
                && parts[1] == "loader"
            {
                let ver = parts[2].split('@').next().unwrap_or("").trim();
                if !ver.is_empty() {
                    return Some(ver.to_string());
                }
            }
        }
    }
    None
}

fn modlauncher_fallback_versions_after_404(requested: &str, metadata_xml: &str) -> Vec<String> {
    let all = parse_metadata_all_versions(metadata_xml);
    let tokens = version_tokens(requested);
    let major = *tokens.first().unwrap_or(&0);
    let minor = *tokens.get(1).unwrap_or(&0);

    let mut same_mm: Vec<String> = all
        .iter()
        .filter(|v| *v != requested)
        .filter(|v| {
            let t = version_tokens(v);
            t.get(0) == Some(&major) && t.get(1) == Some(&minor)
        })
        .cloned()
        .collect();
    same_mm.sort_by(|a, b| cmp_maven_dot_version(b, a));

    let mut out = same_mm;

    if out.is_empty() {
        let mut same_major: Vec<String> = all
            .iter()
            .filter(|v| *v != requested)
            .filter(|v| version_tokens(v).first() == Some(&major))
            .cloned()
            .collect();
        same_major.sort_by(|a, b| cmp_maven_dot_version(b, a));
        out = same_major;
    }

    if let Some(rel) = parse_metadata_release(metadata_xml) {
        if rel != requested && !out.contains(&rel) {
            out.push(rel);
        }
    }
    out
}

/// Скачивает `modlauncher` в `libraries/`. При 404 по подсказке из цепочки подбирает версию по maven-metadata.xml.
pub async fn download_neoforge_modlauncher_to_libraries<F: Fn(&str)>(
    lib_dir: &Path,
    requested: &str,
    log: &F,
) -> Result<PathBuf> {
    let rel_req = maven_to_path(&format!("cpw.mods:modlauncher:{requested}"), None);
    let dest_req = lib_dir.join(&rel_req);
    if dest_req.is_file() {
        return Ok(dest_req);
    }

    match download_file(&modlauncher_jar_url(requested), None).await {
        Ok(bytes) => {
            if let Some(p) = dest_req.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            tokio::fs::write(&dest_req, &bytes).await?;
            return Ok(dest_req);
        }
        Err(Error::Custom(s)) if s.contains("File not found") => {}
        Err(e) => return Err(e),
    }

    log(&format!(
        "NeoForge: modlauncher {requested} нет на Maven — смотрим {}",
        MODLAUNCHER_METADATA_URL
    ));

    let meta = download_file(MODLAUNCHER_METADATA_URL, None).await?;
    let xml = String::from_utf8_lossy(&meta);
    let fallbacks = modlauncher_fallback_versions_after_404(requested, &xml);

    for v in fallbacks {
        let rel = maven_to_path(&format!("cpw.mods:modlauncher:{v}"), None);
        let dest = lib_dir.join(&rel);
        if dest.is_file() {
            return Ok(dest);
        }
        log(&format!("NeoForge ForgeWrapper: пробуем modlauncher {v}…"));
        match download_file(&modlauncher_jar_url(&v), None).await {
            Ok(bytes) => {
                if let Some(p) = dest.parent() {
                    let _ = std::fs::create_dir_all(p);
                }
                tokio::fs::write(&dest, &bytes).await?;
                return Ok(dest);
            }
            Err(Error::Custom(s)) if s.contains("File not found") => continue,
            Err(e) => return Err(e),
        }
    }

    Err(Error::Custom(format!(
        "NeoForge ForgeWrapper: не удалось скачать modlauncher (подсказка «{requested}»). \
         Актуальные версии: {MODLAUNCHER_METADATA_URL}"
    )))
}
