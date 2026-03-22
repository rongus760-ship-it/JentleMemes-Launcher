use tauri::AppHandle;
use serde_json::Value;

// ================= НАСТРОЙКИ И ИНСТАНСЫ =================
#[tauri::command] 
pub async fn load_settings() -> std::result::Result<serde_json::Value, String> { 
    let settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    serde_json::to_value(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_settings(settings: Value) -> std::result::Result<(), String> {
    let ls: crate::config::LauncherSettings = serde_json::from_value(settings)
        .map_err(|e| format!("Invalid settings: {e}"))?;
    crate::config::save_settings(&ls).map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
pub async fn get_backgrounds() -> std::result::Result<Vec<String>, String> {
    let bg_dir = crate::config::get_data_dir().join("backgrounds");
    let _ = std::fs::create_dir_all(&bg_dir);
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&bg_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.to_lowercase().as_str()) {
                    files.push(path.to_string_lossy().to_string());
                }
            }
        }
    }
    Ok(files)
}

#[tauri::command]
pub async fn pick_image_file() -> std::result::Result<Option<String>, String> {
    let result = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
            .set_title("Выберите изображение для фона")
            .pick_file()
    }).await.map_err(|e| e.to_string())?;
    Ok(result.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn copy_background(source_path: String) -> std::result::Result<String, String> {
    let bg_dir = crate::config::get_data_dir().join("backgrounds");
    std::fs::create_dir_all(&bg_dir).map_err(|e| e.to_string())?;
    let src = std::path::Path::new(&source_path);
    let name = src.file_name().ok_or("invalid filename")?;
    let dest = bg_dir.join(name);
    std::fs::copy(src, &dest).map_err(|e| format!("Copy failed: {}", e))?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn delete_background(path: String) -> std::result::Result<(), String> {
    let p = std::path::Path::new(&path);
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_data_dir() -> std::result::Result<String, String> {
    Ok(crate::config::get_data_dir().to_string_lossy().to_string())
}

// URL кастомных сборок встроен в лаунчер (файл src-tauri/custom_packs.json)
const DEFAULT_CUSTOM_PACKS: &str = include_str!("../custom_packs.json");

#[tauri::command]
pub async fn load_custom_packs_config() -> std::result::Result<serde_json::Value, String> {
    match serde_json::from_str::<Value>(DEFAULT_CUSTOM_PACKS) {
        Ok(json) => Ok(json),
        Err(_) => Ok(serde_json::json!({ "url": "" })),
    }
}

#[tauri::command]
pub async fn fetch_custom_packs(url: String) -> std::result::Result<Value, String> {
    if url.trim().is_empty() {
        return Ok(serde_json::json!([]));
    }
    let res = reqwest::get(url.trim()).await.map_err(|e| e.to_string())?;
    let json: Value = res.json().await.map_err(|e| e.to_string())?;
    Ok(json)
}

/// Возвращает кэшированный список кастомных сборок (обновлён при старте лаунчера)
#[tauri::command]
pub fn get_custom_packs() -> std::result::Result<Value, String> {
    Ok(crate::custom_packs::get_cached_packs())
}

/// Принудительно перезагружает список кастомных сборок с удалённого URL
#[tauri::command]
pub async fn refresh_custom_packs() -> std::result::Result<Value, String> {
    crate::custom_packs::fetch_and_cache_packs().await.map_err(|e| e.to_string())
}

#[tauri::command] pub async fn get_instances() -> std::result::Result<Vec<crate::config::InstanceConfig>, String> { crate::core::instance::get_all().map_err(|e| e.to_string()) }
#[tauri::command] pub async fn create_instance(name: String, game_version: String, loader: String, loader_version: String, icon: Option<String>) -> std::result::Result<String, String> { crate::core::instance::create(&name, &game_version, &loader, &loader_version, icon.as_deref()).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn delete_instance(id: String) -> std::result::Result<(), String> { crate::core::instance::delete(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn update_instance_core(id: String, game_version: String, loader: String, loader_version: String) -> std::result::Result<(), String> { crate::core::instance::update_core(&id, &game_version, &loader, &loader_version).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn repair_core(id: String) -> std::result::Result<String, String> { crate::core::instance::repair_core(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub fn open_folder(id: String) { crate::core::instance::open_folder(&id); }
#[tauri::command] pub async fn list_instance_folders(id: String) -> std::result::Result<Vec<String>, String> { crate::core::instance::list_folders(&id).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn save_instance_settings(id: String, settings: crate::config::InstanceSettings) -> std::result::Result<(), String> { crate::core::instance::save_settings(&id, settings).map_err(|e| e.to_string()) }

// ================= МОДЫ И СБОРКИ =================
#[tauri::command] pub async fn get_installed_mods(instance_id: String) -> std::result::Result<Vec<crate::core::mods::ModInfo>, String> { crate::core::mods::get_installed(&instance_id).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn list_worlds(instance_id: String) -> std::result::Result<Vec<String>, String> {
    let saves_dir = crate::config::get_data_dir().join("instances").join(&instance_id).join("saves");
    let mut worlds = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&saves_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                worlds.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    Ok(worlds)
}
#[tauri::command] pub async fn install_datapack(instance_id: String, world_name: String, url: String, filename: String) -> std::result::Result<String, String> {
    let dp_dir = crate::config::get_data_dir().join("instances").join(&instance_id).join("saves").join(&world_name).join("datapacks");
    std::fs::create_dir_all(&dp_dir).map_err(|e| e.to_string())?;
    let bytes = reqwest::get(&url).await.map_err(|e| e.to_string())?.bytes().await.map_err(|e| e.to_string())?;
    std::fs::write(dp_dir.join(&filename), bytes).map_err(|e| e.to_string())?;
    Ok(format!("Датапак установлен в мир {}", world_name))
}
#[tauri::command] pub async fn refresh_mod_metadata(app: AppHandle, instance_id: String) -> std::result::Result<String, String> { crate::core::mods::build_metadata(&app, &instance_id).await.map_err(|e| e.to_string())?; Ok("Метаданные обновлены".into()) }
#[tauri::command] pub async fn get_installed_content(instance_id: String, folder: String) -> std::result::Result<Vec<crate::core::mods::ModInfo>, String> { crate::core::mods::get_installed_from_folder(&instance_id, &folder).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn toggle_mod(instance_id: String, filename: String, enable: bool, folder: Option<String>) -> std::result::Result<(), String> { let f = folder.as_deref().unwrap_or("mods"); crate::core::mods::toggle(&instance_id, f, &filename, enable).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn delete_mod(instance_id: String, filename: String, folder: Option<String>) -> std::result::Result<(), String> { let f = folder.as_deref().unwrap_or("mods"); crate::core::mods::delete(&instance_id, f, &filename).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn check_mod_updates(hashes: Vec<String>, loader: String, game_version: String) -> std::result::Result<Value, String> { crate::core::mods::check_updates(&hashes, &loader, &game_version).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn install_mod_with_dependencies(app: AppHandle, instance_id: String, version_id: String, game_version: String, loader: String) -> std::result::Result<String, String> {
    let download_deps = crate::config::load_settings().map(|s| s.download_dependencies).unwrap_or(true);
    crate::core::mods::install_with_dependencies(app, &instance_id, &version_id, &game_version, &loader, download_deps).await.map_err(|e| e.to_string())
}

