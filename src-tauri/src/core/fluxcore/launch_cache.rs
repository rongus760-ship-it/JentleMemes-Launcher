use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LaunchCache {
    pub version: u32,
    pub chain_hash: String,
    pub settings_hash: String,
    pub classpath: Vec<String>,
    pub asset_index: String,
    pub natives_extracted: Vec<String>,
    pub java_path: String,
    pub java_major: u32,
    pub forge_client_jar: Option<String>,
    pub created_at: u64,
}

impl LaunchCache {
    pub fn path(profile_dir: &Path) -> PathBuf {
        profile_dir.join("launch_cache.json")
    }

    pub fn load(profile_dir: &Path) -> Option<Self> {
        let p = Self::path(profile_dir);
        let data = fs::read_to_string(&p).ok()?;
        let cache: Self = serde_json::from_str(&data).ok()?;
        if cache.version != 1 {
            return None;
        }
        for cp in &cache.classpath {
            if !Path::new(cp).exists() {
                eprintln!(
                    "[FluxCore] LaunchCache invalidated: missing classpath entry {}",
                    cp
                );
                return None;
            }
        }
        for np in &cache.natives_extracted {
            if !Path::new(np).exists() {
                eprintln!(
                    "[FluxCore] LaunchCache invalidated: missing native {}",
                    np
                );
                return None;
            }
        }
        if !cache.java_path.is_empty() && !Path::new(&cache.java_path).exists() {
            eprintln!(
                "[FluxCore] LaunchCache invalidated: java binary missing {}",
                cache.java_path
            );
            return None;
        }
        Some(cache)
    }

    pub fn save(&self, profile_dir: &Path) -> Result<()> {
        let p = Self::path(profile_dir);
        let tmp = p.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &p).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            Error::Custom(format!("Atomic rename launch_cache: {e}"))
        })?;
        Ok(())
    }

    pub fn invalidate(profile_dir: &Path) {
        let _ = fs::remove_file(Self::path(profile_dir));
    }
}

pub fn compute_chain_hash(profile_json_paths: &[PathBuf]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for p in profile_json_paths {
        if let Ok(meta) = fs::metadata(p) {
            let mtime = meta
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            // Tracing-diagnostic: когда `cache_miss` ловит chain_match=false, видно, у какого
            // именно файла mtime изменился между запусками (виновник — кто-то, кто переписывает
            // JSON профиля на каждом старте). INFO (не debug), чтобы поймать без `JM_LOG=debug`.
            tracing::info!(
                target: "fluxcore::launch_cache",
                chain_entry = %p.display(),
                mtime_ms = mtime,
            );
            hasher.update(format!("{}:{}", p.display(), mtime).as_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}

pub fn compute_settings_hash(ram_mb: u32, jvm_args: &str, custom_java: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("{ram_mb}:{jvm_args}:{custom_java}").as_bytes());
    format!("{:x}", hasher.finalize())
}
