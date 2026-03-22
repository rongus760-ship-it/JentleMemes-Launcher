use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use serde_json::Value;
use tauri::AppHandle;
use tokio::sync::Semaphore;
use crate::error::{Result, Error};
use crate::core::api;
use crate::config::get_data_dir;
use crate::core::progress_emit::emit_download_progress;
use crate::core::types::{DownloadProgress, VersionInfo, Library};
use crate::core::utils::download::download_file;
use crate::core::utils::maven::maven_to_path;

pub async fn install_vanilla(version: &str) -> Result<String> {
    let data = api::get_vanilla_version(version).await?;
    save_profile(version, &data)?;
    Ok(version.to_string())
}

pub async fn install_loader(game_version: &str, loader: &str, loader_version: &str) -> Result<String> {
    let vanilla_data = api::get_vanilla_version(game_version).await?;
    let patch_data = api::get_loader_patch(loader, loader_version).await?;
    
    let mut final_data = vanilla_data.clone();
    
    if let Some(main_class) = patch_data.get("mainClass").and_then(|v| v.as_str()) {
        final_data["mainClass"] = Value::String(main_class.to_string());
    }
    // Fetch required dependencies (e.g. net.fabricmc.intermediary for Fabric/Quilt)
    if let Some(requires) = patch_data.get("requires").and_then(|v| v.as_array()) {
        for req in requires {
            if let Some(uid) = req.get("uid").and_then(|v| v.as_str()) {
                let dep_url = format!("https://meta.prismlauncher.org/v1/{}/{}.json", uid, game_version);
                if let Ok(dep_data) = api::HTTP_CLIENT.get(&dep_url).send().await
                    .and_then(|r| Ok(r)).map_err(|e| Error::Custom(e.to_string()))
                {
                    if let Ok(dep_json) = dep_data.json::<Value>().await {
                        if let Some(dep_libs) = dep_json.get("libraries").and_then(|v| v.as_array()) {
                            let mut base_libs = final_data.get("libraries")
                                .and_then(|v| v.as_array()).cloned().unwrap_or_default();
                            base_libs.extend(dep_libs.clone());
                            final_data["libraries"] = Value::Array(base_libs);
                        }
                    }
                }
            }
        }
    }

    if let Some(patch_libs) = patch_data.get("libraries").and_then(|v| v.as_array()) {
        let mut base_libs = final_data.get("libraries").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        base_libs.extend(patch_libs.clone());
        final_data["libraries"] = Value::Array(base_libs);
    }
    if final_data.get("arguments").is_none() {
        final_data["arguments"] = serde_json::json!({ "jvm": [], "game": [] });
    }

    if let Some(mc_args) = patch_data.get("minecraftArguments").and_then(|v| v.as_str()) {
        final_data["minecraftArguments"] = Value::String(mc_args.to_string());
        // minecraftArguments is a complete spec; remove arguments.game to avoid duplication
        if let Some(args) = final_data.get_mut("arguments").and_then(|v| v.as_object_mut()) {
            args.remove("game");
        }
    }

    if let Some(args) = final_data.get_mut("arguments").and_then(|v| v.as_object_mut()) {
        if let Some(jvm_patch) = patch_data.get("+jvmArgs").and_then(|v| v.as_array()) {
            let mut jvm_base = args.get("jvm").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            jvm_base.extend(jvm_patch.clone());
            args.insert("jvm".to_string(), Value::Array(jvm_base));
        }
        if let Some(game_patch) = patch_data.get("+gameArgs").and_then(|v| v.as_array()) {
            let mut game_base = args.get("game").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            game_base.extend(game_patch.clone());
            args.insert("game".to_string(), Value::Array(game_base));
        }
    }

    // Prism Meta uses "+tweakers" for old Forge versions (e.g. 1.7.10)
    // that need --tweakClass arguments appended to minecraftArguments
    if let Some(tweakers) = patch_data.get("+tweakers").and_then(|v| v.as_array()) {
        let mut mc_args = final_data.get("minecraftArguments")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        for tweaker in tweakers {
            if let Some(cls) = tweaker.as_str() {
                mc_args.push_str(&format!(" --tweakClass {}", cls));
            }
        }
        if !mc_args.is_empty() {
            final_data["minecraftArguments"] = Value::String(mc_args);
            if let Some(args) = final_data.get_mut("arguments").and_then(|v| v.as_object_mut()) {
                args.remove("game");
            }
        }
    }

    if let Some(maven_files) = patch_data.get("mavenFiles") {
        final_data["mavenFiles"] = maven_files.clone();
    }

    let target_id = format!("{}-{}-{}", loader, loader_version, game_version);
    final_data["id"] = Value::String(target_id.clone());
    final_data["inheritsFrom"] = Value::String(game_version.to_string());
    
    save_profile(&target_id, &final_data)?;
    Ok(target_id)
}

