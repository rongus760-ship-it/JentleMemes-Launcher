use crate::error::{Error, Result};
use sha1::{Digest, Sha1};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const HASH_BUF_SIZE: usize = 64 * 1024;

pub fn verify_sha1(path: &Path, expected: &str) -> Result<bool> {
    let file = fs::File::open(path)?;
    let mut reader = std::io::BufReader::with_capacity(HASH_BUF_SIZE, file);
    let mut hasher = Sha1::new();
    let mut buf = [0u8; HASH_BUF_SIZE];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = format!("{:x}", hasher.finalize());
    Ok(actual == expected)
}

pub fn reflink_or_copy(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    reflink_copy::reflink_or_copy(src, dst).map_err(|e| {
        Error::Custom(format!(
            "reflink/copy {} → {}: {e}",
            src.display(),
            dst.display()
        ))
    })?;
    Ok(())
}

pub fn atomic_write_bytes(dest: &Path, data: &[u8]) -> Result<()> {
    let tmp = tmp_path(dest);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&tmp, data)?;
    fs::rename(&tmp, dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::Custom(format!("atomic rename: {e}"))
    })
}

pub async fn atomic_download(
    url: &str,
    pool_path: &Path,
    expected_sha1: Option<&str>,
) -> Result<()> {
    let bytes = crate::core::utils::download::download_file(url, expected_sha1).await?;
    let dest = pool_path.to_path_buf();
    tokio::task::spawn_blocking(move || atomic_write_bytes(&dest, &bytes)).await??;
    Ok(())
}

pub async fn materialize_to_instance(
    pool_path: &Path,
    instance_path: &Path,
    expected_sha1: Option<&str>,
    download_url: &str,
) -> Result<()> {
    if instance_path.exists() {
        if let Some(sha1) = expected_sha1 {
            let p = instance_path.to_path_buf();
            let s = sha1.to_string();
            let ok =
                tokio::task::spawn_blocking(move || verify_sha1(&p, &s)).await??;
            if ok {
                return Ok(());
            }
            let _ = tokio::fs::remove_file(instance_path).await;
        } else {
            return Ok(());
        }
    }

    if pool_path.exists() {
        let pool_ok = if let Some(sha1) = expected_sha1 {
            let p = pool_path.to_path_buf();
            let s = sha1.to_string();
            tokio::task::spawn_blocking(move || verify_sha1(&p, &s)).await??
        } else {
            true
        };
        if pool_ok {
            let src = pool_path.to_path_buf();
            let dst = instance_path.to_path_buf();
            tokio::task::spawn_blocking(move || reflink_or_copy(&src, &dst)).await??;
            return Ok(());
        }
        let _ = tokio::fs::remove_file(pool_path).await;
    }

    if !download_url.is_empty() {
        atomic_download(download_url, pool_path, expected_sha1).await?;
        let src = pool_path.to_path_buf();
        let dst = instance_path.to_path_buf();
        tokio::task::spawn_blocking(move || reflink_or_copy(&src, &dst)).await??;
    }

    Ok(())
}

fn tmp_path(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let id = uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_string();
    dest.with_file_name(format!("{name}.tmp_{id}"))
}
