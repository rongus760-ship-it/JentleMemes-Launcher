use std::fs;
use std::time::Duration;
use tokio::time::sleep;
use crate::config::{InstanceConfig, InstanceSettings, get_data_dir};
use crate::error::{Result, Error};

/// Абсолютный путь для Tauri convertFileSrc (без сиротских symlink при ошибке canonicalize).
fn normalize_existing_path(p: std::path::PathBuf) -> String {
    if p.exists() {
        if let Ok(c) = p.canonicalize() {
            return c.to_string_lossy().to_string();
        }
    }
    p.to_string_lossy().to_string()
}

/// Ищет иконку в директории инстанса: instance.png, icon.png, icon.jpg, .icon.*
fn resolve_instance_icon(inst_dir: &std::path::Path, existing_icon: &str) -> String {
    if !existing_icon.is_empty() {
        let p = std::path::Path::new(existing_icon);
        if p.is_absolute() && p.exists() {
            return normalize_existing_path(p.to_path_buf());
        }
        let rel = inst_dir.join(existing_icon);
        if rel.exists() {
            return normalize_existing_path(rel);
        }
    }
    for name in ["instance.png", "icon.png", "icon.jpg", "icon.webp"] {
        let p = inst_dir.join(name);
        if p.exists() {
            return normalize_existing_path(p);
        }
    }
    if let Ok(entries) = fs::read_dir(inst_dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n == ".icon.png" || n == ".icon.jpg" || n == ".icon.webp" {
                return normalize_existing_path(e.path());
            }
        }
    }
    existing_icon.to_string()
}

pub fn get_all() -> Result<Vec<InstanceConfig>> {
    let instances_dir = get_data_dir().join("instances");
    fs::create_dir_all(&instances_dir)?;
    let mut instances = Vec::new();
    if let Ok(entries) = fs::read_dir(&instances_dir) {
        for entry in entries.flatten() {
            let inst_dir = entry.path();
            let path = inst_dir.join("instance.json");
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut conf) = serde_json::from_str::<InstanceConfig>(&content) {
                    conf.icon = resolve_instance_icon(&inst_dir, &conf.icon);
                    instances.push(conf);
                }
            }
        }
    }
    Ok(instances)
}

pub fn get_by_id(id: &str) -> Result<InstanceConfig> {
    let json_path = get_data_dir().join("instances").join(id).join("instance.json");
    let content = fs::read_to_string(&json_path)?;
    serde_json::from_str(&content).map_err(|e| Error::Custom(format!("Ошибка чтения сборки: {}", e)))
}

pub fn sanitize_filename(name: &str) -> String {
    let mut safe = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
            safe.push(c);
        }
    }
    let res = safe.trim().replace(' ', "_");
    if res.is_empty() {
        "instance".to_string()
    } else {
        res
    }
}

pub fn create(name: &str, game_version: &str, loader: &str, loader_version: &str, icon_src: Option<&str>) -> Result<String> {
    let base_name = sanitize_filename(name);
    let mut folder_name = base_name.clone();
    let mut inst_dir = get_data_dir().join("instances").join(&folder_name);
    let mut counter = 2;
    while inst_dir.exists() {
        folder_name = format!("{}_{}", base_name, counter);
        inst_dir = get_data_dir().join("instances").join(&folder_name);
        counter += 1;
    }
    fs::create_dir_all(&inst_dir)?;

    let mut icon_path = String::new();
    if let Some(src) = icon_src {
        if !src.is_empty() {
            let src_p = std::path::Path::new(src);
            if src_p.exists() {
                let ext = src_p.extension().and_then(|e| e.to_str()).unwrap_or("png");
                let dest = inst_dir.join(format!("icon.{}", ext));
                let _ = fs::copy(src_p, &dest);
                icon_path = normalize_existing_path(dest);
            }
        }
    }

    let conf = InstanceConfig {
        id: folder_name.clone(),
        name: name.to_string(),
        game_version: game_version.to_string(),
        loader: loader.to_string(),
        loader_version: loader_version.to_string(),
        icon: icon_path,
        settings: None,
        playtime: 0,
    };
    fs::write(
        inst_dir.join("instance.json"),
        serde_json::to_string_pretty(&conf)?,
    )?;
    Ok(folder_name)
}

