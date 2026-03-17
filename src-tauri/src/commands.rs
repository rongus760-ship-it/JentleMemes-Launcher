use tauri::AppHandle;
use serde_json::Value;

// ================= НАСТРОЙКИ И ИНСТАНСЫ =================
#[tauri::command] 
pub async fn load_settings() -> std::result::Result<serde_json::Value, String> { 
    let settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    serde_json::to_value(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_settings(payload: Value) -> std::result::Result<(), String> {
    let s = if let Some(settings) = payload.get("settings") {
        settings.clone()
    } else {
        payload
    };
    if let Ok(ls) = serde_json::from_value::<crate::config::LauncherSettings>(s) {
        crate::config::save_settings(&ls).map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command] pub async fn get_backgrounds() -> std::result::Result<Vec<String>, String> { Ok(vec![]) }

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
#[tauri::command] pub async fn create_instance(name: String, game_version: String, loader: String, loader_version: String) -> std::result::Result<String, String> { crate::core::instance::create(&name, &game_version, &loader, &loader_version).map_err(|e| e.to_string()) }
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

// ================= MODRINTH & MRPACK =================
#[tauri::command] pub async fn search_modrinth(query: String, project_type: String, game_version: String, loader: String, categories: Vec<String>, page: usize) -> std::result::Result<Value, String> { crate::core::modrinth::search(&query, &project_type, &game_version, &loader, categories, page).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_modrinth_project(id: String) -> std::result::Result<Value, String> { crate::core::modrinth::get_project(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_modrinth_versions(id: String) -> std::result::Result<Value, String> { crate::core::modrinth::get_versions(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn search_curseforge(query: String, project_type: String, game_version: String, loader: String, categories: Vec<String>, page: usize) -> std::result::Result<Value, String> { crate::core::curseforge::search(&query, &project_type, &game_version, &loader, categories, page).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_curseforge_project(id: String) -> std::result::Result<Value, String> { crate::core::curseforge::get_project(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_curseforge_versions(id: String) -> std::result::Result<Value, String> { crate::core::curseforge::get_versions(&id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn search_hybrid(query: String, project_type: String, game_version: String, loader: String, categories: Vec<String>, page: usize) -> std::result::Result<Value, String> { crate::core::curseforge::search_hybrid(&query, &project_type, &game_version, &loader, categories, page).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_hybrid_versions(modrinth_id: Option<String>, curseforge_id: Option<String>) -> std::result::Result<Value, String> { crate::core::curseforge::get_hybrid_versions(modrinth_id, curseforge_id).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn install_modrinth_file(app: AppHandle, instance_id: String, url: String, filename: String, project_type: String) -> std::result::Result<String, String> { crate::core::modrinth::install_file(Some(&app), &instance_id, &url, &filename, &project_type).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn install_mrpack_from_url(app: AppHandle, url: String, name: String) -> std::result::Result<String, String> { crate::core::mrpack::install_from_url(app, &url, &name).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn install_mrpack(app: AppHandle, file_path: String, name: String) -> std::result::Result<String, String> { crate::core::mrpack::install(app, &file_path, &name, None).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn export_instance(id: String, selected_folders: Vec<String>) -> std::result::Result<String, String> { crate::core::mrpack::export(&id, selected_folders).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn export_mrpack(id: String, selected_folders: Vec<String>) -> std::result::Result<String, String> { crate::core::mrpack::export_mrpack(&id, selected_folders).map_err(|e| e.to_string()) }
#[tauri::command] pub async fn import_instance() -> std::result::Result<String, String> { crate::core::mrpack::import().map_err(|e| e.to_string()) }
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
pub fn add_recent_server(ip: String, name: String) -> std::result::Result<(), String> {
    let mut settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    let ip = ip.trim().to_string();
    if ip.is_empty() { return Err("IP не может быть пустым".into()); }
    let name = if name.trim().is_empty() { ip.clone() } else { name.trim().to_string() };
    if let Some(s) = settings.recent_servers.iter_mut().find(|s| s.ip == ip) {
        s.name = name;
    } else {
        settings.recent_servers.push(crate::config::RecentServer {
            ip: ip.clone(),
            name,
            last_played: 0,
            playtime_hours: 0.0,
            last_instance_id: None,
            last_instance_name: None,
        });
    }
    // Храним только 2 последних (по last_played)
    settings.recent_servers.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    settings.recent_servers.truncate(2);
    crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

/// Загружает серверы: recent_servers из настроек + servers.dat, без дубликатов по IP
#[tauri::command]
pub fn load_servers() -> std::result::Result<serde_json::Value, String> {
    let settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    let mut by_ip: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
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
    for s in crate::core::servers::collect_servers_from_dat() {
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
    // Только 2 последних сервера без дублей
    list.truncate(2);
    Ok(serde_json::json!(list))
}

/// Импортирует серверы из servers.dat в настройки
#[tauri::command]
pub fn import_servers_from_dat() -> std::result::Result<usize, String> {
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
    let mut settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let instance_name = instance_id.as_ref().and_then(|id| {
        crate::core::instance::get_all().ok().and_then(|instances| {
            instances.into_iter().find(|i| i.id == *id).map(|i| i.name)
        })
    });
    if let Some(srv) = settings.recent_servers.iter_mut().find(|s| s.ip == ip) {
        srv.last_played = now;
        srv.name = if name.is_empty() { srv.name.clone() } else { name };
        srv.last_instance_id = instance_id;
        srv.last_instance_name = instance_name;
    } else {
        settings.recent_servers.push(crate::config::RecentServer {
            ip: ip.clone(),
            name: if name.is_empty() { ip } else { name },
            last_played: now,
            playtime_hours: 0.0,
            last_instance_id: instance_id,
            last_instance_name: instance_name,
        });
    }
    // Храним только 2 последних сервера (по last_played)
    settings.recent_servers.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    settings.recent_servers.truncate(2);
    crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_last_world(instance_id: String, world_name: String) -> std::result::Result<(), String> {
    let mut settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let instance_name = crate::core::instance::get_all()
        .ok()
        .and_then(|instances| instances.into_iter().find(|i| i.id == instance_id))
        .map(|i| i.name)
        .unwrap_or_else(|| instance_id.clone());
    settings.last_world = Some(crate::config::LastWorldEntry {
        instance_id,
        instance_name,
        world_name,
        last_played: now,
    });
    crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_last_world() -> std::result::Result<serde_json::Value, String> {
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
#[tauri::command] pub fn get_system_ram() -> u64 { crate::core::utils::system::get_system_ram() }
#[tauri::command] pub async fn ping_server(ip: String) -> std::result::Result<Value, String> { crate::core::utils::system::ping_server(&ip).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_loader_versions(loader: String) -> std::result::Result<Vec<String>, String> { crate::core::utils::system::get_loader_versions(&loader).await.map_err(|e| e.to_string()) }
#[tauri::command] pub async fn get_specific_loader_versions(loader: String, game_version: String) -> std::result::Result<Vec<String>, String> { crate::core::utils::system::get_specific_loader_versions(&loader, &game_version).await.map_err(|e| e.to_string()) }