// ================= АВТОРИЗАЦИЯ =================
#[tauri::command] 
pub async fn load_profiles() -> std::result::Result<serde_json::Value, String> { 
    let path = crate::config::get_data_dir().join("profiles.json");
    
    // Структура по умолчанию, если файла нет (чтобы React не крашился)
    let default_profiles = serde_json::json!({
        "accounts": [],
        "active_account_id": "",
        "skin_presets": []
    });

    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str(&content) {
                return Ok(json);
            }
        }
    }
    
    Ok(default_profiles)
}
#[tauri::command] pub async fn save_profiles(profiles: Value) -> std::result::Result<(), String> { std::fs::write(crate::config::get_data_dir().join("profiles.json"), serde_json::to_string(&profiles).unwrap()).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn login_offline(username: String) -> std::result::Result<crate::config::AccountInfo, String> { crate::core::auth::login_offline(&username).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn login_elyby(email: String, password: String) -> std::result::Result<crate::config::AccountInfo, String> { crate::core::auth::login_elyby(&email, &password).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn ms_init_device_code() -> std::result::Result<crate::config::DeviceCodeResponse, String> { crate::core::auth::ms_init_device_code().await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn ms_login_poll(device_code: String, interval: u64) -> std::result::Result<crate::config::AccountInfo, String> { crate::core::auth::ms_login_poll(&device_code, interval).await.map_err(|e| e.to_string()) }

