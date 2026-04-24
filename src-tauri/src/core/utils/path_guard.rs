use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub fn sanitize_path_within(raw: &str, allowed_root: &Path) -> Result<PathBuf> {
    let canonical_root = allowed_root
        .canonicalize()
        .map_err(|e| Error::Custom(format!("Sandbox root does not exist: {e}")))?;
    let joined = allowed_root.join(raw);
    let resolved = joined
        .canonicalize()
        .map_err(|e| Error::Custom(format!("Path does not exist or is invalid: {e}")))?;
    if !resolved.starts_with(&canonical_root) {
        return Err(Error::Custom(format!(
            "Path escapes sandbox: {} is outside {}",
            resolved.display(),
            canonical_root.display()
        )));
    }
    Ok(resolved)
}

// В проде используется `sanitize_path_within`; эта хелпер-функция живёт ради
// интеграционных тестов (проверяет, что абсолютный путь остаётся внутри sandbox).
#[cfg_attr(not(test), allow(dead_code))]
pub fn assert_path_within(absolute: &Path, allowed_root: &Path) -> Result<()> {
    let canonical_root = allowed_root
        .canonicalize()
        .map_err(|e| Error::Custom(format!("Sandbox root canonicalize: {e}")))?;
    let resolved = absolute
        .canonicalize()
        .map_err(|e| Error::Custom(format!("Path canonicalize: {e}")))?;
    if !resolved.starts_with(&canonical_root) {
        return Err(Error::Custom(format!(
            "Path escapes sandbox: {} is outside {}",
            resolved.display(),
            canonical_root.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Создаёт временный каталог с файлом внутри и возвращает (root, file).
    fn fresh_sandbox() -> (PathBuf, PathBuf) {
        let tmp = std::env::temp_dir().join(format!(
            "jm_path_guard_test_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&tmp).unwrap();
        let file = tmp.join("inside.txt");
        fs::write(&file, b"ok").unwrap();
        (tmp, file)
    }

    #[test]
    fn sanitize_path_accepts_relative_path_within_root() {
        let (root, _file) = fresh_sandbox();
        let result = sanitize_path_within("inside.txt", &root).expect("should resolve");
        assert!(result.starts_with(root.canonicalize().unwrap()));
    }

    #[test]
    fn sanitize_path_rejects_parent_escape() {
        let (root, _file) = fresh_sandbox();
        let outside = root.parent().unwrap().join("outside-secret.txt");
        fs::write(&outside, b"nope").unwrap();
        let relative = format!("../{}", outside.file_name().unwrap().to_string_lossy());
        let result = sanitize_path_within(&relative, &root);
        assert!(result.is_err(), "expected escape rejection, got {:?}", result);
        let _ = fs::remove_file(&outside);
    }

    #[test]
    fn sanitize_path_rejects_nonexistent() {
        let (root, _file) = fresh_sandbox();
        let result = sanitize_path_within("does-not-exist.txt", &root);
        assert!(result.is_err());
    }

    #[test]
    fn assert_path_within_rejects_outside() {
        let (root, _file) = fresh_sandbox();
        let outside = std::env::temp_dir();
        let result = assert_path_within(&outside, &root);
        assert!(result.is_err());
    }
}
