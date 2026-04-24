use std::fs;
use std::path::Path;

use crate::core::types::Library;

pub fn library_rel_path_effective_for_os(lib: &Library, mc_version: &str, current_os: &str) -> String {
    let name = normalize_legacy_forge_minecraftforge_coord(&lib.name, mc_version)
        .unwrap_or_else(|| lib.name.clone());
    let mut l = lib.clone();
    l.name = name;
    library_rel_path_for_classpath(&l, current_os)
}

pub fn library_rel_path_under_libraries(lib: &Library) -> String {
    if let Some(ref downloads) = lib.downloads {
        if let Some(artifact) = downloads.get("artifact").and_then(|v| v.as_object()) {
            if let Some(p) = artifact.get("path").and_then(|v| v.as_str()) {
                let t = p.trim();
                if !t.is_empty() {
                    return t.replace('\\', "/");
                }
            }
        }
    }
    maven_to_path(&lib.name, None)
}

pub fn library_rel_path_for_classpath(lib: &Library, current_os: &str) -> String {
    let os_key = match current_os {
        "linux" => "linux",
        "osx" => "osx",
        "windows" => "windows",
        _ => return library_rel_path_under_libraries(lib),
    };
    let has_artifact_path = lib
        .downloads
        .as_ref()
        .and_then(|d| d.get("artifact"))
        .and_then(|v| v.as_object())
        .and_then(|o| o.get("path"))
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if let Some(natives) = &lib.natives {
        if let Some(obj) = natives.as_object() {
            if let Some(classifier) = obj.get(os_key).and_then(|v| v.as_str()) {
                if !has_artifact_path {
                    if let Some(p) = lib
                        .downloads
                        .as_ref()
                        .and_then(|d| d.get("classifiers"))
                        .and_then(|c| c.get(classifier))
                        .and_then(|v| v.as_object())
                        .and_then(|o| o.get("path"))
                        .and_then(|v| v.as_str())
                    {
                        let t = p.trim();
                        if !t.is_empty() {
                            return t.replace('\\', "/");
                        }
                    }
                    return maven_to_path(&lib.name, Some(classifier));
                }
            }
        }
    }
    library_rel_path_under_libraries(lib)
}

/// Каталог `libraries/net/minecraft/client/<ver>` для vanilla `jar_id` (например `1.20.1`).
/// У Forge 1.20.x артефакты часто лежат в `1.20.1-20230612.114412`, а не в `1.20.1`.
pub fn resolve_net_minecraft_client_dir_name(jar_id: &str, lib_dir: &Path) -> String {
    let base = lib_dir.join("net").join("minecraft").join("client");
    let Ok(rd) = fs::read_dir(&base) else {
        return jar_id.to_string();
    };
    let prefix = format!("{jar_id}-");
    let mut names: Vec<String> = rd
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            let p = base.join(n);
            p.is_dir() && (n == jar_id || n.starts_with(&prefix))
        })
        .collect();
    if names.is_empty() {
        return jar_id.to_string();
    }
    names.sort_by(|a, b| b.len().cmp(&a.len()));
    names
        .into_iter()
        .next()
        .unwrap_or_else(|| jar_id.to_string())
}

pub fn maven_to_path(name: &str, override_classifier: Option<&str>) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return name.to_string();
    }

    let group = parts[0].replace('.', "/");
    let artifact = parts[1];

    let mut version = parts[2];
    let mut ext = "jar";

    if version.contains('@') {
        let v_parts: Vec<&str> = version.split('@').collect();
        version = v_parts[0];
        ext = v_parts[1];
    }

    let mut classifier = if let Some(c) = override_classifier {
        format!("-{}", c)
    } else {
        "".to_string()
    };

    if parts.len() > 3 {
        let mut c_str = parts[3];
        if c_str.contains('@') {
            let c_parts: Vec<&str> = c_str.split('@').collect();
            c_str = c_parts[0];
            ext = c_parts[1];
        }
        if override_classifier.is_none() {
            classifier = format!("-{}", c_str);
        }
    }

    format!(
        "{}/{}/{}/{}-{}{}.{}",
        group, artifact, version, artifact, version, classifier, ext
    )
}

pub fn normalize_legacy_forge_minecraftforge_coord(name: &str, mc_version: &str) -> Option<String> {
    const PREFIX: &str = "net.minecraftforge:minecraftforge:";
    let tail = name.strip_prefix(PREFIX)?;
    if tail.is_empty() || tail.contains(':') {
        return None;
    }
    if tail.contains('-') {
        return None;
    }
    let mc = mc_version.trim();
    if mc.is_empty() {
        return None;
    }
    Some(format!("net.minecraftforge:forge:{mc}-{tail}"))
}

pub fn libraries_minecraft_to_maven_central(url: &str) -> Option<String> {
    const OLD: &str = "https://libraries.minecraft.net/";
    if url.starts_with(OLD) {
        Some(url.replacen(OLD, "https://repo1.maven.org/maven2/", 1))
    } else {
        None
    }
}