/// При старте лаунчера обновляет токены Microsoft-аккаунтов (по refresh_token), чтобы не было «недействительная сессия».
#[tauri::command]
pub async fn refresh_microsoft_sessions_startup(app: tauri::AppHandle) -> std::result::Result<(), String> {
    use tauri::Emitter;
    let mut profiles = crate::config::load_profiles().map_err(|e| e.to_string())?;
    let mut changed = false;
    for acc in profiles.accounts.iter_mut() {
        if acc.acc_type != "microsoft" {
            continue;
        }
        match crate::core::auth::refresh_microsoft_account_on_startup(acc).await {
            Ok(new_acc) => {
                let differs = new_acc.token != acc.token
                    || new_acc.ms_refresh_token != acc.ms_refresh_token
                    || new_acc.username != acc.username
                    || new_acc.uuid != acc.uuid;
                *acc = new_acc;
                if differs {
                    changed = true;
                }
            }
            Err(e) => {
                eprintln!("[auth] не удалось обновить сессию {}: {}", acc.username, e);
            }
        }
    }
    if changed {
        crate::config::save_profiles(&profiles).map_err(|e| e.to_string())?;
        let _ = app.emit("profiles_updated", ());
    }
    Ok(())
}

// ================= MODRINTH & MRPACK =================
#[tauri::command] pub async fn search_modrinth(query: String, project_type: String, game_version: String, loader: String, categories: Vec<String>, page: usize, sort: String, sort_desc: Option<bool>) -> std::result::Result<Value, String> {
    let desc = sort_desc.unwrap_or(true);
    crate::core::modrinth::search(&query, &project_type, &game_version, &loader, categories, page, &sort, desc).await.map_err(|e| e.to_string())
}
#[tauri::command] pub async fn get_modrinth_project(id: String) -> std::result::Result<Value, String> { crate::core::modrinth::get_project(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_modrinth_versions(id: String) -> std::result::Result<Value, String> { crate::core::modrinth::get_versions(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn search_curseforge(query: String, project_type: String, game_version: String, loader: String, categories: Vec<String>, page: usize, sort: String, sort_desc: Option<bool>) -> std::result::Result<Value, String> {
    let desc = sort_desc.unwrap_or(true);
    crate::core::curseforge::search(&query, &project_type, &game_version, &loader, categories, page, &sort, desc).await.map_err(|e| e.to_string())
}
#[tauri::command] pub async fn get_curseforge_project(id: String) -> std::result::Result<Value, String> { crate::core::curseforge::get_project(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_curseforge_versions(id: String) -> std::result::Result<Value, String> { crate::core::curseforge::get_versions(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn search_hybrid(query: String, project_type: String, game_version: String, loader: String, categories: Vec<String>, page: usize, sort: String, sort_desc: Option<bool>) -> std::result::Result<Value, String> {
    let desc = sort_desc.unwrap_or(true);
    crate::core::curseforge::search_hybrid(&query, &project_type, &game_version, &loader, categories, page, &sort, desc).await.map_err(|e| e.to_string())
}
#[tauri::command] pub async fn get_hybrid_versions(modrinth_id: Option<String>, curseforge_id: Option<String>) -> std::result::Result<Value, String> { crate::core::curseforge::get_hybrid_versions(modrinth_id, curseforge_id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn install_modrinth_file(app: AppHandle, instance_id: String, url: String, filename: String, project_type: String) -> std::result::Result<String, String> { crate::core::modrinth::install_file(Some(&app), &instance_id, &url, &filename, &project_type).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn install_mrpack_from_url(app: AppHandle, url: String, name: String) -> std::result::Result<String, String> { crate::core::mrpack::install_from_url(app, &url, &name).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn install_mrpack(app: AppHandle, file_path: String, name: String) -> std::result::Result<String, String> { crate::core::mrpack::install(app, &file_path, &name, None).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn export_instance(id: String, selected_folders: Vec<String>) -> std::result::Result<String, String> { crate::core::mrpack::export(&id, selected_folders).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn export_mrpack(app: tauri::AppHandle, id: String, selected_folders: Vec<String>) -> std::result::Result<String, String> { crate::core::mrpack::export_mrpack_async(&app, &id, selected_folders).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn export_jentlepack(app: tauri::AppHandle, id: String, selected_folders: Vec<String>) -> std::result::Result<String, String> { crate::core::mrpack::export_jentlepack_async(&app, &id, selected_folders).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn import_instance(app: AppHandle) -> std::result::Result<crate::core::mrpack::ImportInstanceResult, String> { crate::core::mrpack::import_instance_packed(&app).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn check_modpack_update(instance_id: String) -> std::result::Result<Value, String> { crate::core::mrpack::check_modpack_update(&instance_id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn update_modpack(app: AppHandle, instance_id: String, update_url: String) -> std::result::Result<String, String> { crate::core::mrpack::update_modpack(app, &instance_id, &update_url).await.map_err(|e| e.to_string()) }

