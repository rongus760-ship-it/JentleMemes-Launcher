use once_cell::sync::Lazy;
use serde_json::Value;
use std::sync::Mutex;
use tauri::AppHandle;

use crate::error::Error;

/// Алиас для результатов IPC-команд. Сериализация `Error` возвращает
/// `{ message, detail }` — фронт показывает `message` в toast и `detail` по запросу.
/// Для миграции с `Result<_, String>` достаточно заменить тип возврата и убрать
/// `map_err(|e| e.to_string())`. Автопреобразование из io/reqwest/serde — через `?`.
pub type CmdResult<T> = std::result::Result<T, Error>;

// ================= НАСТРОЙКИ И ИНСТАНСЫ =================
#[tauri::command]
pub async fn load_settings() -> CmdResult<serde_json::Value> {
    let settings = crate::config::load_settings()?;
    Ok(serde_json::to_value(&settings)?)
}

#[tauri::command]
pub async fn save_settings(app: tauri::AppHandle, settings: Value) -> CmdResult<()> {
    use tauri::Emitter;
    let ls: crate::config::LauncherSettings = serde_json::from_value(settings)
        .map_err(|e| Error::Custom(format!("Invalid settings: {e}")))?;
    crate::config::save_settings(&ls)?;
    crate::core::api::reset_http_client_cache();
    let _ = app.emit("settings_updated", ());
    Ok(())
}

/// Частичное обновление настроек. Бэкенд мержит `delta` в текущий JSON и пишет атомарно.
/// Фронт может вызвать `invoke("patch_settings", { delta: { theme: "..." } })` без предварительного
/// `load_settings`, что устраняет гонки между вкладками, которые ранее читали/писали весь объект.
#[tauri::command]
pub async fn patch_settings(app: tauri::AppHandle, delta: Value) -> CmdResult<Value> {
    use tauri::Emitter;
    let current = crate::config::load_settings()?;
    let mut merged = serde_json::to_value(&current)?;
    merge_json_in_place(&mut merged, &delta);
    let ls: crate::config::LauncherSettings = serde_json::from_value(merged.clone())
        .map_err(|e| Error::Custom(format!("Invalid settings after patch: {e}")))?;
    crate::config::save_settings(&ls)?;
    crate::core::api::reset_http_client_cache();
    let _ = app.emit("settings_updated", ());
    Ok(merged)
}

/// Рекурсивный merge объектов JSON. Для не-объектов просто заменяем значение.
/// `null` в delta интерпретируется как «удалить ключ».
fn merge_json_in_place(target: &mut Value, delta: &Value) {
    match (target, delta) {
        (Value::Object(ref mut t_map), Value::Object(d_map)) => {
            for (k, v) in d_map {
                if v.is_null() {
                    t_map.remove(k);
                } else if let Some(existing) = t_map.get_mut(k) {
                    merge_json_in_place(existing, v);
                } else {
                    t_map.insert(k.clone(), v.clone());
                }
            }
        }
        (slot, _) => *slot = delta.clone(),
    }
}

#[tauri::command]
pub fn is_game_session_running() -> bool {
    crate::core::game::launch::any_game_session_running()
}

// ============ ONBOARDING ============
// В `main.rs` при парсинге CLI-аргументов флаг `--onboarding` / `--setup` / `-w`
// выставляет этот флаг в true, чтобы вне зависимости от сохранённых настроек
// визард настройки показался снова.
static FORCE_ONBOARDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn set_force_onboarding(force: bool) {
    FORCE_ONBOARDING.store(force, std::sync::atomic::Ordering::SeqCst);
}

#[tauri::command]
pub async fn is_onboarding_pending() -> std::result::Result<bool, String> {
    if FORCE_ONBOARDING.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(true);
    }
    let settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    Ok(!settings.onboarding_completed)
}

#[tauri::command]
pub async fn complete_onboarding(
    app: tauri::AppHandle,
) -> std::result::Result<(), String> {
    use tauri::Emitter;
    let mut settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    settings.onboarding_completed = true;
    crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    FORCE_ONBOARDING.store(false, std::sync::atomic::Ordering::SeqCst);
    let _ = app.emit("settings_updated", ());
    Ok(())
}

#[tauri::command]
pub fn get_running_instance_ids() -> Vec<String> {
    crate::core::game::launch::running_instance_ids()
}

#[tauri::command]
pub async fn get_overlay_target_rect() -> crate::core::game::overlay_target::OverlayTargetRect {
    // X11 walk / EnumWindows + sysinfo — блокирующие. Уводим в пул блок-потоков,
    // иначе 400-мс пулинг оверлея замораживал main-thread лаунчера.
    tokio::task::spawn_blocking(|| {
        let roots = crate::core::game::launch::running_game_root_pids();
        crate::core::game::overlay_target::resolve_overlay_rect(&roots)
    })
    .await
    .unwrap_or(crate::core::game::overlay_target::OverlayTargetRect {
        x: 0,
        y: 0,
        width: 1280,
        height: 720,
        source: "fallback".into(),
    })
}

#[tauri::command]
pub async fn get_game_overlay_stats() -> crate::core::game::overlay_target::GameOverlayStats {
    tokio::task::spawn_blocking(crate::core::game::overlay_target::overlay_game_stats)
        .await
        .unwrap_or(crate::core::game::overlay_target::GameOverlayStats {
            sessions: 0,
            instance_ids: vec![],
            memory_used_mb: 0.0,
            cpu_percent_total: 0.0,
            pids: vec![],
        })
}

// Команды работы с фоновыми изображениями вынесены в `commands::backgrounds`
// (Phase 3.1 — декомпозиция монолитного commands.rs).

#[tauri::command]
pub fn get_data_dir() -> std::result::Result<String, String> {
    Ok(crate::config::get_data_dir().to_string_lossy().to_string())
}

