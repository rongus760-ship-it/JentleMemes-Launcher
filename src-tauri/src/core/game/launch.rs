use tauri::{AppHandle, Emitter};
use serde_json::Value;
use crate::error::{Result, Error};
use crate::core::types::VersionInfo;
use crate::core::utils::maven::maven_to_path;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};
use std::thread;
use once_cell::sync::Lazy;

/// Evaluates Mojang library rules. Returns true if the library should be
/// included on the current OS. When no rules are present, the library is allowed.
fn check_rules(rules: &Option<Vec<Value>>, current_os: &str) -> bool {
    let Some(rules) = rules else { return true; };
    if rules.is_empty() { return true; }

    let mut dominated_allow = false;
    let mut dominated_disallow = false;

    for rule in rules {
        let action = rule.get("action").and_then(|v| v.as_str()).unwrap_or("allow");
        let os_match = if let Some(os) = rule.get("os").and_then(|v| v.as_object()) {
            os.get("name").and_then(|v| v.as_str()).map_or(true, |n| n == current_os)
        } else {
            true // no OS restriction → matches all
        };

        let is_os_specific = rule.get("os").is_some();

        if action == "allow" {
            if os_match {
                if is_os_specific { return true; }
                dominated_allow = true;
            }
        } else if action == "disallow" {
            if os_match {
                dominated_disallow = true;
            }
        }
    }

    dominated_allow && !dominated_disallow
}

