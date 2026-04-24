//! Команды работы с каталогом фоновых изображений.
//!
//! Разделено из `commands.rs` как часть Phase 3.1 (декомпозиция). Все операции
//! санитайзят путь через `path_guard::sanitize_path_within`, чтобы предотвратить
//! выход из sandbox-каталога `<data_dir>/backgrounds`.

use base64::{engine::general_purpose::STANDARD, Engine};

use crate::core::utils::path_guard::sanitize_path_within;
use crate::error::Error;

use super::CmdResult;

#[tauri::command]
pub async fn get_backgrounds() -> CmdResult<Vec<String>> {
    let bg_dir = crate::config::get_data_dir().join("backgrounds");
    let _ = std::fs::create_dir_all(&bg_dir);
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&bg_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ["png", "jpg", "jpeg", "webp", "gif"]
                    .contains(&ext.to_lowercase().as_str())
                {
                    files.push(path.to_string_lossy().to_string());
                }
            }
        }
    }
    Ok(files)
}

#[tauri::command]
pub async fn pick_image_file() -> CmdResult<Option<String>> {
    let result = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
            .set_title("Выберите изображение для фона")
            .pick_file()
    })
    .await
    .map_err(|e| Error::Custom(e.to_string()))?;
    Ok(result.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn copy_background(source_path: String) -> CmdResult<String> {
    let bg_dir = crate::config::get_data_dir().join("backgrounds");
    std::fs::create_dir_all(&bg_dir).map_err(|e| {
        Error::Custom(format!("Не удалось создать каталог backgrounds: {}", e))
    })?;
    let trimmed = source_path.trim().to_owned();
    let src_path = std::path::Path::new(trimmed.as_str());
    if !src_path.exists() {
        return Err(Error::Custom(format!(
            "Файл недоступен или не найден (часто на Linux после выбора через портал нужен доступ к каталогу): {}",
            source_path
        )));
    }
    let src_canon = src_path.canonicalize().map_err(|e| {
        Error::Custom(format!(
            "Не удалось открыть путь к изображению: {} ({})",
            source_path, e
        ))
    })?;
    let ext = src_canon
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let stem = src_canon
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("bg");
    let safe_stem: String = stem
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(48)
        .collect();
    let safe_stem = if safe_stem.is_empty() {
        "bg".to_string()
    } else {
        safe_stem
    };
    let dest_name = format!("{}_{}.{}", safe_stem, uuid::Uuid::new_v4(), ext);
    let dest = bg_dir.join(dest_name);
    std::fs::copy(&src_canon, &dest).map_err(|e| {
        Error::Custom(format!(
            "Не удалось сохранить фон в каталог данных ({} → {}): {}",
            src_canon.display(),
            dest.display(),
            e
        ))
    })?;
    Ok(dest.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn delete_background(path: String) -> CmdResult<()> {
    let bg_dir = crate::config::get_data_dir().join("backgrounds");
    let _ = std::fs::create_dir_all(&bg_dir);
    let safe = sanitize_path_within(&path, &bg_dir)?;
    if safe.exists() {
        std::fs::remove_file(&safe)?;
    }
    Ok(())
}

#[tauri::command]
pub fn read_local_image_data_url(path: String) -> CmdResult<String> {
    const MAX_BYTES: u64 = 15 * 1024 * 1024;
    let bg_dir = crate::config::get_data_dir().join("backgrounds");
    std::fs::create_dir_all(&bg_dir)?;
    let safe = sanitize_path_within(&path, &bg_dir)?;
    let meta = std::fs::metadata(&safe)?;
    if meta.len() > MAX_BYTES {
        return Err(Error::Custom(format!(
            "Файл слишком большой (лимит {} МБ)",
            MAX_BYTES / (1024 * 1024)
        )));
    }
    let ext = safe
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => return Err(Error::Custom("Неподдерживаемый формат изображения".into())),
    };
    let bytes = std::fs::read(&safe)?;
    let b64 = STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}