pub async fn delete(id: &str) -> Result<()> {
    // Останавливаем запущенный экземпляр
    crate::core::game::launch::stop_instance(id);
    sleep(Duration::from_millis(1000)).await;
    let inst_dir = get_data_dir().join("instances").join(id);
    if inst_dir.exists() {
        for _ in 0..5 {
            sleep(Duration::from_millis(400)).await;
            if tokio::fs::remove_dir_all(&inst_dir).await.is_ok() {
                return Ok(());
            }
        }
        tokio::fs::remove_dir_all(&inst_dir).await?;
    }
    Ok(())
}

/// Добавляет секунды наигранного времени к `instance.json` (после сессии).
pub fn add_playtime(id: &str, seconds: u64) -> Result<()> {
    if seconds == 0 {
        return Ok(());
    }
    let json_path = get_data_dir().join("instances").join(id).join("instance.json");
    if !json_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&json_path)?;
    let mut conf: InstanceConfig = serde_json::from_str(&content)
        .map_err(|e| Error::Custom(format!("instance.json: {}", e)))?;
    conf.playtime = conf.playtime.saturating_add(seconds);
    fs::write(json_path, serde_json::to_string_pretty(&conf)?)?;
    Ok(())
}

pub fn update_core(id: &str, game_version: &str, loader: &str, loader_version: &str) -> Result<()> {
    let json_path = get_data_dir()
        .join("instances")
        .join(id)
        .join("instance.json");
    if json_path.exists() {
        let content = fs::read_to_string(&json_path)?;
        if let Ok(mut conf) = serde_json::from_str::<InstanceConfig>(&content) {
            conf.game_version = game_version.to_string();
            conf.loader = loader.to_string();
            conf.loader_version = loader_version.to_string();
            fs::write(json_path, serde_json::to_string_pretty(&conf)?)?;
            return Ok(());
        }
    }
    Err(Error::Custom("Сборка не найдена".into()))
}

pub async fn repair_core(id: &str) -> Result<String> {
    let data_dir = get_data_dir();
    let json_path = data_dir.join("instances").join(id).join("instance.json");
    if let Ok(content) = fs::read_to_string(&json_path) {
        if let Ok(conf) = serde_json::from_str::<InstanceConfig>(&content) {
            let versions_dir = data_dir.join("versions");
            if let Ok(entries) = fs::read_dir(&versions_dir) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if conf.loader == "vanilla" {
                        if name == conf.game_version {
                            let _ = fs::remove_dir_all(e.path());
                        }
                    } else if name.starts_with(&conf.game_version) && name.contains(&conf.loader) {
                        let _ = fs::remove_dir_all(e.path());
                    }
                }
            }
            return Ok("Ядро очищено! Нажмите ИГРАТЬ для чистой установки.".into());
        }
    }
    Err(Error::Custom("Ошибка чтения сборки".into()))
}

pub fn open_folder(id: &str) {
    let path = get_data_dir().join("instances").join(id);
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
}

pub fn list_folders(id: &str) -> Result<Vec<String>> {
    let inst_dir = get_data_dir().join("instances").join(id);
    let mut folders = Vec::new();
    if let Ok(entries) = fs::read_dir(&inst_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with('.') && name != "crash-reports" {
                    folders.push(name);
                }
            }
        }
    }
    folders.sort();
    Ok(folders)
}

pub fn rename_instance(id: &str, new_name: &str) -> Result<()> {
    let json_path = get_data_dir().join("instances").join(id).join("instance.json");
    if json_path.exists() {
        let content = fs::read_to_string(&json_path)?;
        if let Ok(mut conf) = serde_json::from_str::<InstanceConfig>(&content) {
            conf.name = new_name.to_string();
            fs::write(json_path, serde_json::to_string_pretty(&conf)?)?;
            return Ok(());
        }
    }
    Err(Error::Custom("Сборка не найдена".into()))
}

pub fn unlink_modpack(id: &str) -> Result<()> {
    let inst_dir = get_data_dir().join("instances").join(id);
    let pack_source = inst_dir.join("pack_source.json");
    if pack_source.exists() {
        fs::remove_file(pack_source)?;
    }
    Ok(())
}

pub fn save_settings(id: &str, settings: InstanceSettings) -> Result<()> {
    let json_path = get_data_dir()
        .join("instances")
        .join(id)
        .join("instance.json");
    if json_path.exists() {
        let content = fs::read_to_string(&json_path)?;
        if let Ok(mut conf) = serde_json::from_str::<InstanceConfig>(&content) {
            conf.settings = Some(settings);
            fs::write(json_path, serde_json::to_string_pretty(&conf)?)?;
            return Ok(());
        }
    }
    Err(Error::Custom("Сборка не найдена".into()))
}