// ================= ИГРА (Установка и Запуск) =================
#[tauri::command] 
pub async fn fetch_vanilla_versions() -> std::result::Result<Value, String> { 
    // Возвращаем реальный список версий от Mojang, чтобы React был доволен
    let res = crate::core::api::HTTP_CLIENT
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send().await.map_err(|e| e.to_string())?
        .json::<Value>().await.map_err(|e| e.to_string())?;
    Ok(res)
}

#[tauri::command]
pub async fn download_game_files(app: AppHandle, version_id: String, instance_id: Option<String>) -> std::result::Result<String, String> {
    crate::core::game::install::download_game_files(app, &version_id, instance_id.as_deref()).await.map_err(|e| e.to_string())
}

#[tauri::command] 
pub async fn extract_natives(version_id: String) -> std::result::Result<String, String> { 
    crate::core::game::install::extract_natives(&version_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_version(version_id: String, _url: String) -> std::result::Result<String, String> {
    crate::core::game::install::install_vanilla(&version_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_fabric(version_id: String) -> std::result::Result<String, String> {
    let vers = crate::core::utils::system::get_specific_loader_versions("fabric", &version_id).await.unwrap_or_default();
    let loader_ver = vers.first().map(|s| s.as_str()).unwrap_or("0.15.7");
    crate::core::game::install::install_loader(&version_id, "fabric", loader_ver).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_quilt(version_id: String) -> std::result::Result<String, String> {
    let vers = crate::core::utils::system::get_specific_loader_versions("quilt", &version_id).await.unwrap_or_default();
    let loader_ver = vers.first().map(|s| s.as_str()).unwrap_or("0.24.0");
    crate::core::game::install::install_loader(&version_id, "quilt", loader_ver).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_forge(
    _app: AppHandle, _instance_id: String, game_version: String, 
    loader_version: String, loader_name: String
) -> std::result::Result<String, String> {
    crate::core::game::install::install_loader(&game_version, &loader_name, &loader_version).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_recent_server(ip: String, name: String, instance_id: Option<String>) -> std::result::Result<(), String> {
    let ip = ip.trim().to_string();
    if ip.is_empty() { return Err("IP не может быть пустым".into()); }
    let name = if name.trim().is_empty() { ip.clone() } else { name.trim().to_string() };
    let server = crate::config::RecentServer {
        ip: ip.clone(),
        name,
        last_played: 0,
        playtime_hours: 0.0,
        last_instance_id: None,
        last_instance_name: None,
    };
    if let Some(ref id) = instance_id {
        if !id.is_empty() {
            let inst_dir = crate::config::get_data_dir().join("instances").join(id);
            std::fs::create_dir_all(&inst_dir).map_err(|e| e.to_string())?;
            let mut servers = crate::config::load_instance_servers(&inst_dir).map_err(|e| e.to_string())?;
            if let Some(s) = servers.iter_mut().find(|s| s.ip == ip) {
                s.name = server.name;
            } else {
                servers.push(server);
            }
            servers.sort_by(|a, b| b.last_played.cmp(&a.last_played));
            servers.truncate(2);
            crate::config::save_instance_servers(&inst_dir, &servers).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    // Fallback: global settings
    let mut settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    if let Some(s) = settings.recent_servers.iter_mut().find(|s| s.ip == ip) {
        s.name = server.name;
    } else {
        settings.recent_servers.push(server);
    }
    settings.recent_servers.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    settings.recent_servers.truncate(2);
    crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

/// Загружает серверы: per-instance servers.json + servers.dat, или global при пустом instance_id
#[tauri::command]
pub fn load_servers(instance_id: Option<String>) -> std::result::Result<serde_json::Value, String> {
    let mut by_ip: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
    let from_dat: Vec<_> = if let Some(ref id) = instance_id {
        if !id.is_empty() {
            let inst_dir = crate::config::get_data_dir().join("instances").join(id);
            for s in crate::config::load_instance_servers(&inst_dir).map_err(|e| e.to_string())? {
                let ip = s.ip.clone();
                let mut obj = serde_json::json!({
                    "ip": s.ip,
                    "name": s.name,
                    "last_played": s.last_played,
                    "playtime_hours": s.playtime_hours,
                    "icon": serde_json::Value::Null,
                    "source": "settings"
                });
                if let Some(ref i) = s.last_instance_id { obj["last_instance_id"] = serde_json::json!(i); }
                if let Some(ref n) = s.last_instance_name { obj["last_instance_name"] = serde_json::json!(n); }
                by_ip.entry(ip).or_insert(obj);
            }
            crate::core::servers::collect_servers_from_instance_dat(id)
        } else {
            crate::core::servers::collect_servers_from_dat()
        }
    } else {
        crate::core::servers::collect_servers_from_dat()
    };
    if instance_id.as_deref().unwrap_or("").is_empty() {
        // Global fallback
        let settings = crate::config::load_settings().map_err(|e| e.to_string())?;
        for s in &settings.recent_servers {
            let ip = s.ip.clone();
            let mut obj = serde_json::json!({
                "ip": s.ip,
                "name": s.name,
                "last_played": s.last_played,
                "playtime_hours": s.playtime_hours,
                "icon": serde_json::Value::Null,
                "source": "settings"
            });
            if let Some(ref id) = s.last_instance_id { obj["last_instance_id"] = serde_json::json!(id); }
            if let Some(ref name) = s.last_instance_name { obj["last_instance_name"] = serde_json::json!(name); }
            by_ip.entry(ip).or_insert(obj);
        }
    }
    for s in from_dat {
        by_ip.entry(s.ip.clone()).or_insert_with(|| serde_json::json!({
            "ip": s.ip,
            "name": s.name,
            "last_played": 0,
            "playtime_hours": 0,
            "icon": s.icon,
            "source": "servers_dat"
        }));
    }
    let mut list: Vec<serde_json::Value> = by_ip.into_values().collect();
    list.sort_by(|a, b| {
        let pa = a.get("last_played").and_then(|v| v.as_u64()).unwrap_or(0);
        let pb = b.get("last_played").and_then(|v| v.as_u64()).unwrap_or(0);
        pb.cmp(&pa)
    });
    list.truncate(2);
    Ok(serde_json::json!(list))
}

/// Импортирует серверы из servers.dat в настройки (глобальные или per-instance)
#[tauri::command]
pub fn import_servers_from_dat(instance_id: Option<String>) -> std::result::Result<usize, String> {
    if let Some(ref id) = instance_id {
        if !id.is_empty() {
            let inst_dir = crate::config::get_data_dir().join("instances").join(id);
            std::fs::create_dir_all(&inst_dir).map_err(|e| e.to_string())?;
            let mut servers = crate::config::load_instance_servers(&inst_dir).map_err(|e| e.to_string())?;
            let mut seen: std::collections::HashSet<String> = servers.iter().map(|s| s.ip.clone()).collect();
            let from_dat = crate::core::servers::collect_servers_from_instance_dat(id);
            let mut added = 0;
            for s in from_dat {
                if !seen.contains(&s.ip) {
                    seen.insert(s.ip.clone());
                    servers.push(crate::config::RecentServer {
                        ip: s.ip,
                        name: s.name,
                        last_played: 0,
                        playtime_hours: 0.0,
                        last_instance_id: None,
                        last_instance_name: None,
                    });
                    added += 1;
                }
            }
            if added > 0 {
                crate::config::save_instance_servers(&inst_dir, &servers).map_err(|e| e.to_string())?;
            }
            return Ok(added);
        }
    }
    // Fallback: global settings
    let mut settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    let mut seen: std::collections::HashSet<String> = settings.recent_servers.iter().map(|s| s.ip.clone()).collect();
    let from_dat = crate::core::servers::collect_servers_from_dat();
    let mut added = 0;
    for s in from_dat {
        if !seen.contains(&s.ip) {
            seen.insert(s.ip.clone());
            settings.recent_servers.push(crate::config::RecentServer {
                ip: s.ip,
                name: s.name,
                last_played: 0,
                playtime_hours: 0.0,
                last_instance_id: None,
                last_instance_name: None,
            });
            added += 1;
        }
    }
    if added > 0 {
        crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    }
    Ok(added)
}

#[tauri::command]
pub fn update_server_last_played(ip: String, name: String, instance_id: Option<String>) -> std::result::Result<(), String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let instance_name = instance_id.as_ref().and_then(|id| {
        crate::core::instance::get_all().ok().and_then(|instances| {
            instances.into_iter().find(|i| i.id == *id).map(|i| i.name)
        })
    });
    let new_srv = crate::config::RecentServer {
        ip: ip.clone(),
        name: if name.is_empty() { ip.clone() } else { name },
        last_played: now,
        playtime_hours: 0.0,
        last_instance_id: instance_id.clone(),
        last_instance_name: instance_name,
    };
    if let Some(ref id) = instance_id {
        if !id.is_empty() {
            let inst_dir = crate::config::get_data_dir().join("instances").join(id);
            std::fs::create_dir_all(&inst_dir).map_err(|e| e.to_string())?;
            let mut servers = crate::config::load_instance_servers(&inst_dir).map_err(|e| e.to_string())?;
            if let Some(srv) = servers.iter_mut().find(|s| s.ip == ip) {
                srv.last_played = now;
                srv.name = new_srv.name.clone();
                srv.last_instance_id = instance_id;
                srv.last_instance_name = new_srv.last_instance_name.clone();
            } else {
                servers.push(new_srv);
            }
            servers.sort_by(|a, b| b.last_played.cmp(&a.last_played));
            servers.truncate(2);
            crate::config::save_instance_servers(&inst_dir, &servers).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    // Fallback: global settings
    let mut settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    if let Some(srv) = settings.recent_servers.iter_mut().find(|s| s.ip == ip) {
        srv.last_played = now;
        srv.name = new_srv.name;
        srv.last_instance_id = instance_id;
        srv.last_instance_name = new_srv.last_instance_name;
    } else {
        settings.recent_servers.push(new_srv);
    }
    settings.recent_servers.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    settings.recent_servers.truncate(2);
    crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_last_world(instance_id: String, world_name: String) -> std::result::Result<(), String> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let instance_name = crate::core::instance::get_all()
        .ok()
        .and_then(|instances| instances.into_iter().find(|i| i.id == instance_id))
        .map(|i| i.name)
        .unwrap_or_else(|| instance_id.clone());
    let entry = crate::config::LastWorldEntry {
        instance_id: instance_id.clone(),
        instance_name,
        world_name,
        last_played: now,
    };
    if !instance_id.is_empty() {
        let inst_dir = crate::config::get_data_dir().join("instances").join(&instance_id);
        std::fs::create_dir_all(&inst_dir).map_err(|e| e.to_string())?;
        crate::config::save_instance_last_world(&inst_dir, &entry).map_err(|e| e.to_string())?;
        return Ok(());
    }
    // Fallback: global settings
    let mut settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    settings.last_world = Some(entry);
    crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_last_world(instance_id: Option<String>) -> std::result::Result<serde_json::Value, String> {
    if let Some(ref id) = instance_id {
        if !id.is_empty() {
            let inst_dir = crate::config::get_data_dir().join("instances").join(id);
            let world = crate::config::load_instance_last_world(&inst_dir).map_err(|e| e.to_string())?;
            return Ok(serde_json::to_value(world.as_ref()).unwrap_or(serde_json::Value::Null));
        }
    }
    // Fallback: global settings
    let settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(settings.last_world.as_ref()).unwrap_or(serde_json::Value::Null))
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle, instance_id: String, version_id: String, username: String,
    uuid: String, token: String, acc_type: String, server_ip: String, world_name: Option<String>
) -> std::result::Result<String, String> {
    if let Some(ref w) = world_name {
        let _ = update_last_world(instance_id.clone(), w.clone());
    }
    let server = if server_ip.is_empty() { None } else { Some(server_ip.as_str()) };
    let world = world_name.as_deref();
    crate::core::game::launch::launch(
        app, &instance_id, &version_id, &username, &uuid, &token, &acc_type, server, world
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_instance(instance_id: String) {
    let _ = crate::core::game::launch::stop_instance(&instance_id);
}

#[tauri::command]
pub fn rename_instance(id: String, new_name: String) -> std::result::Result<(), String> {
    crate::core::instance::rename_instance(&id, &new_name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn unlink_modpack(id: String) -> std::result::Result<(), String> {
    crate::core::instance::unlink_modpack(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_pack_source_info(instance_id: String) -> std::result::Result<Option<serde_json::Value>, String> {
    let inst_dir = crate::config::get_data_dir().join("instances").join(&instance_id);
    let source = crate::config::load_pack_source(&inst_dir).map_err(|e| e.to_string())?;
    let json = match source {
        Some(crate::config::PackSource::Modrinth { project_id, version_id, version_name, .. }) => {
            let mut j = serde_json::json!({ "source": "modrinth", "project_id": project_id, "version_id": version_id, "version_name": version_name });
            if let Ok(proj) = crate::core::modrinth::get_project(&project_id).await {
                if let Some(url) = proj.get("icon_url").and_then(|v| v.as_str()) {
                    j["icon_url"] = serde_json::json!(url);
                }
            }
            j
        }
        Some(crate::config::PackSource::Custom { pack_url, .. }) => {
            serde_json::json!({ "source": "custom", "pack_url": pack_url })
        }
        None => return Ok(None),
    };
    Ok(Some(json))
}

// ================= УТИЛИТЫ =================
#[tauri::command]
pub async fn fetch_launcher_news() -> std::result::Result<Vec<crate::core::updater::NewsItem>, String> {
    crate::core::updater::fetch_news().await
}

#[tauri::command]
pub async fn check_launcher_update() -> std::result::Result<serde_json::Value, String> {
    match crate::core::updater::check_update().await? {
        Some(info) => Ok(serde_json::json!({
            "available": true,
            "latest": info.version,
            "current": crate::core::updater::CURRENT_VERSION,
            "changelog": info.changelog,
            "release": info.platforms.get(
                if cfg!(target_os = "windows") { "windows" } else { "linux" }
            ),
        })),
        None => Ok(serde_json::json!({
            "available": false,
            "current": crate::core::updater::CURRENT_VERSION,
        })),
    }
}

#[tauri::command]
pub async fn download_and_apply_update() -> std::result::Result<String, String> {
    let info = crate::core::updater::check_update().await?
        .ok_or("No update available")?;
    let key = if cfg!(target_os = "windows") { "windows" } else { "linux" };
    let release = info.platforms.get(key)
        .ok_or("No release for this platform")?;
    let path = crate::core::updater::download_update(release).await?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(&path).spawn().map_err(|e| e.to_string())?;
        std::process::exit(0);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let current = std::env::current_exe().map_err(|e| e.to_string())?;
        std::fs::copy(&path, &current).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&current, std::fs::Permissions::from_mode(0o755));
        }
        std::process::Command::new(&current).spawn().map_err(|e| e.to_string())?;
        std::process::exit(0);
    }
}

#[tauri::command] pub fn get_system_ram() -> u64 { crate::core::utils::system::get_system_ram() }
#[tauri::command] pub async fn ping_server(ip: String) -> std::result::Result<Value, String> { crate::core::utils::system::ping_server(&ip).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_loader_versions(loader: String, include_snapshots: Option<bool>) -> std::result::Result<Vec<String>, String> { crate::core::utils::system::get_loader_versions(&loader, include_snapshots).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_specific_loader_versions(loader: String, game_version: String) -> std::result::Result<Vec<String>, String> { crate::core::utils::system::get_specific_loader_versions(&loader, &game_version).await.map_err(|e| e.to_string()) }

// ================= УПРАВЛЕНИЕ ОКНОМ =================
#[tauri::command]
pub fn window_close(app: AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        w.close().ok();
    } else {
        app.exit(0);
    }
}

#[tauri::command]
pub fn window_minimize(app: AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        w.minimize().ok();
    }
}

#[tauri::command]
pub fn window_maximize(app: AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        if w.is_maximized().unwrap_or(false) {
            w.unmaximize().ok();
        } else {
            w.maximize().ok();
        }
    }
}

#[tauri::command]
pub fn window_drag(app: AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        w.start_dragging().ok();
    }
}

#[tauri::command]
pub fn window_is_maximized(app: AppHandle) -> bool {
    use tauri::Manager;
    app.get_webview_window("main")
        .and_then(|w| w.is_maximized().ok())
        .unwrap_or(false)
}