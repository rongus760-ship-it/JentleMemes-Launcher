//! Атомарная запись конфигов через `tmp + fs::rename`, с ротацией `.bak`.
//!
//! Проблема, которую решает: пользовательские файлы `settings.json`, `instance.json`,
//! `pack_source.json` раньше писались через `fs::write(path, ...)`. Если процесс падает
//! посреди записи или диск выплёвывает `ENOSPC`, файл остаётся полу-записанным и при
//! следующем старте лаунчер получает корраптовый JSON (и на `serde::from_str` падает
//! в `Default::default()` — то есть пользователь **молча** теряет настройки).
//!
//! `write_atomic` пишет во временный `<path>.tmp`, `fsync`-ит его, затем делает
//! атомарный `rename` поверх целевого файла. На Windows `rename` поверх существующего
//! файла через [`std::fs::rename`] работает так же (POSIX-семантика обеспечивается
//! ядром). Перед `rename` старый файл копируется в `<path>.bak` (1 версия), чтобы дать
//! пользователю шанс на восстановление.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{Error, Result};

/// Записать `bytes` в `path` атомарно, с резервной копией старого состояния.
///
/// Порядок действий:
/// 1. `<path>.tmp` — создать, записать, `fsync`.
/// 2. Если `<path>` уже существует — скопировать в `<path>.bak`.
/// 3. `fs::rename(<path>.tmp, <path>)` — атомарная замена на POSIX.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        Error::Custom(format!(
            "atomic write: path has no parent: {}",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent)?;

    let tmp = path.with_extension(match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => format!("{ext}.tmp"),
        None => "tmp".to_string(),
    });

    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }

    if path.exists() {
        let bak = path.with_extension(match path.extension().and_then(|s| s.to_str()) {
            Some(ext) => format!("{ext}.bak"),
            None => "bak".to_string(),
        });
        // Не ломаем запись, если .bak не удалось создать (например, read-only каталог
        // внешнего диска). Лог в stderr для диагностики.
        if let Err(e) = fs::copy(path, &bak) {
            eprintln!(
                "[JentleMemes] atomic write: failed to snapshot .bak for {}: {}",
                path.display(),
                e
            );
        }
    }

    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::Custom(format!(
            "atomic write: rename failed for {}: {}",
            path.display(),
            e
        ))
    })?;
    Ok(())
}

/// Шорткат для текстовых JSON-конфигов.
pub fn write_atomic_string(path: &Path, text: &str) -> Result<()> {
    write_atomic(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("jm_atomic_fs_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn atomic_write_creates_new_file() {
        let d = tmp_dir();
        let p = d.join("config.json");
        write_atomic_string(&p, "{\"a\":1}").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "{\"a\":1}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn atomic_write_creates_backup_when_overwriting() {
        let d = tmp_dir();
        let p = d.join("config.json");
        fs::write(&p, "{\"a\":1}").unwrap();
        write_atomic_string(&p, "{\"a\":2}").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "{\"a\":2}");
        let bak = p.with_extension("json.bak");
        assert!(bak.exists(), "expected .bak at {}", bak.display());
        assert_eq!(fs::read_to_string(&bak).unwrap(), "{\"a\":1}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn atomic_write_leaves_no_tmp_on_success() {
        let d = tmp_dir();
        let p = d.join("config.json");
        write_atomic_string(&p, "{}").unwrap();
        let tmp = p.with_extension("json.tmp");
        assert!(!tmp.exists());
        let _ = fs::remove_dir_all(&d);
    }
}
