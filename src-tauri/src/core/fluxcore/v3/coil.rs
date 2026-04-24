use crate::core::fluxcore::launch_cache;
use crate::core::game::version_layout::profile_json_path;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const COIL_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoilSnapshotFile {
    pub schema: u32,
    pub version_id: String,
    pub inputs_digest: String,
    pub classpath_fingerprint: String,
    pub artifact_count: u32,
    pub updated_unix: u64,
}

fn coil_snapshot_path(game_dir: &Path) -> PathBuf {
    game_dir.join(".fluxcore").join("coil_snapshot.json")
}

pub fn profile_chain_paths(data_dir: &Path, version_id: &str) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut current = version_id.to_string();
    loop {
        let p = profile_json_path(data_dir, &current);
        if !p.is_file() {
            break;
        }
        paths.push(p.clone());
        let text = fs::read_to_string(&p)?;
        let v: serde_json::Value = serde_json::from_str(&text)?;
        match v.get("inheritsFrom").and_then(|x| x.as_str()) {
            Some(parent) if !parent.trim().is_empty() => current = parent.to_string(),
            _ => break,
        }
    }
    paths.reverse();
    Ok(paths)
}

pub fn profile_inputs_digest(data_dir: &Path, version_id: &str) -> Result<String> {
    let paths = profile_chain_paths(data_dir, version_id)?;
    Ok(launch_cache::compute_chain_hash(&paths))
}

pub fn classpath_fingerprint(classpath: &[String]) -> String {
    let mut sorted: Vec<&str> = classpath.iter().map(|s| s.as_str()).collect();
    sorted.sort_unstable();
    let joined = sorted.join("\n");
    let mut h = Sha256::new();
    h.update(joined.as_bytes());
    format!("{:x}", h.finalize())
}

pub fn load_snapshot(game_dir: &Path) -> Option<CoilSnapshotFile> {
    let p = coil_snapshot_path(game_dir);
    let data = fs::read_to_string(&p).ok()?;
    let s: CoilSnapshotFile = serde_json::from_str(&data).ok()?;
    if s.schema != COIL_SCHEMA {
        return None;
    }
    Some(s)
}

pub fn persist_classpath_snapshot(
    game_dir: &Path,
    version_id: &str,
    inputs_digest: &str,
    classpath: &[String],
) -> Result<super::types::ClasspathSnapshot> {
    let dir = game_dir.join(".fluxcore");
    fs::create_dir_all(&dir)?;
    let fp = classpath_fingerprint(classpath);
    let snap = CoilSnapshotFile {
        schema: COIL_SCHEMA,
        version_id: version_id.to_string(),
        inputs_digest: inputs_digest.to_string(),
        classpath_fingerprint: fp.clone(),
        artifact_count: classpath.len() as u32,
        updated_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    let p = coil_snapshot_path(game_dir);
    let tmp = p.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(&snap)?;
    fs::write(&tmp, &json)?;
    fs::rename(&tmp, &p).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::Custom(format!("FluxCore Coil: atomic rename: {e}"))
    })?;
    Ok(super::types::ClasspathSnapshot {
        key_hash: inputs_digest.to_string(),
        classpath_fingerprint: fp,
        artifact_count: classpath.len() as u32,
    })
}
