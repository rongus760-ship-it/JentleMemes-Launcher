use crate::core::types::Library;
use crate::error::Result;
use std::fs;
use std::path::Path;

pub fn resolve(
    data_dir: &Path,
    instance_dir: &Path,
    all_libraries: &[Library],
) -> Result<String> {
    let natives_dir = instance_dir.join("natives");
    fs::create_dir_all(&natives_dir)?;

    #[cfg(target_os = "linux")]
    let os_key = "linux";
    #[cfg(target_os = "windows")]
    let os_key = "windows";
    #[cfg(target_os = "macos")]
    let os_key = "osx";

    let lib_dir = data_dir.join("libraries");

    for lib in all_libraries {
        let classifier_key = if let Some(natives) = &lib.natives {
            natives
                .get(os_key)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        let Some(classifier) = classifier_key else {
            continue;
        };

        let rel = crate::core::utils::maven::maven_to_path(&lib.name, Some(&classifier));
        let jar_path = lib_dir.join(&rel);
        if !jar_path.is_file() {
            continue;
        }

        let file = fs::File::open(&jar_path)?;
        let mut archive = match zip::ZipArchive::new(file) {
            Ok(a) => a,
            Err(_) => continue,
        };

        for i in 0..archive.len() {
            let mut entry = match archive.by_index(i) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let name = entry.name().to_string();
            if name.starts_with("META-INF") || entry.is_dir() {
                continue;
            }
            let out_path =
                natives_dir.join(std::path::Path::new(&name).file_name().unwrap_or_default());
            if !out_path.exists() {
                if let Ok(mut out_file) = fs::File::create(&out_path) {
                    let _ = std::io::copy(&mut entry, &mut out_file);
                }
            }
        }
    }

    Ok(natives_dir.to_string_lossy().replace('\\', "/"))
}