/// Проверка, что каталог данных пользователя доступен на запись (без root и без «тихих» сбоев при скачивании).
#[tauri::command]
pub fn verify_data_dir_writable() -> std::result::Result<String, String> {
    let dir = crate::config::get_data_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Не удалось создать каталог данных {}: {}", dir.display(), e))?;
    let test = dir.join(".jm_write_test");
    std::fs::write(&test, b"ok").map_err(|e| {
        format!(
            "Запись в {} невозможна: {}. Не запускайте лаунчер от root; проверьте владельца каталога (chown) и права.",
            dir.display(),
            e
        )
    })?;
    let _ = std::fs::remove_file(&test);
    Ok(dir.to_string_lossy().into_owned())
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
    let res = crate::core::api::http_client()
        .get(url.trim())
        .send()
        .await
        .map_err(|e| e.to_string())?;
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
    crate::custom_packs::fetch_and_cache_packs()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_instances() -> std::result::Result<Vec<crate::config::InstanceConfig>, String> {
    crate::core::instance::get_all().map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn create_instance(
    name: String,
    game_version: String,
    loader: String,
    loader_version: String,
    icon: Option<String>,
) -> std::result::Result<String, String> {
    crate::core::instance::create(
        &name,
        &game_version,
        &loader,
        &loader_version,
        icon.as_deref(),
    )
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn delete_instance(id: String) -> std::result::Result<(), String> {
    crate::core::instance::delete(&id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn update_instance_core(
    id: String,
    game_version: String,
    loader: String,
    loader_version: String,
) -> std::result::Result<(), String> {
    crate::core::instance::update_core(&id, &game_version, &loader, &loader_version)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn repair_core(id: String) -> std::result::Result<String, String> {
    crate::core::instance::repair_core(&id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn open_folder(app: AppHandle, id: String) -> std::result::Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = crate::config::get_data_dir().join("instances").join(&id);
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_launcher_data_folder(app: AppHandle) -> std::result::Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = crate::config::get_data_dir();
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_default_data_dir_path() -> std::result::Result<String, String> {
    Ok(crate::config::default_data_dir_without_override()
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub fn get_data_root_override_path_json() -> std::result::Result<Option<String>, String> {
    Ok(crate::config::read_data_root_override().map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn pick_data_root_folder() -> std::result::Result<Option<String>, String> {
    let result = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Каталог данных лаунчера")
            .pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(result.map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn apply_data_root_override(path: String) -> std::result::Result<(), String> {
    let p = std::path::PathBuf::from(path.trim());
    if !p.is_absolute() {
        return Err("Нужен абсолютный путь".into());
    }
    crate::config::set_data_root_override(Some(p)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_data_root_override() -> std::result::Result<(), String> {
    crate::config::set_data_root_override(None).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_launcher_active_ms(
    app: AppHandle,
    delta_ms: u64,
) -> std::result::Result<(), String> {
    let mut s = crate::config::load_settings().map_err(|e| e.to_string())?;
    s.launcher_active_ms_accumulated = s.launcher_active_ms_accumulated.saturating_add(delta_ms);
    let hours = s.token_refresh_active_hours.max(0.25);
    let threshold_ms = (hours * 3600.0 * 1000.0) as u64;
    if s.launcher_active_ms_accumulated >= threshold_ms {
        s.launcher_active_ms_accumulated = 0;
        crate::config::save_settings(&s).map_err(|e| e.to_string())?;
        refresh_microsoft_sessions_startup(app.clone()).await?;
    } else {
        crate::config::save_settings(&s).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct JavaRuntimeRow {
    pub path: String,
    pub major: u32,
    pub label: String,
}

#[tauri::command]
pub async fn list_detected_java_runtimes() -> std::result::Result<Vec<JavaRuntimeRow>, String> {
    let mut out: Vec<JavaRuntimeRow> = Vec::new();
    let base = crate::config::get_data_dir().join("java");
    if let Ok(rd) = std::fs::read_dir(&base) {
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            if name == "runtimes" {
                if let Ok(rd2) = std::fs::read_dir(&p) {
                    for e2 in rd2.flatten() {
                        let p2 = e2.path();
                        if !p2.is_dir() {
                            continue;
                        }
                        let n2 = p2.file_name().unwrap().to_string_lossy().to_string();
                        if let Some(bin) = crate::core::java::find_java_binary_public(&p2) {
                            if let Some(m) = crate::core::java::detect_java_major(&bin).await {
                                out.push(JavaRuntimeRow {
                                    path: bin.clone(),
                                    major: m,
                                    label: format!("runtimes/{} (Java {})", n2, m),
                                });
                            }
                        }
                    }
                }
                continue;
            }
            if !name.starts_with("java-") {
                continue;
            }
            if let Some(bin) = crate::core::java::find_java_binary_public(&p) {
                if let Some(m) = crate::core::java::detect_java_major(&bin).await {
                    out.push(JavaRuntimeRow {
                        path: bin.clone(),
                        major: m,
                        label: format!("{} (Java {})", name, m),
                    });
                }
            }
        }
    }
    if let Ok(ls) = crate::config::load_settings() {
        let cj = ls.custom_java_path.trim();
        if !cj.is_empty() {
            if let Some(m) = crate::core::java::detect_java_major(cj).await {
                let dup = out.iter().any(|r| r.path == cj);
                if !dup {
                    out.push(JavaRuntimeRow {
                        path: cj.to_string(),
                        major: m,
                        label: format!("Глобальный путь (Java {})", m),
                    });
                }
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn download_java_major(
    app: AppHandle,
    major: u32,
) -> std::result::Result<String, String> {
    crate::core::java::ensure_java(&app, major)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_java_build_options(
    provider: String,
    major: u32,
) -> std::result::Result<Vec<crate::core::java::JavaBuildOption>, String> {
    crate::core::java::list_java_builds(&provider, major)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_java_build(
    app: AppHandle,
    build_id: String,
    download_url: String,
    archive_name: String,
) -> std::result::Result<String, String> {
    crate::core::java::download_java_build(&app, &build_id, &download_url, &archive_name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_java_default_for_major(
    major: u32,
    runtime_subdir_rel: Option<String>,
) -> std::result::Result<(), String> {
    let mut s = crate::config::load_settings().map_err(|e| e.to_string())?;
    let key = major.to_string();
    match runtime_subdir_rel {
        Some(v) => {
            let t = v.trim().to_string();
            if t.is_empty() || t.contains("..") {
                return Err("Некорректный подкаталог".into());
            }
            s.java_major_default_subdir.insert(key, t);
        }
        None => {
            s.java_major_default_subdir.remove(&key);
        }
    }
    crate::config::save_settings(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn install_dropped_content_file(
    instance_id: String,
    source_path: String,
    folder: String,
) -> std::result::Result<(), String> {
    use std::fs;
    use std::path::Path;
    if instance_id.contains("..") || instance_id.contains('/') || instance_id.contains('\\') {
        return Err("Некорректный ID сборки".into());
    }
    let sub = match folder.as_str() {
        "mods" | "resourcepacks" | "shaderpacks" => folder.as_str(),
        _ => return Err("Неверная папка".into()),
    };
    let dest_dir = crate::config::get_data_dir()
        .join("instances")
        .join(&instance_id)
        .join(sub);
    fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let src = Path::new(&source_path);
    let name = src
        .file_name()
        .ok_or_else(|| "Нет имени файла".to_string())?;
    if name.to_string_lossy().contains("..") {
        return Err("Некорректное имя файла".into());
    }
    let dest = dest_dir.join(name);
    fs::copy(src, &dest).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn backup_instance_zip(instance_id: String) -> std::result::Result<String, String> {
    use std::fs::File;
    use std::io::{Read, Write};
    use walkdir::WalkDir;
    use zip::write::FileOptions;
    use zip::CompressionMethod;

    let dest = rfd::FileDialog::new()
        .set_file_name(&format!("{}-backup.zip", instance_id))
        .set_title("Сохранить резервную копию сборки")
        .save_file()
        .ok_or_else(|| "Отменено".to_string())?;

    let inst = crate::config::get_data_dir()
        .join("instances")
        .join(&instance_id);
    if !inst.is_dir() {
        return Err("Сборка не найдена".into());
    }

    let file = File::create(&dest).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = FileOptions::default().compression_method(CompressionMethod::Deflated);

    for entry in WalkDir::new(&inst).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(&inst)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let zip_name = format!("{}/{}", instance_id, rel);
        zip.start_file(zip_name, opts).map_err(|e| e.to_string())?;
        let mut f = File::open(path).map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        zip.write_all(&buf).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(dest.to_string_lossy().to_string())
}

#[derive(serde::Serialize)]
pub struct InstanceContentCounts {
    pub mods: usize,
    pub resourcepacks: usize,
    pub shaderpacks: usize,
}

#[tauri::command]
pub fn get_instance_content_counts(
    instance_id: String,
) -> std::result::Result<InstanceContentCounts, String> {
    let inst = crate::config::get_data_dir()
        .join("instances")
        .join(instance_id.trim());
    if !inst.is_dir() {
        return Err("Сборка не найдена".into());
    }
    fn count_visible(dir: &std::path::Path) -> usize {
        std::fs::read_dir(dir)
            .ok()
            .map(|r| {
                r.filter_map(|e| e.ok())
                    .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                    .count()
            })
            .unwrap_or(0)
    }
    Ok(InstanceContentCounts {
        mods: count_visible(&inst.join("mods")),
        resourcepacks: count_visible(&inst.join("resourcepacks")),
        shaderpacks: count_visible(&inst.join("shaderpacks")),
    })
}

/// Только под `data_dir`; `rel_path` без `..` (например `instances/<id>/mods`).
#[tauri::command]
pub fn list_data_subdir_entries(
    rel_path: String,
) -> std::result::Result<Vec<serde_json::Value>, String> {
    let base = crate::config::get_data_dir();
    let cleaned = rel_path.trim().trim_start_matches('/').replace('\\', "/");
    let mut cur = base.clone();
    for seg in cleaned.split('/').filter(|s| !s.is_empty()) {
        if seg == ".." {
            return Err("Некорректный путь".into());
        }
        cur.push(seg);
    }
    if !cur.starts_with(&base) {
        return Err("Путь вне каталога данных лаунчера".into());
    }
    if !cur.is_dir() {
        return Ok(vec![]);
    }
    let mut out: Vec<serde_json::Value> = Vec::new();
    for e in std::fs::read_dir(&cur).map_err(|e| e.to_string())? {
        let e = e.map_err(|err| err.to_string())?;
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let meta = e.metadata().map_err(|err| err.to_string())?;
        out.push(serde_json::json!({
            "name": name,
            "is_dir": meta.is_dir(),
            "size": meta.len(),
        }));
    }
    out.sort_by(|a, b| {
        let da = a["is_dir"].as_bool() == Some(true);
        let db = b["is_dir"].as_bool() == Some(true);
        match (da, db) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let na = a["name"].as_str().unwrap_or("").to_lowercase();
                let nb = b["name"].as_str().unwrap_or("").to_lowercase();
                na.cmp(&nb)
            }
        }
    });
    Ok(out)
}
#[tauri::command]
pub async fn list_instance_folders(id: String) -> std::result::Result<Vec<String>, String> {
    crate::core::instance::list_folders(&id).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn save_instance_settings(
    id: String,
    settings: crate::config::InstanceSettings,
) -> std::result::Result<(), String> {
    crate::core::instance::save_settings(&id, settings).map_err(|e| e.to_string())
}

// ================= МОДЫ И СБОРКИ =================
#[tauri::command]
pub async fn get_installed_mods(
    instance_id: String,
) -> std::result::Result<Vec<crate::core::mods::ModInfo>, String> {
    crate::core::mods::get_installed(&instance_id).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn list_worlds(instance_id: String) -> std::result::Result<Vec<String>, String> {
    let saves_dir = crate::config::get_data_dir()
        .join("instances")
        .join(&instance_id)
        .join("saves");
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
#[tauri::command]
pub async fn install_datapack(
    instance_id: String,
    world_name: String,
    url: String,
    filename: String,
) -> std::result::Result<String, String> {
    let dp_dir = crate::config::get_data_dir()
        .join("instances")
        .join(&instance_id)
        .join("saves")
        .join(&world_name)
        .join("datapacks");
    std::fs::create_dir_all(&dp_dir).map_err(|e| e.to_string())?;
    let bytes = crate::core::api::http_client()
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    std::fs::write(dp_dir.join(&filename), bytes).map_err(|e| e.to_string())?;
    Ok(format!("Датапак установлен в мир {}", world_name))
}
#[tauri::command]
pub async fn refresh_mod_metadata(
    app: AppHandle,
    instance_id: String,
) -> std::result::Result<String, String> {
    crate::core::mods::build_metadata(&app, &instance_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok("Метаданные обновлены".into())
}
#[tauri::command]
pub async fn get_installed_content(
    instance_id: String,
    folder: String,
    include_file_hashes: Option<bool>,
) -> std::result::Result<Vec<crate::core::mods::ModInfo>, String> {
    let include = include_file_hashes.unwrap_or(true);
    crate::core::mods::get_installed_from_folder(&instance_id, &folder, include)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn toggle_mod(
    instance_id: String,
    filename: String,
    enable: bool,
    folder: Option<String>,
) -> std::result::Result<(), String> {
    let f = folder.as_deref().unwrap_or("mods");
    crate::core::mods::toggle(&instance_id, f, &filename, enable).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn delete_mod(
    instance_id: String,
    filename: String,
    folder: Option<String>,
) -> std::result::Result<(), String> {
    let f = folder.as_deref().unwrap_or("mods");
    crate::core::mods::delete(&instance_id, f, &filename)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn check_mod_updates(
    hashes: Vec<String>,
    loader: String,
    game_version: String,
) -> std::result::Result<Value, String> {
    crate::core::mods::check_updates(&hashes, &loader, &game_version)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn install_mod_with_dependencies(
    app: AppHandle,
    instance_id: String,
    version_id: String,
    game_version: String,
    loader: String,
) -> std::result::Result<String, String> {
    let download_deps = crate::config::load_settings()
        .map(|s| s.download_dependencies)
        .unwrap_or(true);
    crate::core::mods::install_with_dependencies(
        app,
        &instance_id,
        &version_id,
        &game_version,
        &loader,
        download_deps,
    )
    .await
    .map_err(|e| e.to_string())
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
#[tauri::command]
pub async fn save_profiles(profiles: Value) -> std::result::Result<(), String> {
    std::fs::write(
        crate::config::get_data_dir().join("profiles.json"),
        serde_json::to_string(&profiles).unwrap(),
    )
    .map_err(|e| e.to_string())
}

/// Текстура скина: Microsoft — session Mojang по UUID; Ely.by — **сначала** session Ely (как в игре), иначе Mojang по нику.
#[tauri::command]
pub async fn resolve_session_skin(
    uuid: String,
    account_type: String,
    username: String,
) -> std::result::Result<Option<Value>, String> {
    if account_type != "microsoft" && account_type != "elyby" {
        return Ok(None);
    }
    if account_type == "elyby" {
        match crate::core::auth::fetch_session_skin_url(&uuid, true).await {
            Ok(Some((url, slim))) => {
                return Ok(Some(serde_json::json!({ "url": url, "slim": slim })));
            }
            Ok(None) | Err(_) => {}
        }
        let u = username.trim();
        if !u.is_empty() {
            match crate::core::auth::fetch_skin_texture_by_username_mojang_or_ely(u).await {
                Ok(Some((url, slim))) => {
                    return Ok(Some(serde_json::json!({ "url": url, "slim": slim })));
                }
                Ok(None) | Err(_) => {}
            }
        }
        return Ok(None);
    }
    match crate::core::auth::fetch_session_skin_url(&uuid, false).await {
        Ok(Some((url, slim))) => Ok(Some(serde_json::json!({ "url": url, "slim": slim }))),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Полная PNG-текстура Mojang по нику (обычно 64×64 со слоем), для превью «по нику» вместо minotar 64×32.
#[tauri::command]
pub async fn resolve_skin_texture_by_username(
    username: String,
) -> std::result::Result<Option<Value>, String> {
    let u = username.trim();
    if u.is_empty() {
        return Ok(None);
    }
    match crate::core::auth::fetch_skin_texture_by_username_mojang_or_ely(u).await {
        Ok(Some((url, slim))) => Ok(Some(serde_json::json!({ "url": url, "slim": slim }))),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Скачать скин по нику (Mojang или skinsystem Ely) и загрузить в профиль текущего аккаунта.
#[tauri::command]
pub async fn upload_skin_from_remote_username_for_account(
    account_id: String,
    username: String,
    slim: bool,
) -> std::result::Result<(), String> {
    let u = username.trim();
    if u.is_empty() {
        return Err("Укажите никнейм".into());
    }
    let path = crate::config::get_data_dir().join("profiles.json");
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let accounts = v
        .get("accounts")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "Нет списка аккаунтов".to_string())?;
    let acc = accounts
        .iter()
        .find(|a| a.get("id").and_then(|x| x.as_str()) == Some(account_id.as_str()))
        .ok_or_else(|| "Аккаунт не найден".to_string())?;
    let acc_type = acc.get("acc_type").and_then(|x| x.as_str()).unwrap_or("");
    if acc_type != "microsoft" && acc_type != "elyby" {
        return Err("Только для аккаунтов Microsoft или Ely.by".to_string());
    }
    if acc_type == "elyby" {
        return Err(crate::core::auth::ELY_SKIN_CHANGE_WEB_ONLY.to_string());
    }
    let token = acc
        .get("token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "Нет токена сессии".to_string())?
        .to_string();
    let (tex_url, slim_resolved) =
        crate::core::auth::fetch_skin_texture_by_username_mojang_or_ely(u)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Скин для этого ника не найден (Mojang и Ely.by)".to_string())?;
    let use_slim = slim || slim_resolved;
    let bytes = crate::core::auth::download_skin_png_from_url(&tex_url)
        .await
        .map_err(|e| e.to_string())?;
    crate::core::auth::upload_skin_minecraft_services(&token, &bytes, use_slim)
        .await
        .map_err(|e| e.to_string())
}

/// Загрузить локальный PNG в профиль Minecraft (Microsoft). Для Ely.by см. `ELY_SKIN_CHANGE_WEB_ONLY`.
#[tauri::command]
pub async fn upload_skin_mojang_for_account(
    account_id: String,
    png_base64: String,
    slim: bool,
) -> std::result::Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let path = crate::config::get_data_dir().join("profiles.json");
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let accounts = v
        .get("accounts")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "Нет списка аккаунтов".to_string())?;
    let acc = accounts
        .iter()
        .find(|a| a.get("id").and_then(|x| x.as_str()) == Some(account_id.as_str()))
        .ok_or_else(|| "Аккаунт не найден".to_string())?;
    let acc_type = acc.get("acc_type").and_then(|x| x.as_str()).unwrap_or("");
    if acc_type != "microsoft" && acc_type != "elyby" {
        return Err("Только для аккаунтов Microsoft или Ely.by".to_string());
    }
    if acc_type == "elyby" {
        return Err(crate::core::auth::ELY_SKIN_CHANGE_WEB_ONLY.to_string());
    }
    let token = acc
        .get("token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "Нет токена сессии".to_string())?
        .to_string();
    let mut b64 = png_base64.trim();
    if let Some(rest) = b64.strip_prefix("data:image/png;base64,") {
        b64 = rest.trim();
    }
    let bytes = STANDARD.decode(b64).map_err(|e| e.to_string())?;
    crate::core::auth::upload_skin_minecraft_services(&token, &bytes, slim)
        .await
        .map_err(|e| e.to_string())
}

/// Сбросить скин Ely.by на стандартный.
#[tauri::command]
pub async fn delete_skin_elyby_for_account(account_id: String) -> std::result::Result<(), String> {
    let path = crate::config::get_data_dir().join("profiles.json");
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let accounts = v
        .get("accounts")
        .and_then(|a| a.as_array())
        .ok_or_else(|| "Нет списка аккаунтов".to_string())?;
    let acc = accounts
        .iter()
        .find(|a| a.get("id").and_then(|x| x.as_str()) == Some(account_id.as_str()))
        .ok_or_else(|| "Аккаунт не найден".to_string())?;
    if acc.get("acc_type").and_then(|x| x.as_str()) != Some("elyby") {
        return Err("Только для аккаунта Ely.by".to_string());
    }
    Err(crate::core::auth::ELY_SKIN_RESET_WEB_ONLY.to_string())
}
#[tauri::command]
pub async fn login_offline(
    username: String,
) -> std::result::Result<crate::config::AccountInfo, String> {
    crate::core::auth::login_offline(&username)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn login_elyby(
    email: String,
    password: String,
) -> std::result::Result<crate::config::AccountInfo, String> {
    crate::core::auth::login_elyby(&email, &password)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn ms_init_device_code() -> std::result::Result<crate::config::DeviceCodeResponse, String>
{
    crate::core::auth::ms_init_device_code()
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn ms_login_poll(
    device_code: String,
    interval: u64,
) -> std::result::Result<crate::config::AccountInfo, String> {
    crate::core::auth::ms_login_poll(&device_code, interval)
        .await
        .map_err(|e| e.to_string())
}

static MS_OAUTH_BUSY: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static MS_OAUTH_DONE: Lazy<Mutex<Option<std::result::Result<crate::config::AccountInfo, String>>>> =
    Lazy::new(|| Mutex::new(None));

/// Сохранённый PKCE verifier текущей сессии OAuth (используется callback-ом навигации).
static MS_OAUTH_VERIFIER: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

/// Открыть мини-окно Microsoft-входа. Использует nativeclient-redirect (зарегистрирован для public-клиентов
/// по умолчанию, не требует настройки Azure) + перехват навигации WebView. Результат берётся через
/// `ms_oauth_try_take_account`.
#[tauri::command]
pub async fn ms_oauth_prepare_interactive(
    app: tauri::AppHandle,
) -> std::result::Result<String, String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    {
        let mut busy = MS_OAUTH_BUSY.lock().map_err(|e| e.to_string())?;
        if *busy {
            return Err("Уже выполняется вход Microsoft".into());
        }
        *busy = true;
    }
    if let Ok(mut g) = MS_OAUTH_DONE.lock() {
        *g = None;
    }

    let redirect_uri = crate::core::auth::MS_NATIVE_REDIRECT.to_string();
    let verifier = crate::core::auth::oauth_pkce_verifier();
    let challenge = crate::core::auth::oauth_pkce_challenge(&verifier);
    let auth_url = crate::core::auth::ms_oauth_authorize_url(&redirect_uri, &challenge);

    if let Ok(mut g) = MS_OAUTH_VERIFIER.lock() {
        *g = Some(verifier.clone());
    }

    // Закрываем старое окно, если есть
    if let Some(w) = app.get_webview_window("jentle-ms-oauth") {
        let _ = w.close();
    }

    let parsed = match tauri::Url::parse(&auth_url) {
        Ok(u) => u,
        Err(e) => {
            if let Ok(mut busy) = MS_OAUTH_BUSY.lock() {
                *busy = false;
            }
            return Err(format!("Некорректный URL авторизации: {e}"));
        }
    };

    let app_for_nav = app.clone();
    let redirect_for_nav = redirect_uri.clone();

    let build = WebviewWindowBuilder::new(&app, "jentle-ms-oauth", WebviewUrl::External(parsed))
        .title("Вход Microsoft / Xbox")
        .inner_size(520.0, 820.0)
        .center()
        .resizable(true)
        .on_navigation(move |url| {
            let url_str = url.as_str();
            // Срабатываем только на переходе к nativeclient-redirect (игнорим промежуточные формы логина)
            if !url_str.starts_with(&redirect_for_nav) {
                return true;
            }
            let Some(result) = crate::core::auth::parse_ms_redirect_url(url_str) else {
                return true;
            };
            let app_c = app_for_nav.clone();
            // Отдаём процессинг отдельной задаче, чтобы не блокировать UI-поток
            tauri::async_runtime::spawn(async move {
                finalize_ms_oauth(app_c, result).await;
            });
            // Блокируем саму навигацию — иначе Edge WebView2 попытается открыть nativeclient URL,
            // что бесполезно. Окно закроется в finalize_ms_oauth.
            false
        });

    if let Err(e) = build.build() {
        if let Ok(mut busy) = MS_OAUTH_BUSY.lock() {
            *busy = false;
        }
        return Err(format!("Не удалось создать окно: {e}"));
    }

    // На всякий случай автозавершение по таймауту (15 минут) — чтобы MS_OAUTH_BUSY не завис.
    let app_timeout = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(900)).await;
        let still_busy = MS_OAUTH_BUSY
            .lock()
            .map(|g| *g)
            .unwrap_or(false);
        let already_done = MS_OAUTH_DONE
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(true);
        if still_busy && !already_done {
            finalize_ms_oauth(app_timeout, Err("Время ожидания входа истекло".into())).await;
        }
    });

    Ok(auth_url)
}

async fn finalize_ms_oauth(
    app: tauri::AppHandle,
    code_or_err: std::result::Result<String, String>,
) {
    use tauri::Manager;

    let out: std::result::Result<crate::config::AccountInfo, String> = match code_or_err {
        Err(e) => Err(e),
        Ok(code) => {
            let verifier = MS_OAUTH_VERIFIER
                .lock()
                .ok()
                .and_then(|mut g| g.take())
                .unwrap_or_default();
            let exchange = async {
                let (ms_a, ms_r) = crate::core::auth::ms_oauth_exchange_code_for_tokens(
                    &code,
                    crate::core::auth::MS_NATIVE_REDIRECT,
                    &verifier,
                )
                .await?;
                crate::core::auth::ms_account_from_ms_oauth_tokens(&ms_a, &ms_r).await
            };
            exchange.await.map_err(|e| e.to_string())
        }
    };

    if let Some(w) = app.get_webview_window("jentle-ms-oauth") {
        let _ = w.close();
    }
    if let Ok(mut g) = MS_OAUTH_DONE.lock() {
        *g = Some(out);
    }
    if let Ok(mut busy) = MS_OAUTH_BUSY.lock() {
        *busy = false;
    }
    if let Ok(mut v) = MS_OAUTH_VERIFIER.lock() {
        *v = None;
    }
}

#[tauri::command]
pub fn ms_oauth_try_take_account() -> Option<Value> {
    let done = MS_OAUTH_DONE.lock().ok()?.take()?;
    Some(match done {
        Ok(acc) => serde_json::json!({ "ok": true, "account": acc }),
        Err(e) => serde_json::json!({ "ok": false, "error": e }),
    })
}

/// При старте лаунчера обновляет токены Microsoft-аккаунтов (по refresh_token), чтобы не было «недействительная сессия».
#[tauri::command]
pub async fn refresh_microsoft_sessions_startup(
    app: tauri::AppHandle,
) -> std::result::Result<(), String> {
    use tauri::Emitter;
    let mut profiles = crate::config::load_profiles().map_err(|e| e.to_string())?;
    let mut changed = crate::config::normalize_account_types(&mut profiles);
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

/// Перед запуском игры: обновить Minecraft JWT для Microsoft (истёкший токен → «недействительная сессия»).
///
/// Оптимизация: рефреш через OAuth (Azure → Xbox Live → XSTS → Mojang → profile) делает
/// **5 последовательных HTTP round-trip'ов** и в реальных замерах стоит 8-12 секунд.
/// Minecraft JWT действителен ~24 часа. Пока он не истёк — сеть ПРОПУСКАЕМ: игра примет
/// этот токен при handshake, а мы не тормозим запуск. Локальная проверка `exp`-claim
/// JWT занимает <0.1 мс и не требует сети.
///
/// Если токен истёк — идём полным путём refresh'а (как было). Это безопасный worst-case.
#[tauri::command]
pub async fn refresh_account_for_launch(
    app: tauri::AppHandle,
    account_id: String,
) -> std::result::Result<Value, String> {
    use tauri::Emitter;
    let t_begin = std::time::Instant::now();
    let mut profiles = crate::config::load_profiles().map_err(|e| e.to_string())?;
    if crate::config::normalize_account_types(&mut profiles) {
        let _ = crate::config::save_profiles(&profiles);
    }
    let idx = profiles
        .accounts
        .iter()
        .position(|a| a.id == account_id)
        .ok_or_else(|| "Аккаунт не найден".to_string())?;
    let acc = profiles.accounts[idx].clone();
    if acc.acc_type != "microsoft" {
        return Ok(serde_json::to_value(&acc).map_err(|e| e.to_string())?);
    }

    // Быстрый путь: MC JWT ещё не истёк → возвращаем аккаунт как есть, без HTTP.
    // 0-2 мс вместо 8-12 секунд сетевых round-trip'ов через MS/Xbox/Mojang.
    // `exp` — unix timestamp в секундах; если уже прошёл, делаем полный refresh.
    if !crate::core::auth::ms_token_expired(&acc.token) {
        tracing::info!(
            target: "auth::ms",
            event = "refresh_skip_valid_token",
            dt_ms = t_begin.elapsed().as_millis() as u64,
        );
        return serde_json::to_value(&acc).map_err(|e| e.to_string());
    }

    tracing::info!(target: "auth::ms", event = "refresh_needed_token_expired");
    let new_acc = crate::core::auth::refresh_microsoft_account_on_startup(&acc)
        .await
        .map_err(|e| e.to_string())?;
    let differs = new_acc.token != acc.token
        || new_acc.ms_refresh_token != acc.ms_refresh_token
        || new_acc.username != acc.username
        || new_acc.uuid != acc.uuid;
    profiles.accounts[idx] = new_acc.clone();
    if differs {
        crate::config::save_profiles(&profiles).map_err(|e| e.to_string())?;
        let _ = app.emit("profiles_updated", ());
    }
    tracing::info!(
        target: "auth::ms",
        event = "refresh_done",
        dt_ms = t_begin.elapsed().as_millis() as u64,
    );
    serde_json::to_value(&new_acc).map_err(|e| e.to_string())
}

// ================= MODRINTH & MRPACK =================
#[tauri::command]
pub async fn search_modrinth(
    query: String,
    project_type: String,
    game_version: String,
    loader: String,
    categories: Vec<String>,
    page: usize,
    sort: String,
    sort_desc: Option<bool>,
) -> std::result::Result<Value, String> {
    let desc = sort_desc.unwrap_or(true);
    crate::core::modrinth::search(
        &query,
        &project_type,
        &game_version,
        &loader,
        categories,
        page,
        &sort,
        desc,
    )
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn get_modrinth_project(id: String) -> std::result::Result<Value, String> {
    crate::core::modrinth::get_project(&id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn get_modrinth_versions(id: String) -> std::result::Result<Value, String> {
    crate::core::modrinth::get_versions(&id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn search_curseforge(
    query: String,
    project_type: String,
    game_version: String,
    loader: String,
    categories: Vec<String>,
    page: usize,
    sort: String,
    sort_desc: Option<bool>,
) -> std::result::Result<Value, String> {
    let desc = sort_desc.unwrap_or(true);
    crate::core::curseforge::search(
        &query,
        &project_type,
        &game_version,
        &loader,
        categories,
        page,
        &sort,
        desc,
    )
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn get_curseforge_project(id: String) -> std::result::Result<Value, String> {
    crate::core::curseforge::get_project(&id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn get_curseforge_versions(id: String) -> std::result::Result<Value, String> {
    crate::core::curseforge::get_versions(&id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn get_modrinth_version_details(
    version_id: String,
) -> std::result::Result<Value, String> {
    crate::core::modrinth::get_version_details_for_ui(&version_id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn get_curseforge_file_details(
    mod_id: String,
    file_id: String,
) -> std::result::Result<Value, String> {
    crate::core::curseforge::get_file_detail_for_ui(&mod_id, &file_id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn download_mod_to_user_folder(
    download_url: Option<String>,
    modrinth_version_id: Option<String>,
    curseforge_project_id: Option<String>,
    curseforge_file_id: Option<String>,
    filename_hint: Option<String>,
) -> std::result::Result<String, String> {
    crate::core::mods::download_mod_file_to_user_folder(
        download_url,
        modrinth_version_id,
        curseforge_project_id,
        curseforge_file_id,
        filename_hint,
    )
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn search_hybrid(
    query: String,
    project_type: String,
    game_version: String,
    loader: String,
    categories: Vec<String>,
    page: usize,
    sort: String,
    sort_desc: Option<bool>,
) -> std::result::Result<Value, String> {
    let desc = sort_desc.unwrap_or(true);
    crate::core::curseforge::search_hybrid(
        &query,
        &project_type,
        &game_version,
        &loader,
        categories,
        page,
        &sort,
        desc,
    )
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn get_hybrid_versions(
    modrinth_id: Option<String>,
    curseforge_id: Option<String>,
) -> std::result::Result<Value, String> {
    crate::core::curseforge::get_hybrid_versions(modrinth_id, curseforge_id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn install_modrinth_file(
    app: AppHandle,
    instance_id: String,
    url: String,
    filename: String,
    project_type: String,
) -> std::result::Result<String, String> {
    crate::core::modrinth::install_file(Some(&app), &instance_id, &url, &filename, &project_type)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn install_curseforge_mod_file(
    app: AppHandle,
    instance_id: String,
    curseforge_project_id: String,
    curseforge_file_id: String,
    filename: Option<String>,
    project_type: String,
) -> std::result::Result<String, String> {
    crate::core::mods::install_from_curseforge(
        &app,
        &instance_id,
        &curseforge_project_id,
        &curseforge_file_id,
        filename.as_deref(),
        &project_type,
    )
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn install_mrpack_from_url(
    app: AppHandle,
    url: String,
    name: String,
    modrinth_project_id: Option<String>,
    modrinth_version_id: Option<String>,
    curseforge_project_id: Option<String>,
    curseforge_file_id: Option<String>,
) -> std::result::Result<String, String> {
    crate::core::mrpack::install_from_url(
        app,
        &url,
        &name,
        modrinth_project_id,
        modrinth_version_id,
        curseforge_project_id,
        curseforge_file_id,
    )
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn install_mrpack(
    app: AppHandle,
    file_path: String,
    name: String,
) -> std::result::Result<String, String> {
    use tauri::Emitter;
    let r = crate::core::mrpack::install(app.clone(), &file_path, &name, None).await;
    crate::core::progress_emit::emit_install_progress_cleared(&app);
    if r.is_ok() {
        let _ = app.emit("instances_changed", ());
    }
    r.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn export_instance(
    id: String,
    selected_folders: Vec<String>,
) -> std::result::Result<String, String> {
    crate::core::mrpack::export(&id, selected_folders).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn export_mrpack(
    app: tauri::AppHandle,
    id: String,
    selected_folders: Vec<String>,
) -> std::result::Result<String, String> {
    crate::core::mrpack::export_mrpack_async(&app, &id, selected_folders)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn export_jentlepack(
    app: tauri::AppHandle,
    id: String,
    selected_folders: Vec<String>,
) -> std::result::Result<String, String> {
    crate::core::mrpack::export_jentlepack_async(&app, &id, selected_folders)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn export_jentlepack_to_temp(
    app: tauri::AppHandle,
    id: String,
    selected_folders: Vec<String>,
) -> std::result::Result<String, String> {
    crate::core::mrpack::export_jentlepack_to_temp_path(&app, &id, selected_folders)
        .await
        .map_err(|e| e.to_string())
}

/// Чтение файла только из `<data>/tmp/` (после export_jentlepack_to_temp). Base64 — чтобы не гонять огромный JSON-массив байт.
#[tauri::command]
pub fn read_data_tmp_file_base64(path: String) -> std::result::Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
    use std::fs;
    use std::path::Path;
    let tmp_root = crate::config::get_data_dir().join("tmp");
    let tmp_root = fs::canonicalize(&tmp_root).map_err(|e| e.to_string())?;
    let p = Path::new(path.trim());
    let abs = p.canonicalize().map_err(|e| e.to_string())?;
    if !abs.starts_with(&tmp_root) {
        return Err("Файл вне папки tmp лаунчера".into());
    }
    let bytes = fs::read(&abs).map_err(|e| e.to_string())?;
    Ok(B64.encode(bytes))
}
#[tauri::command]
pub async fn import_instance(
    app: AppHandle,
) -> std::result::Result<crate::core::mrpack::ImportInstanceResult, String> {
    crate::core::mrpack::import_instance_packed(&app)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn import_jentlepack_from_url(
    app: AppHandle,
    url: String,
) -> std::result::Result<crate::core::mrpack::ImportInstanceResult, String> {
    crate::core::mrpack::import_jentlepack_from_url(&app, url)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn check_modpack_update(instance_id: String) -> std::result::Result<Value, String> {
    crate::core::mrpack::check_modpack_update(&instance_id)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn update_modpack(
    app: AppHandle,
    instance_id: String,
    update_url: String,
    new_pack_version_ref: Option<String>,
) -> std::result::Result<String, String> {
    crate::core::mrpack::update_modpack(
        app,
        &instance_id,
        &update_url,
        new_pack_version_ref.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

// ================= ИГРА (Установка и Запуск) =================
#[tauri::command]
pub async fn fetch_vanilla_versions() -> std::result::Result<Value, String> {
    crate::core::loader_meta::manifest_v2()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn download_game_files(
    app: AppHandle,
    version_id: String,
    instance_id: Option<String>,
) -> std::result::Result<String, String> {
    crate::core::game::install::download_game_files(app, &version_id, instance_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_version(
    version_id: String,
    _url: String,
) -> std::result::Result<String, String> {
    crate::core::game::install::install_vanilla(&version_id)
        .await
        .map_err(|e| e.to_string())
}

async fn resolve_fabric_quilt_loader_version(
    loader: &str,
    game_version: &str,
    preferred: Option<String>,
) -> String {
    // Early-exit: если инстанс уже хранит конкретную версию лоадера (типичный кейс
    // для всех модпаков), не ходим в FabricMC API. Раньше `get_specific_loader_versions`
    // вызывался всегда, результат выбрасывался — пустая трата ~200-800 мс сети на
    // каждом «Играть».
    if let Some(p) = preferred
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return p.to_string();
    }
    let gv = game_version.trim();
    let vers = crate::core::utils::system::get_specific_loader_versions(loader, gv)
        .await
        .unwrap_or_default();
    let default = if loader == "fabric" {
        "0.15.7"
    } else {
        "0.24.0"
    };
    vers.first().cloned().unwrap_or_else(|| default.to_string())
}

#[tauri::command]
pub async fn install_fabric(
    version_id: String,
    loader_version: Option<String>,
) -> std::result::Result<String, String> {
    let loader_ver =
        resolve_fabric_quilt_loader_version("fabric", &version_id, loader_version).await;
    crate::core::game::install::install_loader(&version_id, "fabric", &loader_ver)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_quilt(
    version_id: String,
    loader_version: Option<String>,
) -> std::result::Result<String, String> {
    let loader_ver =
        resolve_fabric_quilt_loader_version("quilt", &version_id, loader_version).await;
    crate::core::game::install::install_loader(&version_id, "quilt", &loader_ver)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_forge(
    _app: AppHandle,
    _instance_id: String,
    game_version: String,
    loader_version: String,
    loader_name: String,
) -> std::result::Result<String, String> {
    crate::core::game::install::install_loader(&game_version, &loader_name, &loader_version)
        .await
        .map_err(|e| e.to_string())
}

async fn resolve_liteloader_version(game_version: &str, preferred: Option<String>) -> String {
    let gv = game_version.trim();
    let vers = crate::core::utils::system::get_specific_loader_versions("liteloader", gv)
        .await
        .unwrap_or_default();
    let pref = preferred
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    if let Some(p) = pref {
        return p;
    }
    vers.first().cloned().unwrap_or_default()
}

#[tauri::command]
pub async fn install_liteloader(
    version_id: String,
    loader_version: Option<String>,
) -> std::result::Result<String, String> {
    let s = crate::config::load_settings().unwrap_or_default();
    if !s.enable_alpha_loaders {
        return Err(
            "Включите «Альфа: LiteLoader и ModLoader» в расширенных настройках лаунчера.".into(),
        );
    }
    let loader_ver = resolve_liteloader_version(&version_id, loader_version).await;
    if loader_ver.is_empty() {
        return Err("Не выбрана версия LiteLoader для этой версии Minecraft.".into());
    }
    crate::core::game::install::install_loader(&version_id, "liteloader", &loader_ver)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn install_modloader(
    version_id: String,
    loader_version: Option<String>,
) -> std::result::Result<String, String> {
    let s = crate::config::load_settings().unwrap_or_default();
    if !s.enable_alpha_loaders {
        return Err(
            "Включите «Альфа: LiteLoader и ModLoader» в расширенных настройках лаунчера.".into(),
        );
    }
    let lv = loader_version
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("manual");
    crate::core::game::install::install_loader(&version_id, "modloader", lv)
        .await
        .map_err(|e| e.to_string())
}

/// Консолидированная подготовка к запуску. Схлопывает 4-5 последовательных
/// frontend→backend invoke'ов (`fetch_vanilla_versions` → `install_version` →
/// `install_<loader>` → `download_game_files`) в одну Rust-функцию.
///
/// Выгоды:
///   1. 3-4 IPC boundary меньше (~5-20 мс на холодный чат serialize/deserialize).
///   2. На warm-path всё разрешается синхронно без единого HTTP запроса
///      (`install_vanilla` early-exit + `install_loader` fast-path + `download_game_files`
///      sentinel). Prism-эквивалент: «Component list update/resolve task succeeded» за 0 мс.
///   3. Фронт становится проще и не знает про цепочку установки.
///
/// Фронт должен звать один раз перед `launch_game`/`fluxcore_launch`.
#[tauri::command]
pub async fn prepare_launch(
    app: AppHandle,
    instance_id: String,
    game_version: String,
    loader: String,
    loader_version: Option<String>,
) -> std::result::Result<String, String> {
    use tauri::Emitter;
    let t_begin = std::time::Instant::now();
    let log = |msg: String| {
        let _ = app.emit(
            &format!("log_{}", instance_id),
            format!("[JentleMemes] {}", msg),
        );
    };
    log(format!("▸ prepare_launch: старт (loader={loader}, game={game_version})"));

    let t_vanilla = std::time::Instant::now();
    crate::core::game::install::install_vanilla(&game_version)
        .await
        .map_err(|e| e.to_string())?;
    let dv = t_vanilla.elapsed().as_millis() as u64;
    if dv > 50 {
        log(format!("⏱ install_vanilla Δ={dv}ms"));
    }

    // 2. Лоадер (если не vanilla). `install_loader` на warm-path короткозамыкается
    // на уже существующем модифицированном JSON (Fabric/Quilt/LiteLoader).
    let settings = crate::config::load_settings().unwrap_or_default();
    let t_loader = std::time::Instant::now();
    let launch_version = match loader.as_str() {
        "" | "vanilla" => game_version.clone(),
        "fabric" | "quilt" => {
            let lv =
                resolve_fabric_quilt_loader_version(&loader, &game_version, loader_version).await;
            crate::core::game::install::install_loader(&game_version, &loader, &lv)
                .await
                .map_err(|e| e.to_string())?
        }
        "forge" | "neoforge" => crate::core::game::install::install_loader(
            &game_version,
            &loader,
            loader_version.as_deref().unwrap_or(""),
        )
        .await
        .map_err(|e| e.to_string())?,
        "liteloader" => {
            if !settings.enable_alpha_loaders {
                return Err(
                    "Включите «Альфа: LiteLoader и ModLoader» в расширенных настройках.".into(),
                );
            }
            let lv = resolve_liteloader_version(&game_version, loader_version).await;
            if lv.is_empty() {
                return Err("Не выбрана версия LiteLoader для этой версии Minecraft.".into());
            }
            crate::core::game::install::install_loader(&game_version, "liteloader", &lv)
                .await
                .map_err(|e| e.to_string())?
        }
        "modloader" => {
            if !settings.enable_alpha_loaders {
                return Err(
                    "Включите «Альфа: LiteLoader и ModLoader» в расширенных настройках.".into(),
                );
            }
            let lv = loader_version
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("manual");
            crate::core::game::install::install_loader(&game_version, "modloader", lv)
                .await
                .map_err(|e| e.to_string())?
        }
        other => return Err(format!("Неизвестный загрузчик: {other}")),
    };
    let dl = t_loader.elapsed().as_millis() as u64;
    if dl > 50 {
        log(format!("⏱ install_loader Δ={dl}ms"));
    }

    let t_dl = std::time::Instant::now();
    crate::core::game::install::download_game_files(app.clone(), &launch_version, Some(&instance_id))
        .await
        .map_err(|e| e.to_string())?;
    let dd = t_dl.elapsed().as_millis() as u64;
    if dd > 50 {
        log(format!("⏱ download_game_files Δ={dd}ms"));
    }

    let total = t_begin.elapsed().as_millis() as u64;
    log(format!("⏱ prepare_launch total={total}ms"));
    Ok(launch_version)
}

#[tauri::command]
pub fn add_recent_server(
    ip: String,
    name: String,
    instance_id: Option<String>,
) -> std::result::Result<(), String> {
    let ip = ip.trim().to_string();
    if ip.is_empty() {
        return Err("IP не может быть пустым".into());
    }
    let name = if name.trim().is_empty() {
        ip.clone()
    } else {
        name.trim().to_string()
    };
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
            let mut servers =
                crate::config::load_instance_servers(&inst_dir).map_err(|e| e.to_string())?;
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
    settings
        .recent_servers
        .sort_by(|a, b| b.last_played.cmp(&a.last_played));
    settings.recent_servers.truncate(2);
    crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

/// Загружает серверы: per-instance servers.json + servers.dat, или global при пустом instance_id
#[tauri::command]
pub fn load_servers(instance_id: Option<String>) -> std::result::Result<serde_json::Value, String> {
    let mut by_ip: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
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
                if let Some(ref i) = s.last_instance_id {
                    obj["last_instance_id"] = serde_json::json!(i);
                }
                if let Some(ref n) = s.last_instance_name {
                    obj["last_instance_name"] = serde_json::json!(n);
                }
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
            if let Some(ref id) = s.last_instance_id {
                obj["last_instance_id"] = serde_json::json!(id);
            }
            if let Some(ref name) = s.last_instance_name {
                obj["last_instance_name"] = serde_json::json!(name);
            }
            by_ip.entry(ip).or_insert(obj);
        }
    }
    for s in from_dat {
        by_ip.entry(s.ip.clone()).or_insert_with(|| {
            serde_json::json!({
                "ip": s.ip,
                "name": s.name,
                "last_played": 0,
                "playtime_hours": 0,
                "icon": s.icon,
                "source": "servers_dat"
            })
        });
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
            let mut servers =
                crate::config::load_instance_servers(&inst_dir).map_err(|e| e.to_string())?;
            let mut seen: std::collections::HashSet<String> =
                servers.iter().map(|s| s.ip.clone()).collect();
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
                crate::config::save_instance_servers(&inst_dir, &servers)
                    .map_err(|e| e.to_string())?;
            }
            return Ok(added);
        }
    }
    // Fallback: global settings
    let mut settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    let mut seen: std::collections::HashSet<String> = settings
        .recent_servers
        .iter()
        .map(|s| s.ip.clone())
        .collect();
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
pub fn update_server_last_played(
    ip: String,
    name: String,
    instance_id: Option<String>,
) -> std::result::Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let instance_name = instance_id.as_ref().and_then(|id| {
        crate::core::instance::get_all()
            .ok()
            .and_then(|instances| instances.into_iter().find(|i| i.id == *id).map(|i| i.name))
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
            let mut servers =
                crate::config::load_instance_servers(&inst_dir).map_err(|e| e.to_string())?;
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
    settings
        .recent_servers
        .sort_by(|a, b| b.last_played.cmp(&a.last_played));
    settings.recent_servers.truncate(2);
    crate::config::save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_last_world(
    instance_id: String,
    world_name: String,
) -> std::result::Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
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
        let inst_dir = crate::config::get_data_dir()
            .join("instances")
            .join(&instance_id);
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
pub fn get_last_world(
    instance_id: Option<String>,
) -> std::result::Result<serde_json::Value, String> {
    if let Some(ref id) = instance_id {
        if !id.is_empty() {
            let inst_dir = crate::config::get_data_dir().join("instances").join(id);
            let world =
                crate::config::load_instance_last_world(&inst_dir).map_err(|e| e.to_string())?;
            return Ok(serde_json::to_value(world.as_ref()).unwrap_or(serde_json::Value::Null));
        }
    }
    // Fallback: global settings
    let settings = crate::config::load_settings().map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(settings.last_world.as_ref()).unwrap_or(serde_json::Value::Null))
}

#[tauri::command]
pub async fn launch_game(
    app: AppHandle,
    instance_id: String,
    version_id: String,
    username: String,
    uuid: String,
    token: String,
    acc_type: String,
    server_ip: String,
    world_name: Option<String>,
) -> std::result::Result<String, String> {
    if let Ok(s) = crate::config::load_settings() {
        if s.token_refresh_on_instance_launch {
            let _ = refresh_microsoft_sessions_startup(app.clone()).await;
        }
    }
    if let Some(ref w) = world_name {
        let _ = update_last_world(instance_id.clone(), w.clone());
    }
    let server = if server_ip.is_empty() {
        None
    } else {
        Some(server_ip.as_str())
    };
    let world = world_name.as_deref();
    crate::core::fluxcore::conductor::launch(
        app,
        &instance_id,
        &version_id,
        &username,
        &uuid,
        &token,
        &acc_type,
        server,
        world,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fluxcore_launch(
    app: AppHandle,
    instance_id: String,
    version_id: String,
    username: String,
    uuid: String,
    token: String,
    acc_type: String,
    server_ip: Option<String>,
    world_name: Option<String>,
) -> std::result::Result<String, String> {
    crate::core::fluxcore::conductor::launch(
        app,
        &instance_id,
        &version_id,
        &username,
        &uuid,
        &token,
        &acc_type,
        server_ip.as_deref(),
        world_name.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
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
pub async fn get_pack_source_info(
    instance_id: String,
) -> std::result::Result<Option<serde_json::Value>, String> {
    let inst_dir = crate::config::get_data_dir()
        .join("instances")
        .join(&instance_id);
    let source = crate::config::load_pack_source(&inst_dir).map_err(|e| e.to_string())?;
    let json = match source {
        Some(crate::config::PackSource::Modrinth {
            project_id,
            version_id,
            version_name,
            ..
        }) => {
            let mut j = serde_json::json!({ "source": "modrinth", "project_id": project_id, "version_id": version_id, "version_name": version_name });
            if let Ok(proj) = crate::core::modrinth::get_project(&project_id).await {
                if let Some(url) = proj.get("icon_url").and_then(|v| v.as_str()) {
                    j["icon_url"] = serde_json::json!(url);
                }
            }
            j
        }
        Some(crate::config::PackSource::Curseforge {
            project_id,
            file_id,
            version_name,
            ..
        }) => {
            let mut j = serde_json::json!({
                "source": "curseforge",
                "project_id": project_id.clone(),
                "version_id": file_id,
                "version_name": version_name,
            });
            if let Ok(proj) = crate::core::curseforge::get_project(&project_id).await {
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
pub async fn fetch_launcher_news(
) -> std::result::Result<Vec<crate::core::updater::NewsItem>, String> {
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
    let info = crate::core::updater::check_update()
        .await?
        .ok_or("No update available")?;
    let key = if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    let release = info
        .platforms
        .get(key)
        .ok_or("No release for this platform")?;
    let path = crate::core::updater::download_update(release).await?;

    #[cfg(target_os = "windows")]
    {
        crate::core::updater::schedule_replace_running_exe_with(&path)?;
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
        std::process::Command::new(&current)
            .spawn()
            .map_err(|e| e.to_string())?;
        std::process::exit(0);
    }
}

/// ОС, под которую собран бинарник лаунчера (для UI: блоки только для Linux и т.д.).
#[tauri::command]
pub fn runtime_os() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "other"
    }
}

#[tauri::command]
pub fn get_system_ram() -> u64 {
    crate::core::utils::system::get_system_ram()
}
#[tauri::command]
pub async fn ping_server(ip: String) -> std::result::Result<Value, String> {
    crate::core::utils::system::ping_server(&ip)
        .await
        .map_err(|e| e.diagnostic_report())
}
#[tauri::command]
pub async fn get_loader_versions(
    loader: String,
    include_snapshots: Option<bool>,
    include_alpha_beta: Option<bool>,
) -> std::result::Result<Vec<String>, String> {
    crate::core::utils::system::get_loader_versions(&loader, include_snapshots, include_alpha_beta)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ui_game_versions(
    include_snapshots: bool,
    include_alpha_beta: bool,
) -> std::result::Result<Vec<String>, String> {
    crate::core::loader_meta::lists::modrinth_game_versions(include_snapshots, include_alpha_beta)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn get_specific_loader_versions(
    loader: String,
    game_version: String,
) -> std::result::Result<Vec<String>, String> {
    crate::core::utils::system::get_specific_loader_versions(&loader, &game_version)
        .await
        .map_err(|e| e.to_string())
}

// =============== Команды оверлея (Phase 4, 2.0) ===============

/// Останавливает активную игровую сессию. Используется кнопкой «Стоп» в оверлее
/// и в будущих хук-API. Если `instance_id` пуст — останавливает первую найденную
/// активную сессию (удобно, когда оверлей не знает id конкретного запуска).
#[tauri::command]
pub fn stop_game_from_overlay(instance_id: Option<String>) -> std::result::Result<(), String> {
    let target: Option<String> = match instance_id {
        Some(s) if !s.trim().is_empty() => Some(s),
        _ => crate::core::game::launch::running_instance_ids()
            .into_iter()
            .next(),
    };
    if let Some(id) = target {
        crate::core::game::launch::stop_instance(&id).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Возвращает последние N строк из logs/latest.log конкретного инстанса.
/// Если файла нет или прочесть не удалось — вернёт пустую строку (не ошибку),
/// чтобы оверлей мог спокойно отображать «нет логов».
#[tauri::command]
pub fn tail_game_log(
    instance_id: String,
    lines: Option<usize>,
) -> std::result::Result<String, String> {
    let n = lines.unwrap_or(120).clamp(1, 2000);
    let inst_dir = crate::config::get_data_dir()
        .join("instances")
        .join(&instance_id);
    let log_path = inst_dir.join("logs").join("latest.log");
    if !log_path.exists() {
        return Ok(String::new());
    }
    let Ok(content) = std::fs::read_to_string(&log_path) else {
        return Ok(String::new());
    };
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(n);
    Ok(all[start..].join("\n"))
}

/// Делает скриншот активного окна Minecraft (или всего экрана) и сохраняет его
/// в `<instance>/screenshots/launcher-YYYYMMDD-HHMMSS.png`. Работает через
/// системные утилиты на Linux (grim → maim → scrot → import) и PowerShell на
/// Windows. Возвращает путь к файлу. Если ни одна утилита недоступна — ошибка.
#[tauri::command]
pub fn take_minecraft_screenshot(
    instance_id: Option<String>,
) -> std::result::Result<String, String> {
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    let target_id = match instance_id {
        Some(s) if !s.trim().is_empty() => Some(s),
        _ => crate::core::game::launch::running_instance_ids()
            .into_iter()
            .next(),
    };

    let base_dir = if let Some(id) = target_id.as_ref() {
        let d = crate::config::get_data_dir()
            .join("instances")
            .join(id)
            .join("screenshots");
        d
    } else {
        crate::config::get_data_dir().join("screenshots")
    };
    std::fs::create_dir_all(&base_dir).map_err(|e| e.to_string())?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let out = base_dir.join(format!("launcher-{ts}.png"));
    let out_str = out.to_string_lossy().to_string();

    #[cfg(target_os = "linux")]
    {
        let candidates: [(&str, Vec<String>); 4] = [
            ("grim", vec![out_str.clone()]),
            ("maim", vec![out_str.clone()]),
            ("scrot", vec![out_str.clone()]),
            (
                "import",
                vec!["-window".into(), "root".into(), out_str.clone()],
            ),
        ];
        let mut last_err = String::from(
            "не найдена ни одна утилита скриншота (grim/maim/scrot/import)",
        );
        for (bin, args) in candidates {
            match Command::new(bin).args(&args).status() {
                Ok(st) if st.success() => return Ok(out_str),
                Ok(st) => {
                    last_err = format!("{bin} завершился с кодом {}", st.code().unwrap_or(-1));
                }
                Err(e) => {
                    last_err = format!("{bin}: {e}");
                }
            }
        }
        return Err(last_err);
    }

    #[cfg(target_os = "windows")]
    {
        let ps = format!(
            "Add-Type -AssemblyName System.Drawing,System.Windows.Forms; \
             $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds; \
             $bmp = New-Object System.Drawing.Bitmap $b.Width,$b.Height; \
             $g = [System.Drawing.Graphics]::FromImage($bmp); \
             $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size); \
             $bmp.Save('{}', [System.Drawing.Imaging.ImageFormat]::Png);",
            out_str.replace("'", "''")
        );
        let st = Command::new("powershell")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps])
            .status()
            .map_err(|e| e.to_string())?;
        if !st.success() {
            return Err(format!("powershell завершился с кодом {}", st.code().unwrap_or(-1)));
        }
        return Ok(out_str);
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Err("скриншот не поддержан на этой ОС".into())
    }
}

// Команды управления окном вынесены в `commands::window` (Phase 3.1).
// ================= ПОДМОДУЛИ (Phase 3.1 — декомпозиция commands.rs) =================
pub mod backgrounds;
pub mod window;

pub use backgrounds::*;
pub use window::*;
