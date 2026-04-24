use crate::error::Result;
use futures::future::try_join_all;
use std::path::{Path, PathBuf};

pub fn suggested_parallel_strands(io_weight_hint: u32) -> usize {
    let n = (io_weight_hint / 40).max(1).min(8);
    n as usize
}

pub async fn resolve_legacy_forge_universals_chunked(
    coords: Vec<String>,
    lib_dir: &Path,
    installer_jar: Option<PathBuf>,
    chunk_size: usize,
) -> Result<()> {
    if coords.is_empty() {
        return Ok(());
    }
    let lib_dir = lib_dir.to_path_buf();
    let chunk_size = chunk_size.clamp(1, 12);
    for chunk in coords.chunks(chunk_size) {
        let futs: Vec<_> = chunk
            .iter()
            .cloned()
            .map(|coord| {
                let lib_dir = lib_dir.clone();
                let inst = installer_jar.clone();
                async move {
                    crate::core::game::install::ensure_legacy_forge_universal_jar_resolved(
                        inst.as_deref().filter(|p| p.is_file()),
                        &lib_dir,
                        &coord,
                    )
                    .await
                }
            })
            .collect();
        try_join_all(futs).await?;
    }
    Ok(())
}