/// Хранит запущенные процессы по instance_id для возможности остановки
static RUNNING: Lazy<Mutex<HashMap<String, Arc<Mutex<Option<Child>>>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn launch(
    app: AppHandle, instance_id: &str, version_id: &str, username: &str, 
    uuid: &str, token: &str, acc_type: &str, server_ip: Option<&str>, world_name: Option<&str>,
) -> Result<String> {
    let data_dir = crate::config::get_data_dir();
    let settings = crate::config::load_settings().unwrap_or_default();
    let game_dir = data_dir.join("instances").join(instance_id);
    let natives_dir = data_dir.join("versions").join(version_id).join("natives");
    let lib_dir = data_dir.join("libraries");

    std::fs::create_dir_all(&game_dir)?;
    
    let log = |msg: &str| {
        let _ = app.emit(&format!("log_{}", instance_id), format!("[JentleMemes] {}", msg));
    };

    log("Начинаем сборку профилей...");

    let mut current_id = version_id.to_string();
    let mut all_v_infos = Vec::new();
    loop {
        let v_path = data_dir.join("versions").join(&current_id).join(format!("{}.json", current_id));
        if !v_path.exists() { break; }
        let v_info: VersionInfo = serde_json::from_str(&std::fs::read_to_string(&v_path)?)?;
        let parent = v_info.inherits_from.clone();
        all_v_infos.push(v_info);
        if let Some(p) = parent { current_id = p; } else { break; }
    }
    
    all_v_infos.reverse();
    if all_v_infos.is_empty() { return Err(Error::Custom("Профили не найдены".into())); }

    let main_v_info = all_v_infos.last().unwrap().clone();
    let vanilla_info = all_v_infos.first().unwrap().clone();

    let lib_dir_str = lib_dir.to_string_lossy().replace('\\', "/");
    let natives_dir_str = natives_dir.to_string_lossy().replace('\\', "/");
    let game_dir_str = game_dir.to_string_lossy().replace('\\', "/");
    let assets_dir_str = data_dir.join("assets").to_string_lossy().replace('\\', "/");
    let asset_index = main_v_info.assets.clone().or_else(|| vanilla_info.assets.clone()).unwrap_or_else(|| "legacy".into());

    let jar_id = main_v_info.inherits_from.as_deref().unwrap_or(version_id);
    let client_jar_path = data_dir.join("versions").join(jar_id).join(format!("{}.jar", jar_id)).to_string_lossy().replace('\\', "/");

    let custom_installer = data_dir.join("versions").join(version_id).join("installer.jar");
    let forge_installer_path = if custom_installer.exists() {
        custom_installer.to_string_lossy().replace('\\', "/")
    } else {
        String::new()
    };

    #[cfg(target_os = "linux")]   let current_os = "linux";
    #[cfg(target_os = "windows")] let current_os = "windows";
    #[cfg(target_os = "macos")]   let current_os = "osx";

    let mut libs_map = HashMap::new();
    for v_info in &all_v_infos {
        for lib in &v_info.libraries {
            if !check_rules(&lib.rules, current_os) { continue; }

            let path = maven_to_path(&lib.name, None);
            let full_path = lib_dir.join(&path);
            if full_path.exists() {
                // Deduplicate by group:artifact:classifier so that:
                // - Different versions collapse (guava:15.0 vs guava:17.0)
                // - Classifiers stay separate (lwjgl vs lwjgl:natives-linux)
                let parts: Vec<&str> = lib.name.split(':').collect();
                let dedup_key = if parts.len() >= 4 {
                    format!("{}:{}:{}", parts[0], parts[1], parts[3].split('@').next().unwrap_or(""))
                } else if parts.len() >= 2 {
                    format!("{}:{}", parts[0], parts[1])
                } else {
                    lib.name.clone()
                };
                libs_map.insert(dedup_key, full_path.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    
    let mut classpath: Vec<String> = libs_map.into_values().collect();
    let jar_path = data_dir.join("versions").join(jar_id).join(format!("{}.jar", jar_id));
    if jar_path.exists() { classpath.push(jar_path.to_string_lossy().replace('\\', "/")); }

    #[cfg(target_os = "windows")] let cp_sep = ";";
    #[cfg(not(target_os = "windows"))] let cp_sep = ":";
    let classpath_str = classpath.join(cp_sep);

    let replace_vars = |mut s: String| -> String {
        s = s.replace("${natives_directory}", &natives_dir_str)
             .replace("${launcher_name}", "JentleMemes")
             .replace("${launcher_version}", "1.0")
             .replace("${library_directory}", &lib_dir_str)
             .replace("${classpath_separator}", cp_sep)
             .replace("${version_name}", version_id)
             .replace("${classpath}", &classpath_str)
             .replace("${client_jar}", &client_jar_path)
             .replace("${auth_player_name}", username)
             .replace("${game_directory}", &game_dir_str)
             .replace("${assets_root}", &assets_dir_str)
             .replace("${assets_index_name}", &asset_index)
             .replace("${auth_uuid}", uuid)
             .replace("${auth_access_token}", token)
             .replace("${user_type}", acc_type)
             .replace("${version_type}", main_v_info.type_.as_deref().unwrap_or("release"))
             .replace("${user_properties}", "{}")
             .replace("${game_assets}", &assets_dir_str)
             .replace("${auth_session}", token)
             .replace("${clientid}", "0")
             .replace("${auth_xuid}", "0")
             .replace("${clientId}", "0")
             .replace("${resolution_width}", "925")
             .replace("${resolution_height}", "530");
             
        while let (Some(start), Some(end)) = (s.find('['), s.find(']')) {
            if start < end {
                let inner = &s[start + 1..end];
                let path = lib_dir.join(maven_to_path(inner, None)).to_string_lossy().replace('\\', "/");
                s.replace_range(start..=end, &path);
            } else { break; }
        }
        s
    };

    let main_class = main_v_info.main_class.clone().or_else(|| vanilla_info.main_class.clone()).ok_or_else(|| Error::Custom("Main class not found".into()))?;
    log(&format!("Главный класс игры: {}", main_class));

    let mut args = Vec::new();
    args.push(format!("-Xmx{}M", settings.ram_mb));
    args.push(format!("-Djava.library.path={}", natives_dir_str));

    // Добавляем аргументы JVM из профилей
    for v_info in &all_v_infos {
        if let Some(jvm_args) = v_info.arguments.as_ref().and_then(|a| a.get("jvm")).and_then(|a| a.as_array()) {
            for v in jvm_args {
                if let Some(s) = v.as_str() { args.push(replace_vars(s.to_string())); } 
                else if let Some(obj) = v.as_object() {
                    let mut allow = true;
                    if let Some(rules) = obj.get("rules").and_then(|r| r.as_array()) {
                        for rule in rules {
                            let action = rule.get("action").and_then(|a| a.as_str()).unwrap_or("allow");
                            let os_name = rule.get("os").and_then(|os| os.get("name")).and_then(|n| n.as_str());
                            let os_match = match os_name {
                                Some("windows") => cfg!(target_os = "windows"),
                                Some("linux") => cfg!(target_os = "linux"),
                                Some("osx") => cfg!(target_os = "macos"),
                                _ => true,
                            };
                            if action == "allow" && !os_match { allow = false; }
                            if action == "disallow" && os_match { allow = false; }
                        }
                    }
                    if allow {
                        if let Some(val) = obj.get("value") {
                            if let Some(s) = val.as_str() { args.push(replace_vars(s.to_string())); } 
                            else if let Some(arr) = val.as_array() {
                                for item in arr { if let Some(s) = item.as_str() { args.push(replace_vars(s.to_string())); } }
                            }
                        }
                    }
                }
            }
        }
    }
    
    if !args.contains(&"-cp".to_string()) && !args.iter().any(|a| a.starts_with("-Djava.class.path")) {
        args.push("-cp".to_string());
        args.push(classpath_str.clone());
    }

    args.push(main_class.clone());

    // --- ОСОБАЯ ОБРАБОТКА FORGEWRAPPER ---
    // ForgeWrapper использует JVM-свойства для поиска installer и minecraft jar,
    // а также -Dforgewrapper.librariesDir для каталога библиотек.
    if main_class.to_lowercase().contains("forgewrapper") {
        if forge_installer_path.is_empty() {
            log("КРИТИЧЕСКАЯ ОШИБКА: ForgeWrapper найден, но файл installer.jar не скачался.");
            return Err(Error::Custom("Установщик Forge не найден! Нажмите 'Починить ядро' в настройках сборки.".into()));
        }
        log(&format!("Настроен ForgeWrapper! Путь к установщику: {}", forge_installer_path));

        let idx = args.iter().position(|a| a == &main_class).unwrap_or(args.len());
        args.insert(idx, format!("-Dforgewrapper.installer={}", forge_installer_path));
        args.insert(idx + 1, format!("-Dforgewrapper.minecraft={}", client_jar_path));
        args.insert(idx + 2, format!("-Dforgewrapper.librariesDir={}", lib_dir_str));
    }

    // Collect game args from the LAST (most derived) profile that has them,
    // since install_loader already merges vanilla args into the loader profile.
    let mut game_args = Vec::new();
    for v_info in all_v_infos.iter().rev() {
        if let Some(legacy) = &v_info.minecraft_arguments {
            game_args.clear();
            for s in legacy.split_whitespace() { game_args.push(s.to_string()); }
            break;
        }
        if let Some(g_args) = v_info.arguments.as_ref().and_then(|a| a.get("game")).and_then(|a| a.as_array()) {
            if !g_args.is_empty() {
                game_args.clear();
                for v in g_args { if let Some(s) = v.as_str() { game_args.push(s.to_string()); } }
                break;
            }
        }
    }

    for arg in game_args { args.push(replace_vars(arg)); }

    // Прямой запуск на сервер (Quick Play). Формат: host:port (если порт не указан — :25565).
    // 1.20+: --quickPlayMultiplayer. До 1.20: --server (удалён в 23w14a).
    if let Some(ip) = server_ip {
        if !ip.is_empty() {
            let addr = if ip.contains(':') {
                ip.to_string()
            } else {
                format!("{}:25565", ip)
            };
            let is_120_plus = jar_id.split('.').nth(1)
                .and_then(|s| s.parse::<u32>().ok())
                .map_or(false, |minor| minor >= 20);
            log(&format!("Прямой вход на сервер: {} (версия {} — {})", addr, jar_id, if is_120_plus { "quickPlayMultiplayer" } else { "server" }));
            if is_120_plus {
                args.push("--quickPlayMultiplayer".into());
                args.push(addr);
            } else {
                args.push("--server".into());
                args.push(addr);
            }
        }
    }

    // Прямой вход в мир (Quick Play). 1.20+: --quickPlaySingleplayer "Имя мира"
    if let Some(wn) = world_name {
        if !wn.is_empty() {
            let is_120_plus = jar_id.split('.').nth(1)
                .and_then(|s| s.parse::<u32>().ok())
                .map_or(false, |minor| minor >= 20);
            if is_120_plus {
                args.push("--quickPlaySingleplayer".into());
                args.push(wn.to_string());
            }
        }
    }

    let required_java = vanilla_info.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);
    let java_path = if !settings.custom_java_path.is_empty() {
        settings.custom_java_path
    } else {
        log(&format!("Требуется Java {}. Проверяем наличие...", required_java));
        match crate::core::java::ensure_java(&app, required_java).await {
            Ok(path) => path,
            Err(e) => {
                log(&format!("Ошибка при подготовке Java: {}", e));
                return Err(e);
            }
        }
    };
    log(&format!("Java {}: {}", required_java, java_path));
    
    log("Запуск Java процесса...");
    log(&format!("Команда: {} {}", java_path, args.join(" "))); // для отладки

    let use_discrete_gpu = {
        let inst_path = data_dir.join("instances").join(instance_id).join("instance.json");
        if inst_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&inst_path) {
                if let Ok(conf) = serde_json::from_str::<crate::config::InstanceConfig>(&content) {
                    conf.settings.as_ref().map(|s| s.use_discrete_gpu).unwrap_or(false)
                } else { false }
            } else { false }
        } else { false }
    };

    let mut cmd = Command::new(&java_path);
    cmd.args(&args).current_dir(&game_dir).stdout(Stdio::piped()).stderr(Stdio::piped());

    #[cfg(target_os = "linux")]
    if use_discrete_gpu {
        cmd.env("__NV_PRIME_RENDER_OFFLOAD", "1")
           .env("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
    }
    #[cfg(target_os = "windows")]
    if use_discrete_gpu {
        cmd.env("SHIM_MCCOMPAT", "0x800000001");
    }

    let mut child = match cmd.spawn() 
    {
        Ok(c) => c,
        Err(e) => {
            log(&format!("ОШИБКА ЗАПУСКА: {}", e));
            return Err(Error::Custom(format!("Не удалось запустить Java: {}", e)));
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Храним процесс для возможности остановки
    let child_handle: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(Some(child)));
    RUNNING.lock().unwrap().insert(instance_id.to_string(), child_handle.clone());
    
    let app_handle = app.clone();
    let inst_id = instance_id.to_string();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().flatten() { 
            let _ = app_handle.emit(&format!("log_{}", inst_id), line);
        }
    });

    let app_handle2 = app.clone();
    let inst_id2 = instance_id.to_string();
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().flatten() { 
            let _ = app_handle2.emit(&format!("log_{}", inst_id2), line);
        }
    });

    // Поток ожидания завершения с поддержкой остановки
    let app_handle3 = app.clone();
    let inst_id3 = instance_id.to_string();
    thread::spawn(move || {
        loop {
            let done = {
                let mut guard = child_handle.lock().unwrap();
                if let Some(ref mut c) = *guard {
                    match c.try_wait() {
                        Ok(Some(_)) => { *guard = None; true }
                        Ok(None) => false,
                        Err(_) => { *guard = None; true }
                    }
                } else {
                    true
                }
            };
            if done {
                let _ = RUNNING.lock().unwrap().remove(&inst_id3);
                let _ = app_handle3.emit("exit_", &inst_id3);
                let _ = app_handle3.emit(&format!("exit_{}", inst_id3), ());
                break;
            }
            thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    Ok("Игра запущена!".into())
}

pub fn stop_instance(instance_id: &str) -> Result<()> {
    let mut running = RUNNING.lock().unwrap();
    if let Some(handle) = running.get(instance_id) {
        let mut guard = handle.lock().unwrap();
        if let Some(ref mut child) = *guard {
            let _ = child.kill();
            *guard = None;
        }
        drop(guard);
        running.remove(instance_id);
    }
    Ok(())
}