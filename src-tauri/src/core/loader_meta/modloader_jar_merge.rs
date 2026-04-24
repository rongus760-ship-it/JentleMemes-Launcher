use crate::error::{Error, Result};
use std::collections::HashSet;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::read::ZipArchive;
use zip::write::FileOptions;
use zip::CompressionMethod;

fn is_meta_inf_entry(name: &str) -> bool {
    let n = name.trim_start_matches("./").replace('\\', "/");
    let lower = n.to_ascii_lowercase();
    lower == "meta-inf" || lower.starts_with("meta-inf/")
}

pub fn merge_zip_into_jar(jar_path: &Path, overlay_zip_bytes: &[u8]) -> Result<()> {
    let tmp_path = jar_path.with_extension("jar.tmp_merge");

    let jar_bytes = fs::read(jar_path).map_err(|e| Error::Custom(e.to_string()))?;
    let mut base_archive = ZipArchive::new(Cursor::new(&jar_bytes))
        .map_err(|e| Error::Custom(format!("ModLoader merge: base not zip ({e})")))?;
    let mut overlay_archive = ZipArchive::new(Cursor::new(overlay_zip_bytes))
        .map_err(|e| Error::Custom(format!("ModLoader merge: overlay not zip ({e})")))?;

    let out_file = fs::File::create(&tmp_path)
        .map_err(|e| Error::Custom(format!("ModLoader merge: create tmp: {e}")))?;
    let mut writer = zip::ZipWriter::new(out_file);
    let opts = FileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut overlay_names: HashSet<String> = HashSet::new();
    for i in 0..overlay_archive.len() {
        let file = overlay_archive
            .by_index(i)
            .map_err(|e| Error::Custom(e.to_string()))?;
        let name = file.name().to_string();
        if !name.ends_with('/') && !is_meta_inf_entry(&name) {
            overlay_names.insert(name);
        }
    }

    let mut buf = Vec::with_capacity(8192);

    for i in 0..base_archive.len() {
        let mut file = base_archive
            .by_index(i)
            .map_err(|e| Error::Custom(e.to_string()))?;
        let name = file.name().to_string();
        if name.ends_with('/') || is_meta_inf_entry(&name) || overlay_names.contains(&name) {
            continue;
        }
        buf.clear();
        file.read_to_end(&mut buf)
            .map_err(|e| Error::Custom(e.to_string()))?;
        writer
            .start_file(&name, opts)
            .map_err(|e| Error::Custom(e.to_string()))?;
        writer
            .write_all(&buf)
            .map_err(|e| Error::Custom(e.to_string()))?;
    }

    for i in 0..overlay_archive.len() {
        let mut file = overlay_archive
            .by_index(i)
            .map_err(|e| Error::Custom(e.to_string()))?;
        let name = file.name().to_string();
        if name.ends_with('/') || is_meta_inf_entry(&name) {
            continue;
        }
        buf.clear();
        file.read_to_end(&mut buf)
            .map_err(|e| Error::Custom(e.to_string()))?;
        writer
            .start_file(&name, opts)
            .map_err(|e| Error::Custom(e.to_string()))?;
        writer
            .write_all(&buf)
            .map_err(|e| Error::Custom(e.to_string()))?;
    }

    writer
        .finish()
        .map_err(|e| Error::Custom(format!("ModLoader merge: finalize zip: {e}")))?;
    fs::rename(&tmp_path, jar_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        Error::Custom(format!("ModLoader merge: atomic rename: {e}"))
    })?;
    Ok(())
}
