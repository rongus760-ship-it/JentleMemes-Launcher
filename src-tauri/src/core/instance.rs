use crate::config::{get_data_dir, InstanceConfig, InstanceSettings};
use crate::core::api;
use crate::error::{Error, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::sleep;

static INSTANCE_LOCKS: Lazy<Mutex<HashMap<String, std::sync::Arc<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn instance_lock(id: &str) -> std::sync::Arc<Mutex<()>> {
    let mut map = INSTANCE_LOCKS.lock().unwrap();
    map.entry(id.to_string())
        .or_insert_with(|| std::sync::Arc::new(Mutex::new(())))
        .clone()
}

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
    let ex = existing_icon.trim();
    if !ex.is_empty() {
        if ex.starts_with("http://") || ex.starts_with("https://") {
            return ex.to_string();
        }
        let p = std::path::Path::new(ex);
        if p.is_absolute() && p.exists() {
            return normalize_existing_path(p.to_path_buf());
        }
        let rel = inst_dir.join(ex);
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

pub fn create(
    name: &str,
    game_version: &str,
    loader: &str,
    loader_version: &str,
    icon_src: Option<&str>,
) -> Result<String> {
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
    let inst_path = inst_dir.join("instance.json");
    crate::core::utils::atomic_fs::write_atomic_string(
        &inst_path,
        &serde_json::to_string_pretty(&conf)?,
    )?;
    Ok(folder_name)
}

pub async fn delete(id: &str) -> Result<()> {
    // Останавливаем запущенный экземпляр. Ошибка «нет такого процесса» — ожидаемая,
    // игнорируем; любую другую тоже не прокидываем, чтобы delete не блокировался.
    let _ = crate::core::game::launch::stop_instance(id);
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
    let _lock = instance_lock(id);
    let _guard = _lock.lock().unwrap();
    let json_path = get_data_dir()
        .join("instances")
        .join(id)
        .join("instance.json");
    if !json_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&json_path)?;
    let mut conf: InstanceConfig = serde_json::from_str(&content)
        .map_err(|e| Error::Custom(format!("instance.json: {}", e)))?;
    conf.playtime = conf.playtime.saturating_add(seconds);
    let tmp = json_path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&conf)?)?;
    fs::rename(&tmp, &json_path)?;
    Ok(())
}

pub fn update_core(id: &str, game_version: &str, loader: &str, loader_version: &str) -> Result<()> {
    let _lock = instance_lock(id);
    let _guard = _lock.lock().unwrap();
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
            let tmp = json_path.with_extension("json.tmp");
            fs::write(&tmp, serde_json::to_string_pretty(&conf)?)?;
            fs::rename(&tmp, &json_path)?;
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
            let gv = conf.game_version.trim();
            if conf.loader == "vanilla" {
                if !gv.is_empty() {
                    let _ = fs::remove_dir_all(versions_dir.join(gv));
                }
            } else {
                // Как в install_loader: `neoforge/26.1.2.7-beta-1.21.6` и легаси `neoforge-…`.
                let lv = api::normalize_loader_version(&conf.loader_version);
                if !gv.is_empty() && !lv.is_empty() {
                    let target_id =
                        crate::core::game::version_layout::modded_profile_id(&conf.loader, &lv, gv);
                    let _ = fs::remove_dir_all(crate::core::game::version_layout::profile_dir(
                        &data_dir, &target_id,
                    ));
                    let legacy_id = format!("{}-{}-{}", conf.loader, lv, gv);
                    let _ = fs::remove_dir_all(versions_dir.join(&legacy_id));
                    let old_slug_dir = versions_dir.join(&conf.loader).join(format!("{lv}-{gv}"));
                    let _ = fs::remove_dir_all(&old_slug_dir);
                }
                if let Ok(entries) = fs::read_dir(&versions_dir) {
                    for e in entries.flatten() {
                        let name = e.file_name().to_string_lossy().to_string();
                        if gv.is_empty() {
                            continue;
                        }
                        // Легаси-имена вроде «1.20.1-forge-…»
                        if name.starts_with(gv) && name.contains(&conf.loader) {
                            let _ = fs::remove_dir_all(e.path());
                        }
                    }
                }
            }
            return Ok("Ядро очищено! Нажмите ИГРАТЬ для чистой установки.".into());
        }
    }
    Err(Error::Custom("Ошибка чтения сборки".into()))
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
    let json_path = get_data_dir()
        .join("instances")
        .join(id)
        .join("instance.json");
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
