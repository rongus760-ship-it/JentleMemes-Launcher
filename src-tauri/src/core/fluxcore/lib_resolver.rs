use crate::core::game::launch::check_rules;
use crate::core::types::VersionInfo;
use crate::core::utils::maven::library_rel_path_under_libraries;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct LibResult {
    pub classpath: Vec<String>,
    pub missing_libs: Vec<String>,
}

pub fn resolve(
    all_v_infos: &[VersionInfo],
    lib_dir: &Path,
    current_os: &str,
) -> LibResult {
    let mut existing: HashSet<String> = HashSet::new();
    if let Ok(walk) = walkdir::WalkDir::new(lib_dir)
        .min_depth(1)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()
    {
        for entry in walk {
            if entry.file_type().is_file() {
                if let Ok(rel) = entry.path().strip_prefix(lib_dir) {
                    existing.insert(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }

    let mut classpath: Vec<String> = Vec::new();
    let mut lib_key_index: HashMap<String, usize> = HashMap::new();
    let mut missing_libs: Vec<String> = Vec::new();

    for v_info in all_v_infos {
        for lib in v_info.libraries.iter().chain(v_info.maven_files.iter()) {
            if !check_rules(&lib.rules, current_os) {
                continue;
            }
            let rel = library_rel_path_under_libraries(lib);
            if !existing.contains(&rel) {
                missing_libs.push(rel.clone());
                continue;
            }
            let full_path = lib_dir.join(&rel);
            let parts: Vec<&str> = lib.name.split(':').collect();
            let dedup_key = if parts.len() >= 4 {
                format!("{}:{}:{}", parts[0], parts[1], parts[3].split('@').next().unwrap_or(""))
            } else if parts.len() >= 2 {
                format!("{}:{}", parts[0], parts[1])
            } else {
                lib.name.clone()
            };
            let path_str = full_path.to_string_lossy().replace('\\', "/");
            if let Some(&idx) = lib_key_index.get(&dedup_key) {
                classpath[idx] = path_str;
            } else {
                lib_key_index.insert(dedup_key, classpath.len());
                classpath.push(path_str);
            }
        }
    }

    LibResult {
        classpath,
        missing_libs,
    }
}
