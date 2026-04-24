use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
pub fn ensure_long_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if s.starts_with(r"\\?\") {
        p.to_path_buf()
    } else if let Ok(abs) = p.canonicalize() {
        let abs_s = abs.to_string_lossy();
        if abs_s.starts_with(r"\\?\") {
            abs
        } else {
            PathBuf::from(format!(r"\\?\{}", abs_s))
        }
    } else {
        p.to_path_buf()
    }
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_long_path(p: &Path) -> PathBuf {
    p.to_path_buf()
}
