use crate::core::game::install;
use crate::core::game::version_layout;
use crate::error::Result;
use std::path::Path;

pub struct ForgeResult {
    pub client_jar_path: Option<String>,
    pub installer_path: Option<String>,
}

pub async fn resolve(
    data_dir: &Path,
    version_id: &str,
    jar_id: &str,
    is_forge_wrapper: bool,
    is_legacy_forge: bool,
) -> Result<ForgeResult> {
    let needs_official = is_forge_wrapper
        || (version_layout::is_neoforge_profile_id(version_id) && !is_legacy_forge);

    if !needs_official {
        return Ok(ForgeResult {
            client_jar_path: None,
            installer_path: None,
        });
    }

    let mut mc_hints: Vec<String> = vec![jar_id.to_string()];
    if let Some(m) = version_layout::minecraft_version_from_profile_id(version_id) {
        if !mc_hints.contains(&m) {
            mc_hints.push(m);
        }
    }

    let inst_jar = version_layout::profile_dir(data_dir, version_id).join("installer.jar");
    if inst_jar.is_file() {
        if let Some(m) = install::forge_minecraft_version_from_installer_jar(&inst_jar) {
            if !mc_hints.contains(&m) {
                mc_hints.push(m);
            }
        }
    }

    let downloaded =
        install::download_official_client_jar_to_libraries(data_dir, &mc_hints).await?;
    install::ensure_official_client_jar_for_forge_libraries(
        data_dir,
        version_id,
        &downloaded,
        &mc_hints,
    )?;

    let client_jar = downloaded.to_string_lossy().replace('\\', "/");

    let installer_path = if is_forge_wrapper && inst_jar.is_file() {
        match install::patch_installer_remove_processor_outputs(&inst_jar) {
            Ok(patched) => Some(patched.to_string_lossy().replace('\\', "/")),
            Err(_) => Some(inst_jar.to_string_lossy().replace('\\', "/")),
        }
    } else if inst_jar.is_file() {
        Some(inst_jar.to_string_lossy().replace('\\', "/"))
    } else {
        None
    };

    Ok(ForgeResult {
        client_jar_path: Some(client_jar),
        installer_path,
    })
}