fn save_profile(id: &str, json: &Value) -> Result<()> {
    let dir = get_data_dir().join("versions").join(id);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(format!("{}.json", id)), serde_json::to_string_pretty(json)?)?;
    Ok(())
}

fn check_lib_rules(rules: &Option<Vec<serde_json::Value>>, current_os: &str) -> bool {
    let Some(rules) = rules else { return true; };
    if rules.is_empty() { return true; }

    let mut allowed = false;
    let mut disallowed = false;

    for rule in rules {
        let action = rule.get("action").and_then(|v| v.as_str()).unwrap_or("allow");
        let os_match = if let Some(os) = rule.get("os").and_then(|v| v.as_object()) {
            os.get("name").and_then(|v| v.as_str()).map_or(true, |n| n == current_os)
        } else {
            true
        };
        let is_os_specific = rule.get("os").is_some();

        if action == "allow" {
            if os_match {
                if is_os_specific { return true; }
                allowed = true;
            }
        } else if action == "disallow" && os_match {
            disallowed = true;
        }
    }

    allowed && !disallowed
}

pub async fn download_game_files(app: AppHandle, version_id: &str, instance_id: Option<&str>) -> Result<String> {
    let data_dir = get_data_dir();
    let inst_id_opt = instance_id.map(String::from);
    let mut all_libraries: Vec<Library> = Vec::new();
    let mut current_id = version_id.to_string();
    let mut main_v_info: Option<VersionInfo> = None;

    // Собираем все библиотеки по цепочке наследования
    loop {
        let v_path = data_dir.join("versions").join(&current_id).join(format!("{}.json", current_id));
        if !v_path.exists() {
            break;
        }

        let v_info: VersionInfo = serde_json::from_str(&fs::read_to_string(&v_path)?)?;
        all_libraries.extend(v_info.libraries.clone());
        if main_v_info.is_none() {
            main_v_info = Some(v_info.clone());
        }
        if let Some(parent) = v_info.inherits_from {
            current_id = parent;
        } else {
            break;
        }
    }

    let v_info = main_v_info.ok_or_else(|| Error::Custom("Version not found".into()))?;

    // --- Скачивание библиотек ---
    let mut tasks = Vec::new();

    #[cfg(target_os = "linux")]   let os_name = "linux";
    #[cfg(target_os = "windows")] let os_name = "windows";
    #[cfg(target_os = "macos")]   let os_name = "osx";

    for lib in &all_libraries {
        if !check_lib_rules(&lib.rules, os_name) { continue; }

        // Determine whether to download the base (non-native) artifact.
        // Skip ONLY when `downloads` exists but has no `artifact` key
        // (explicitly native-only libs like lwjgl-platform).
        // When `downloads` is absent entirely (old format, e.g. 1.7.10),
        // we still need to download using Maven coordinates.
        let has_downloads = lib.downloads.is_some();
        let has_artifact = lib.downloads.as_ref()
            .and_then(|d| d.get("artifact"))
            .is_some();
        let should_download_base = has_artifact || !has_downloads;

        if should_download_base {
            let mut url = String::new();
            let path = maven_to_path(&lib.name, None);
            let dest = data_dir.join("libraries").join(&path);

            if !dest.exists() {
                if let Some(downloads) = &lib.downloads {
                    if let Some(artifact) = downloads.get("artifact").and_then(|v| v.as_object()) {
                        if let Some(url_str) = artifact.get("url").and_then(|v| v.as_str()) {
                            if !url_str.is_empty() {
                                url = url_str.to_string();
                            }
                        }
                    }
                }
                if url.is_empty() {
                    let base_url = if let Some(base) = &lib.url {
                        if base.ends_with('/') { base.clone() } else { format!("{}/", base) }
                    } else if lib.name.starts_with("net.minecraftforge") {
                        "https://maven.minecraftforge.net/".to_string()
                    } else if lib.name.starts_with("net.neoforged") {
                        "https://maven.neoforged.net/releases/".to_string()
                    } else if lib.name.starts_with("io.github.zekerzhayard") {
                        "https://maven.prismlauncher.org/".to_string()
                    } else {
                        "https://libraries.minecraft.net/".to_string()
                    };
                    url = format!("{}{}", base_url, path);
                }
                tasks.push((url, dest));
            }
        }

        // Native classifiers (e.g. lwjgl-platform, jinput-platform)
        if let Some(natives) = &lib.natives {
            if let Some(classifier_key) = natives.get(os_name).and_then(|v| v.as_str()) {
                if let Some(classifiers) = lib.downloads.as_ref()
                    .and_then(|d| d.get("classifiers"))
                    .and_then(|c| c.get(classifier_key))
                    .and_then(|v| v.as_object())
                {
                    if let Some(url) = classifiers.get("url").and_then(|v| v.as_str()) {
                        let native_path = if let Some(p) = classifiers.get("path").and_then(|v| v.as_str()) {
                            p.to_string()
                        } else {
                            maven_to_path(&lib.name, Some(classifier_key))
                        };
                        let dest = data_dir.join("libraries").join(&native_path);
                        if !dest.exists() {
                            tasks.push((url.to_string(), dest));
                        }
                    }
                }
            }
        }
    }

    let total_tasks = tasks.len();
    if total_tasks > 0 {
        let downloaded = Arc::new(AtomicUsize::new(0));
        let semaphore = Arc::new(Semaphore::new(15));
        let mut handles = Vec::new();

        emit_download_progress(&app, DownloadProgress {
            task_name: format!("Скачивание библиотек (0/{})...", total_tasks),
            downloaded: 0,
            total: total_tasks,
            instance_id: inst_id_opt.clone(),
            ..Default::default()
        });

        for (url, dest) in tasks {
            let dl_counter = downloaded.clone();
            let app_clone = app.clone();
            let sem = semaphore.clone();
            let inst_id = inst_id_opt.clone();

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                if let Some(p) = dest.parent() {
                    let _ = tokio::fs::create_dir_all(p).await;
                }
                match download_file(&url, None).await {
                    Ok(bytes) => {
                        if let Err(e) = tokio::fs::write(&dest, bytes).await {
                            eprintln!("Ошибка записи библиотеки {}: {}", dest.display(), e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Ошибка скачивания {}: {}", url, e);
                    }
                }

                let current = dl_counter.fetch_add(1, Ordering::SeqCst) + 1;
                emit_download_progress(&app_clone, DownloadProgress {
                    task_name: format!("Скачивание библиотек ({}/{})...", current, total_tasks),
                    downloaded: current,
                    total: total_tasks,
                    instance_id: inst_id.clone(),
                    ..Default::default()
                });
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }
    }

    // --- Скачивание ассетов ---
    {
        let assets_dir = data_dir.join("assets");
        fs::create_dir_all(assets_dir.join("indexes"))?;
        fs::create_dir_all(assets_dir.join("objects"))?;

        // Find asset index from the vanilla (root) profile
        let mut root_id = version_id.to_string();
        loop {
            let vp = data_dir.join("versions").join(&root_id).join(format!("{}.json", root_id));
            if !vp.exists() { break; }
            let raw: Value = serde_json::from_str(&fs::read_to_string(&vp)?)?;
            if let Some(parent) = raw.get("inheritsFrom").and_then(|v| v.as_str()) {
                root_id = parent.to_string();
            } else {
                break;
            }
        }
        let root_path = data_dir.join("versions").join(&root_id).join(format!("{}.json", root_id));
        if root_path.exists() {
            let root_raw: Value = serde_json::from_str(&fs::read_to_string(&root_path)?)?;
            if let Some(ai) = root_raw.get("assetIndex") {
                let index_id = ai.get("id").and_then(|v| v.as_str()).unwrap_or("legacy");
                let index_url = ai.get("url").and_then(|v| v.as_str());
                let index_path = assets_dir.join("indexes").join(format!("{}.json", index_id));

                if let Some(url) = index_url {
                    if !index_path.exists() {
                        match download_file(url, None).await {
                            Ok(bytes) => { fs::write(&index_path, &bytes)?; }
                            Err(e) => { eprintln!("Ошибка скачивания asset index: {}", e); }
                        }
                    }
                }

                if index_path.exists() {
                    let index_raw: Value = serde_json::from_str(&fs::read_to_string(&index_path)?)?;
                    if let Some(objects) = index_raw.get("objects").and_then(|v| v.as_object()) {
                        let mut asset_tasks = Vec::new();
                        for (_name, obj) in objects {
                            if let Some(hash) = obj.get("hash").and_then(|v| v.as_str()) {
                                let prefix = &hash[..2];
                                let dest = assets_dir.join("objects").join(prefix).join(hash);
                                if !dest.exists() {
                                    let url = format!("https://resources.download.minecraft.net/{}/{}", prefix, hash);
                                    asset_tasks.push((url, dest));
                                }
                            }
                        }

                        let total_assets = asset_tasks.len();
                        if total_assets > 0 {
                            let downloaded = Arc::new(AtomicUsize::new(0));
                            let sem = Arc::new(Semaphore::new(30));
                            let mut handles = Vec::new();

                            emit_download_progress(&app, DownloadProgress {
                                task_name: format!("Скачивание ассетов (0/{})...", total_assets),
                                downloaded: 0,
                                total: total_assets,
                                instance_id: inst_id_opt.clone(),
                                ..Default::default()
                            });

                            for (url, dest) in asset_tasks {
                                let dl = downloaded.clone();
                                let app_c = app.clone();
                                let total = total_assets;
                                let s = sem.clone();
                                let inst_id = inst_id_opt.clone();
                                handles.push(tokio::spawn(async move {
                                    let _permit = s.acquire().await.unwrap();
                                    if let Some(p) = dest.parent() {
                                        let _ = tokio::fs::create_dir_all(p).await;
                                    }
                                    match download_file(&url, None).await {
                                        Ok(bytes) => { let _ = tokio::fs::write(&dest, bytes).await; }
                                        Err(e) => { eprintln!("Ошибка ассета {}: {}", url, e); }
                                    }
                                    let cur = dl.fetch_add(1, Ordering::SeqCst) + 1;
                                    if cur % 20 == 0 || cur == total {
                                        emit_download_progress(&app_c, DownloadProgress {
                                            task_name: format!("Скачивание ассетов ({}/{})...", cur, total),
                                            downloaded: cur,
                                            total,
                                            instance_id: inst_id.clone(),
                                            ..Default::default()
                                        });
                                    }
                                }));
                            }
                            for h in handles { let _ = h.await; }
                        }
                    }
                }
            }
        }
    }

    // --- Определяем ID версии, от которой нужно взять ядро (client.jar) ---
    let jar_id = v_info.inherits_from.as_deref().unwrap_or(version_id);
    let jar_path = data_dir.join("versions").join(jar_id).join(format!("{}.jar", jar_id));

    println!("[DEBUG] jar_id = {}, jar_path = {:?}", jar_id, jar_path);

    if !jar_path.exists() {
        // Загружаем JSON целевой версии как сырое значение (чтобы получить доступ к mainJar)
        let target_json_path = data_dir.join("versions").join(jar_id).join(format!("{}.json", jar_id));
        if !target_json_path.exists() {
            return Err(Error::Custom(format!("Файл описания версии {} не найден", jar_id)));
        }

        let target_raw: Value = serde_json::from_str(&fs::read_to_string(target_json_path)?)?;
        println!("[DEBUG] JSON загружен, ищем URL...");

        // Пытаемся получить URL ядра: сначала стандартный путь, затем mainJar
        let client_url = target_raw
            .pointer("/downloads/client/url")
            .and_then(|v| v.as_str())
            .or_else(|| {
                target_raw
                    .pointer("/mainJar/downloads/artifact/url")
                    .and_then(|v| v.as_str())
            })
            .map(String::from);

        println!("[DEBUG] client_url = {:?}", client_url);

        let url = match client_url {
            Some(u) => u,
            None => {
                // Если не нашли, пробуем поискать в родительской версии (на случай, если у jar_id есть inheritsFrom)
                if let Some(parent) = target_raw.get("inheritsFrom").and_then(|v| v.as_str()) {
                    let parent_path = data_dir.join("versions").join(parent).join(format!("{}.json", parent));
                    if parent_path.exists() {
                        let parent_raw: Value = serde_json::from_str(&fs::read_to_string(parent_path)?)?;
                        let parent_url = parent_raw
                            .pointer("/downloads/client/url")
                            .and_then(|v| v.as_str())
                            .or_else(|| {
                                parent_raw
                                    .pointer("/mainJar/downloads/artifact/url")
                                    .and_then(|v| v.as_str())
                            })
                            .map(String::from);
                        if let Some(u) = parent_url {
                            u
                        } else {
                            return Err(Error::Custom(format!(
                                "Не удалось найти URL для client.jar в версиях {} и {}",
                                jar_id, parent
                            )));
                        }
                    } else {
                        return Err(Error::Custom(format!(
                            "Родительская версия {} не найдена, а в {} нет URL ядра",
                            parent, jar_id
                        )));
                    }
                } else {
                    return Err(Error::Custom(format!(
                        "В версии {} не найден URL для client.jar (ни в downloads.client, ни в mainJar)",
                        jar_id
                    )));
                }
            }
        };

        if let Some(p) = jar_path.parent() {
            fs::create_dir_all(p)?;
            println!("[DEBUG] Папка создана: {:?}", p);
        }

        emit_download_progress(&app, DownloadProgress {
            task_name: "Скачивание ядра Minecraft...".into(),
            downloaded: 0,
            total: 1,
            instance_id: inst_id_opt.clone(),
            ..Default::default()
        });

        println!("[DEBUG] Начинаем скачивание с URL: {}", url);

        match download_file(&url, None).await {
            Ok(bytes) => {
                println!("[DEBUG] Скачано {} байт", bytes.len());
                fs::write(&jar_path, bytes)?;
                println!("[DEBUG] Файл сохранён: {:?}", jar_path);
                emit_download_progress(&app, DownloadProgress {
                    task_name: "Скачивание ядра Minecraft...".into(),
                    downloaded: 1,
                    total: 1,
                    instance_id: inst_id_opt.clone(),
                    ..Default::default()
                });
            }
            Err(e) => {
                eprintln!("[ERROR] Ошибка скачивания ядра: {}", e);
                return Err(Error::Custom(format!("Ошибка скачивания ядра: {}", e)));
            }
        }
    } else {
        println!("[DEBUG] Ядро уже существует, пропускаем скачивание.");
    }

    // --- Копирование client.jar в libraries для Forge-инсталлятора ---
    // Forge installer ожидает клиент по Maven-пути: net/minecraft/client/{ver}/client-{ver}-official.jar
    if version_id.contains("forge") || version_id.contains("neoforge") {
        let official_jar = data_dir.join("libraries")
            .join("net").join("minecraft").join("client").join(jar_id)
            .join(format!("client-{}-official.jar", jar_id));
        if !official_jar.exists() && jar_path.exists() {
            if let Some(p) = official_jar.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(&jar_path, &official_jar)?;
            println!("[DEBUG] Client.jar скопирован для Forge: {:?}", official_jar);
        }
    }

    // --- Скачивание mavenFiles (для Forge/NeoForge ForgeWrapper) ---
    let mut all_maven_files: Vec<Library> = Vec::new();
    {
        let mut cur_id = version_id.to_string();
        loop {
            let vp = data_dir.join("versions").join(&cur_id).join(format!("{}.json", cur_id));
            if !vp.exists() { break; }
            let vi: VersionInfo = serde_json::from_str(&fs::read_to_string(&vp)?)?;
            all_maven_files.extend(vi.maven_files.clone());
            if let Some(parent) = vi.inherits_from { cur_id = parent; } else { break; }
        }
    }

    if !all_maven_files.is_empty() {
        let installer_dest = data_dir.join("versions").join(version_id).join("installer.jar");
        let mut maven_tasks = Vec::new();
        let mut installer_url_for_copy: Option<(String, Option<String>)> = None;

        for mf in &all_maven_files {
            let path = maven_to_path(&mf.name, None);
            let dest = data_dir.join("libraries").join(&path);

            let classifier = mf.name.split(':').nth(3).unwrap_or("").split('@').next().unwrap_or("");
            let is_installer = classifier == "installer";

            if !dest.exists() {
                let mut url = String::new();
                if let Some(downloads) = &mf.downloads {
                    if let Some(artifact) = downloads.get("artifact").and_then(|v| v.as_object()) {
                        if let Some(url_str) = artifact.get("url").and_then(|v| v.as_str()) {
                            if !url_str.is_empty() {
                                url = url_str.to_string();
                            }
                        }
                    }
                }
                if url.is_empty() {
                    let base_url = if let Some(base) = &mf.url {
                        if base.ends_with('/') { base.clone() } else { format!("{}/", base) }
                    } else if mf.name.starts_with("net.minecraftforge") {
                        "https://maven.minecraftforge.net/".to_string()
                    } else if mf.name.starts_with("net.neoforged") {
                        "https://maven.neoforged.net/releases/".to_string()
                    } else {
                        "https://libraries.minecraft.net/".to_string()
                    };
                    url = format!("{}{}", base_url, path);
                }

                if is_installer {
                    installer_url_for_copy = Some((url.clone(), None));
                }
                maven_tasks.push((url, dest));
            } else if is_installer && !installer_dest.exists() {
                installer_url_for_copy = Some((String::new(), Some(dest.to_string_lossy().to_string())));
            }
        }

        let total_maven = maven_tasks.len();
        if total_maven > 0 {
            let downloaded = Arc::new(AtomicUsize::new(0));
            let mut handles = Vec::new();

            emit_download_progress(&app, DownloadProgress {
                task_name: "Скачивание файлов Forge...".into(),
                downloaded: 0,
                total: total_maven,
                instance_id: inst_id_opt.clone(),
                ..Default::default()
            });

            for (url, dest) in maven_tasks {
                let dl_counter = downloaded.clone();
                let app_clone = app.clone();
                let total = total_maven;
                let inst_id = inst_id_opt.clone();

                handles.push(tokio::spawn(async move {
                    if let Some(p) = dest.parent() {
                        let _ = tokio::fs::create_dir_all(p).await;
                    }
                    match download_file(&url, None).await {
                        Ok(bytes) => {
                            if let Err(e) = tokio::fs::write(&dest, bytes).await {
                                eprintln!("Ошибка записи mavenFile {}: {}", dest.display(), e);
                            }
                        }
                        Err(e) => {
                            eprintln!("Ошибка скачивания mavenFile {}: {}", url, e);
                        }
                    }
                    let current = dl_counter.fetch_add(1, Ordering::SeqCst) + 1;
                    if current % 5 == 0 || current == total {
                        emit_download_progress(&app_clone, DownloadProgress {
                            task_name: "Скачивание файлов Forge...".into(),
                            downloaded: current,
                            total,
                            instance_id: inst_id.clone(),
                            ..Default::default()
                        });
                    }
                }));
            }

            for handle in handles {
                let _ = handle.await;
            }
        }

        if !installer_dest.exists() {
            if let Some((url, existing_path)) = installer_url_for_copy {
                if let Some(p) = installer_dest.parent() {
                    fs::create_dir_all(p)?;
                }
                if let Some(src) = existing_path {
                    fs::copy(&src, &installer_dest)?;
                } else if !url.is_empty() {
                    match download_file(&url, None).await {
                        Ok(bytes) => { fs::write(&installer_dest, bytes)?; }
                        Err(e) => {
                            eprintln!("[ERROR] Ошибка скачивания installer.jar: {}", e);
                        }
                    }
                }
            }
        }
    }

    // --- Распаковка нативных библиотек ---
    {
        let natives_dir = data_dir.join("versions").join(version_id).join("natives");
        fs::create_dir_all(&natives_dir)?;

        #[cfg(target_os = "linux")]   let os_key = "linux";
        #[cfg(target_os = "windows")] let os_key = "windows";
        #[cfg(target_os = "macos")]   let os_key = "osx";

        for lib in &all_libraries {
            let classifier_key = if let Some(natives) = &lib.natives {
                natives.get(os_key).and_then(|v| v.as_str()).map(|s| s.to_string())
            } else {
                None
            };

            let Some(classifier) = classifier_key else { continue };

            let native_jar_path = data_dir.join("libraries").join(maven_to_path(&lib.name, Some(&classifier)));
            if !native_jar_path.exists() { continue; }

            let file = match fs::File::open(&native_jar_path) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let mut archive = match zip::ZipArchive::new(file) {
                Ok(a) => a,
                Err(_) => continue,
            };

            for i in 0..archive.len() {
                let mut entry = match archive.by_index(i) {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let name = entry.name().to_string();
                if name.starts_with("META-INF") || entry.is_dir() { continue; }

                let out_path = natives_dir.join(
                    std::path::Path::new(&name).file_name().unwrap_or_default()
                );
                if let Ok(mut out_file) = fs::File::create(&out_path) {
                    let _ = std::io::copy(&mut entry, &mut out_file);
                }
            }
        }
    }

    Ok("Файлы скачаны".into())
}

pub async fn extract_natives(_version_id: &str) -> Result<String> { Ok("ОК".into()) }