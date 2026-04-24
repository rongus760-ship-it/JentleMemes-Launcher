use crate::core::game::version_layout;
use crate::core::types::VersionInfo;
use crate::error::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ChainResult {
    pub all_v_infos: Vec<VersionInfo>,
    pub main_class: String,
    pub jar_id: String,
    pub is_forge_wrapper: bool,
    pub is_bootstrap_launcher: bool,
    pub is_legacy_forge: bool,
    pub profile_json_paths: Vec<PathBuf>,
}

pub fn resolve(data_dir: &Path, version_id: &str) -> Result<ChainResult> {
    let mut current_id = version_id.to_string();
    let mut all_v_infos = Vec::new();
    let mut profile_json_paths = Vec::new();

    loop {
        let v_path = version_layout::profile_json_path(data_dir, &current_id);
        if !v_path.exists() {
            break;
        }
        let v_info: VersionInfo = serde_json::from_str(&fs::read_to_string(&v_path)?)?;
        profile_json_paths.push(v_path);
        let parent = v_info.inherits_from.clone();
        all_v_infos.push(v_info);
        if let Some(p) = parent {
            current_id = p;
        } else {
            break;
        }
    }

    all_v_infos.reverse();
    profile_json_paths.reverse();

    if all_v_infos.is_empty() {
        return Err(Error::Custom("Профили не найдены".into()));
    }

    let main_v_info = all_v_infos.last().unwrap();
    let vanilla_info = all_v_infos.first().unwrap();

    let main_class = main_v_info
        .main_class
        .clone()
        .or_else(|| vanilla_info.main_class.clone())
        .ok_or_else(|| Error::Custom("Main class not found".into()))?;

    let is_forge_wrapper = main_class.to_lowercase().contains("forgewrapper");
    let is_bootstrap_launcher = main_class.to_lowercase().contains("bootstraplauncher");
    let is_legacy_forge = main_class.to_lowercase().contains("launchwrapper")
        || all_v_infos.iter().any(|v| {
            v.minecraft_arguments
                .as_deref()
                .map(|m| m.contains("FMLTweaker") || m.contains("fml.common.launcher"))
                .unwrap_or(false)
        });

    let leaf_inherits = main_v_info.inherits_from.as_deref().unwrap_or(version_id);
    let jar_id = vanilla_info
        .id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(leaf_inherits)
        .to_string();

    Ok(ChainResult {
        all_v_infos,
        main_class,
        jar_id,
        is_forge_wrapper,
        is_bootstrap_launcher,
        is_legacy_forge,
        profile_json_paths,
    })
}
