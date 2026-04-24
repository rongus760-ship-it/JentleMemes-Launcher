use crate::config::get_data_dir;
use crate::core::api;
use crate::core::game::version_layout::{
    self, is_forge_profile_id, is_neoforge_profile_id, modded_profile_id,
};
use crate::core::progress_emit::emit_download_progress;
use crate::core::types::{DownloadProgress, Library, VersionInfo};
use crate::core::utils::download::download_file;
use crate::core::utils::maven::{
    libraries_minecraft_to_maven_central, library_rel_path_under_libraries, maven_to_path,
    normalize_legacy_forge_minecraftforge_coord, resolve_net_minecraft_client_dir_name,
};
use crate::error::{Error, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Semaphore;

/// JentleWrapper (встроенный ForgeWrapper) без сети — в `libraries/` до прохода загрузок.
fn materialize_embedded_vendor_library(data_dir: &Path, lib: &Library) {
    if !lib.name.starts_with("wtf.jentlememes:wrapper:") {
        return;
    }
    let path = library_rel_path_under_libraries(lib);
    let dest = data_dir.join("libraries").join(&path);
    if dest.is_file() {
        return;
    }
    if let Some(p) = dest.parent() {
        let _ = fs::create_dir_all(p);
    }
    let bytes = crate::core::loader_meta::embedded_jar_bytes();
    let _ = fs::write(&dest, bytes);
}

/// Установка ванильного профиля.
///
/// Fast path: если `versions/<id>/<id>.json` уже на диске — не ходим в Mojang API.
/// LibraryTab зовёт нас на каждом «Играть», и лишний HTTP-запрос к launchermeta.mojang.com
/// добавлял 300–1500 мс к холодному старту (а на плохой сети — несколько секунд) без
/// какой-либо пользы: манифест версии неизменный, Mojang не правит его задним числом.
/// Prism/MultiMC работают так же — качают манифест только при явном «Update» или когда
/// профиля нет.
pub async fn install_vanilla(version: &str) -> Result<String> {
    let data_dir = get_data_dir();
    let profile_path = version_layout::profile_json_path(&data_dir, version);
    if profile_path.is_file() {
        return Ok(version.to_string());
    }
    let data = api::get_vanilla_version(version).await?;
    save_profile(version, &data)?;
    Ok(version.to_string())
}

fn legacy_forge_neoforge_profile_has_installer_maven(existing: &Value) -> bool {
    existing
        .get("mavenFiles")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().any(|e| {
                e.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.contains(":installer"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn forge_neoforge_install_loader_skip_rewrite_disk(
    loader: &str,
    existing: &Value,
    main_class: &str,
) -> bool {
    let mcl = main_class.to_lowercase();
    if mcl.contains("forgewrapper") {
        return false;
    }
    if mcl.contains("bootstraplauncher") {
        return true;
    }
    if (loader == "forge" || loader == "neoforge") && mcl.contains("launchwrapper") {
        if !legacy_forge_neoforge_profile_has_installer_maven(existing) {
            return false;
        }
    }
    if loader == "forge" && mcl.contains("launchwrapper") {
        let Some(args) = existing.get("minecraftArguments").and_then(|x| x.as_str()) else {
            return false;
        };
        let a = args.to_lowercase();
        let has_forge_entrypoint = a.contains("tweakclass")
            || a.contains("fmltweaker")
            || a.contains("legacylauncher")
            || a.contains("fmlsetup");
        return has_forge_entrypoint;
    }
    true
}

pub async fn install_loader(
    game_version: &str,
    loader: &str,
    loader_version: &str,
) -> Result<String> {
    let alpha = crate::config::load_settings()
        .map(|s| s.enable_alpha_loaders)
        .unwrap_or(false);
    if matches!(loader, "liteloader" | "modloader") && !alpha {
        return Err(Error::Custom(
            "Включите «Альфа: LiteLoader и ModLoader» в расширенных настройках.".into(),
        ));
    }
    let game_version = game_version.trim();
    if game_version.is_empty() {
        return Err(Error::Custom("Не указана версия Minecraft".into()));
    }

    // Fast path: если профиль модлоадера уже собран на диске — не ходим в сеть.
    // `target_id` детерминирован от (loader, loader_version, game_version), так что
    // при его существовании ни FabricMC, ни Mojang запрашивать не нужно: JSON не
    // меняется. Это критично для тёплого старта — без этого каждый запуск весит
    // ~1-3 секунды сетевых ретраев на запрос manifest + loader-patch.
    // Для Forge/NeoForge fast-path отключён: там есть последующий промо-путь
    // (ForgeWrapper → BootstrapLauncher), который требует полной сборки patch_data.
    let lv_id_early = api::normalize_loader_version(loader_version);
    let target_id_early = modded_profile_id(loader, &lv_id_early, game_version);
    if !matches!(loader, "forge" | "neoforge") {
        let existing_path = version_layout::profile_json_path(&get_data_dir(), &target_id_early);
        if existing_path.is_file() {
            return Ok(target_id_early);
        }
    }

    let vanilla_data = api::get_vanilla_version(game_version).await?;
    let patch_data = api::get_loader_patch(game_version, loader, loader_version).await?;

    let mut final_data = vanilla_data.clone();

    if let Some(main_class) = patch_data.get("mainClass").and_then(|v| v.as_str()) {
        final_data["mainClass"] = Value::String(main_class.to_string());
    }
    if let Some(patch_libs) = patch_data.get("libraries").and_then(|v| v.as_array()) {
        let mut base_libs = final_data
            .get("libraries")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        base_libs.extend(patch_libs.clone());
        final_data["libraries"] = Value::Array(base_libs);
    }
    if final_data.get("arguments").is_none() {
        final_data["arguments"] = serde_json::json!({ "jvm": [], "game": [] });
    }

    if let Some(mc_args) = patch_data
        .get("minecraftArguments")
        .and_then(|v| v.as_str())
    {
        final_data["minecraftArguments"] = Value::String(mc_args.to_string());
        // minecraftArguments is a complete spec; remove arguments.game to avoid duplication
        if let Some(args) = final_data
            .get_mut("arguments")
            .and_then(|v| v.as_object_mut())
        {
            args.remove("game");
        }
    }

    if let Some(args) = final_data
        .get_mut("arguments")
        .and_then(|v| v.as_object_mut())
    {
        if let Some(jvm_patch) = patch_data.get("+jvmArgs").and_then(|v| v.as_array()) {
            let mut jvm_base = args
                .get("jvm")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            jvm_base.extend(jvm_patch.clone());
            args.insert("jvm".to_string(), Value::Array(jvm_base));
        }
        if let Some(game_patch) = patch_data.get("+gameArgs").and_then(|v| v.as_array()) {
            let mut game_base = args
                .get("game")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            game_base.extend(game_patch.clone());
            args.insert("game".to_string(), Value::Array(game_base));
        }
    }

    // Некоторые патчи (легаси Forge) используют "+tweakers" вместо полного minecraftArguments
    // that need --tweakClass arguments appended to minecraftArguments
    if let Some(tweakers) = patch_data.get("+tweakers").and_then(|v| v.as_array()) {
        let mut mc_args = final_data
            .get("minecraftArguments")
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
            if let Some(args) = final_data
                .get_mut("arguments")
                .and_then(|v| v.as_object_mut())
            {
                args.remove("game");
            }
        }
    }

    if let Some(maven_files) = patch_data.get("mavenFiles") {
        final_data["mavenFiles"] = maven_files.clone();
    }

    let lv_id = api::normalize_loader_version(loader_version);
    let target_id = modded_profile_id(loader, &lv_id, game_version);
    final_data["id"] = Value::String(target_id.clone());
    final_data["inheritsFrom"] = Value::String(game_version.to_string());

    // `install_forge` вызывается перед каждым запуском из UI. После успешной установки Forge/NeoForge
    // инсталлер перезаписывает этот JSON (mainClass → BootstrapLauncher и т.д.). Повторная запись
    // патча с ForgeWrapper откатывает профиль и снова запускает только установщик.
    if loader == "forge" || loader == "neoforge" {
        let existing = version_layout::profile_json_path(&get_data_dir(), &target_id);
        if let Ok(txt) = fs::read_to_string(&existing) {
            if let Ok(v) = serde_json::from_str::<Value>(&txt) {
                if let Some(mc) = v.get("mainClass").and_then(|x| x.as_str()) {
                    if forge_neoforge_install_loader_skip_rewrite_disk(loader, &v, mc) {
                        return Ok(target_id);
                    }
                }
            }
        }
    }

    save_profile(&target_id, &final_data)?;
    Ok(target_id)
}

/// Запись профиля версии на диск.
///
/// КРИТИЧНО для LaunchCache: пишем только если контент *действительно* изменился.
/// LibraryTab на каждом `Играть` вызывает `install_version` → `install_vanilla` →
/// `save_profile`, а также `install_fabric/quilt/forge/...` → `install_loader` →
/// `save_profile`. Если мы переписываем файл байт-в-байт теми же данными, mtime
/// меняется, `compute_chain_hash` возвращает новый SHA → `LaunchCache::load()`
/// всегда получает `chain_match=false`. Тёплый путь бесполезен.
///
/// Сравнение идёт по сериализованной строке: если JSON эквивалентен текущему
/// содержимому файла — ничего не делаем, сохраняя mtime. Иначе — атомарный rename.
fn save_profile(id: &str, json: &Value) -> Result<()> {
    let data_dir = get_data_dir();
    let dir = version_layout::profile_dir(&data_dir, id);
    fs::create_dir_all(&dir)?;
    let dest = version_layout::profile_json_path(&data_dir, id);

    let new_text = serde_json::to_string_pretty(json)?;
    if let Ok(existing) = fs::read_to_string(&dest) {
        if existing == new_text {
            // Файл уже содержит ровно этот JSON — не трогаем mtime, иначе
            // LaunchCache::chain_hash будет отличаться на каждом запуске.
            return Ok(());
        }
    }

    let tmp = dest.with_extension("json.tmp");
    fs::write(&tmp, &new_text)?;
    fs::rename(&tmp, &dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        Error::Custom(format!("Atomic save_profile: {e}"))
    })?;
    Ok(())
}

/// JAR установщика Forge/NeoForge: копия в versions/…/installer.jar или mavenFiles (classifier `installer`).
pub(crate) fn forge_installer_jar_path(
    version_id: &str,
    current: &Value,
    data_dir: &Path,
) -> Option<std::path::PathBuf> {
    let cand = version_layout::profile_dir(data_dir, version_id).join("installer.jar");
    if cand.is_file() {
        return Some(cand);
    }
    let arr = current.get("mavenFiles")?.as_array()?;
    for f in arr {
        let name = f.get("name").and_then(|v| v.as_str())?;
        let parts: Vec<&str> = name.split(':').collect();
        if parts.len() < 4 {
            continue;
        }
        let classifier = parts[3].split('@').next().unwrap_or("");
        if classifier != "installer" {
            continue;
        }
        let lib: Library = serde_json::from_value(f.clone()).ok()?;
        let rel = library_rel_path_under_libraries(&lib);
        let full = data_dir.join("libraries").join(rel);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

fn read_version_json_from_installer_jar(jar: &Path) -> Result<Value> {
    let file = fs::File::open(jar)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut ent = archive
        .by_name("version.json")
        .map_err(|e| Error::Custom(format!("нет version.json в {}: {}", jar.display(), e)))?;
    let mut s = String::new();
    ent.read_to_string(&mut s)?;
    serde_json::from_str(&s)
        .map_err(|e| Error::Custom(format!("version.json в {}: {}", jar.display(), e)))
}

fn read_install_profile_from_installer_jar(jar: &Path) -> Result<Value> {
    let file = fs::File::open(jar)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut ent = archive.by_name("install_profile.json").map_err(|e| {
        Error::Custom(format!(
            "нет install_profile.json в {}: {}",
            jar.display(),
            e
        ))
    })?;
    let mut s = String::new();
    ent.read_to_string(&mut s)?;
    serde_json::from_str(&s)
        .map_err(|e| Error::Custom(format!("install_profile.json в {}: {}", jar.display(), e)))
}

fn forge_processor_outputs_exist(installer_jar: &Path, lib_dir: &Path) -> bool {
    let ip = match read_install_profile_from_installer_jar(installer_jar) {
        Ok(v) => v,
        Err(_) => return true,
    };
    let data = match ip.get("data").and_then(|d| d.as_object()) {
        Some(d) => d,
        None => return true,
    };
    for key in ["PATCHED", "MC_SLIM", "MC_EXTRA", "MC_SRG"] {
        if let Some(entry) = data.get(key).and_then(|v| v.get("client")).and_then(|v| v.as_str()) {
            let trimmed = entry.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let maven_coord = &trimmed[1..trimmed.len() - 1];
                let parts: Vec<&str> = maven_coord.split(':').collect();
                if parts.len() >= 3 {
                    let classifier = if parts.len() >= 4 { Some(parts[3]) } else { None };
                    let rel = maven_to_path(
                        &format!("{}:{}:{}", parts[0], parts[1], parts[2]),
                        classifier,
                    );
                    let full = lib_dir.join(&rel);
                    if !full.is_file() {
                        eprintln!(
                            "[JentleMemes] Forge promote blocked: processor output missing: {} ({})",
                            key, full.display()
                        );
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn neoforge_mcp_client_version_from_install_profile(install_profile: &Value) -> Option<String> {
    let raw = install_profile
        .get("data")?
        .get("MCP_VERSION")?
        .get("client")?
        .as_str()?;
    let trimmed = raw.trim().trim_matches(|c| c == '\'' || c == '"');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// `MCP_VERSION.client` из `install_profile.json` установщика для `version_id` (каталог `client-*-srg` / `mcAndNeoForm` в FML).
pub fn neoforge_mcp_client_from_version_folder(
    data_dir: &Path,
    version_id: &str,
) -> Option<String> {
    let v_path = version_layout::profile_json_path(data_dir, version_id);
    let s = fs::read_to_string(&v_path).ok()?;
    let current: Value = serde_json::from_str(&s).ok()?;
    let jar = forge_installer_jar_path(version_id, &current, data_dir)?;
    let install_profile = read_install_profile_from_installer_jar(&jar).ok()?;
    neoforge_mcp_client_version_from_install_profile(&install_profile)
}

fn mc_prefix_before_forge_in_version_field(v: &str) -> Option<String> {
    const NEEDLES: [&str; 2] = ["-forge-", "-Forge-"];
    for n in NEEDLES {
        if let Some(i) = v.find(n) {
            let mc = v[..i].trim();
            if !mc.is_empty() {
                return Some(mc.to_string());
            }
        }
    }
    None
}

/// Версия Minecraft из `install_profile.json` (Forge / NeoForge / легаси), как в путях `client-<ver>-official.jar`.
fn minecraft_version_from_install_profile_value(install_profile: &Value) -> Option<String> {
    if let Some(m) = install_profile.get("minecraft") {
        if let Some(t) = m.as_str().map(str::trim).filter(|s| !s.is_empty()) {
            return Some(t.to_string());
        }
        if let Some(t) = m
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Some(t.to_string());
        }
    }
    if let Some(v) = install_profile.get("version").and_then(|v| v.as_str()) {
        if let Some(mc) = mc_prefix_before_forge_in_version_field(v) {
            return Some(mc);
        }
    }
    None
}

/// Версия Minecraft из `install_profile.json` внутри `…-installer.jar` (Forge / NeoForge).
pub fn forge_minecraft_version_from_installer_jar(jar: &Path) -> Option<String> {
    let install_profile = read_install_profile_from_installer_jar(jar).ok()?;
    minecraft_version_from_install_profile_value(&install_profile)
}

fn collect_mc_hints(chain_jar_id: &str, extra: &[String]) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let push = |out: &mut Vec<String>, s: &str| {
        let t = s.trim();
        if !t.is_empty() && !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    };
    push(&mut order, chain_jar_id);
    for e in extra {
        push(&mut order, e);
    }
    order
}

/// Гарантирует наличие `libraries/net/minecraft/client/<mc>/client-<mc>-official.jar`.
/// Сначала проверяет, есть ли файл; затем пробует скопировать из `versions/<mc>/<mc>.jar`;
/// только в крайнем случае скачивает с Mojang. Возвращает путь к первому готовому jar.
pub async fn download_official_client_jar_to_libraries(
    data_dir: &Path,
    hints: &[String],
) -> Result<PathBuf> {
    let lib_client = data_dir.join("libraries/net/minecraft/client");
    fs::create_dir_all(&lib_client)?;

    for mc in hints {
        let dest = lib_client.join(mc).join(format!("client-{mc}-official.jar"));
        if dest.is_file() {
            return Ok(dest);
        }
    }

    for mc in hints {
        let dest = lib_client.join(mc).join(format!("client-{mc}-official.jar"));
        let src = data_dir.join("versions").join(mc).join(format!("{mc}.jar"));
        if src.is_file() {
            if let Some(p) = dest.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(&src, &dest)?;
            println!("[JentleMemes] client-official: скопирован из versions/{mc}/{mc}.jar");
            return Ok(dest);
        }
    }

    let manifest_body = reqwest::get(
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
    )
    .await
    .map_err(|e| Error::Custom(format!("Mojang manifest: {e}")))?
    .text()
    .await
    .map_err(|e| Error::Custom(format!("Mojang manifest body: {e}")))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_body)?;
    let versions = manifest
        .get("versions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::Custom("manifest: нет versions[]".into()))?;

    for mc in hints {
        let dest = lib_client.join(mc).join(format!("client-{mc}-official.jar"));
        if dest.is_file() {
            return Ok(dest);
        }

        let version_url = versions
            .iter()
            .find(|v| v.get("id").and_then(|i| i.as_str()) == Some(mc))
            .and_then(|v| v.get("url").and_then(|u| u.as_str()))
            .map(String::from);

        let Some(ver_url) = version_url else {
            eprintln!("[JentleMemes] Mojang manifest: нет версии {mc}");
            continue;
        };

        let ver_body = reqwest::get(&ver_url)
            .await
            .map_err(|e| Error::Custom(format!("Mojang version JSON {mc}: {e}")))?
            .text()
            .await
            .map_err(|e| Error::Custom(format!("Mojang version JSON body {mc}: {e}")))?;
        let ver_json: serde_json::Value = serde_json::from_str(&ver_body)?;

        let client_url = ver_json
            .pointer("/downloads/client/url")
            .and_then(|v| v.as_str())
            .or_else(|| {
                ver_json
                    .pointer("/mainJar/downloads/artifact/url")
                    .and_then(|v| v.as_str())
            })
            .map(String::from);

        let Some(url) = client_url else {
            eprintln!("[JentleMemes] Нет URL client.jar для {mc}");
            continue;
        };

        println!("[JentleMemes] Скачиваем ванильный client {mc} → {}", dest.display());
        let bytes = download_file(&url, None).await?;
        if let Some(p) = dest.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(&dest, &bytes)?;
        println!("[JentleMemes] Ванильный client {mc}: {} байт → {:?}", bytes.len(), dest);

        let versions_jar = data_dir.join("versions").join(mc).join(format!("{mc}.jar"));
        if !versions_jar.is_file() {
            if let Some(p) = versions_jar.parent() {
                let _ = fs::create_dir_all(p);
            }
            let _ = fs::write(&versions_jar, &bytes);
        }
        return Ok(dest);
    }

    Err(Error::Custom(format!(
        "Не удалось подготовить ванильный client.jar ни для одной из версий: {}",
        hints.join(", ")
    )))
}

/// Копирует vanilla `versions/<id>/<id>.jar` (путь в `jar_path`) в `libraries/net/minecraft/client/.../client-*-official.jar`
/// и в папку профиля — как ожидают install_profile / постпроцессоры Forge и NeoForge.
/// `extra_plain_mc_versions` — версии из `install_profile` установщика и т.п.; путь всегда **нижний** `…/client/<версия>/`.
/// Безопасно вызывать повторно (пропускает уже существующие файлы).
pub fn ensure_official_client_jar_for_forge_libraries(
    data_dir: &Path,
    version_id: &str,
    jar_path: &Path,
    extra_plain_mc_versions: &[String],
) -> Result<()> {
    if !jar_path.is_file() {
        return Ok(());
    }
    let Some(vanilla_versions_id) = jar_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(());
    };
    let lib_root = data_dir.join("libraries");
    let client_root = lib_root.join("net").join("minecraft").join("client");
    fs::create_dir_all(&client_root)?;

    let push_id = |out: &mut Vec<String>, s: &str| {
        let t = s.trim();
        if !t.is_empty() && !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    };
    let mut plain_ids: Vec<String> = Vec::new();
    push_id(&mut plain_ids, vanilla_versions_id);
    for e in extra_plain_mc_versions {
        push_id(&mut plain_ids, e);
    }
    if let Some(m) = version_layout::minecraft_version_from_profile_id(version_id) {
        push_id(&mut plain_ids, &m);
    }

    let dir_name = resolve_net_minecraft_client_dir_name(vanilla_versions_id, &lib_root);
    push_id(&mut plain_ids, &dir_name);

    // Каталог с timestamp (1.21.6-…) для Maven-артефактов slim/extra
    let ts_jar = client_root
        .join(&dir_name)
        .join(format!("client-{dir_name}-official.jar"));
    if !ts_jar.is_file() {
        if let Some(p) = ts_jar.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(jar_path, &ts_jar)?;
        println!(
            "[DEBUG] Client.jar → Forge maven dir «{}»: {:?}",
            dir_name, ts_jar
        );
    }

    for mc in &plain_ids {
        let dest = client_root
            .join(mc)
            .join(format!("client-{mc}-official.jar"));
        if dest.is_file() {
            continue;
        }
        if let Some(p) = dest.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(jar_path, &dest)?;
        println!("[DEBUG] Client.jar → libraries/net/minecraft/client/{mc}/ → {:?}", dest);
    }

    let mc_stem = version_layout::minecraft_version_from_profile_id(version_id)
        .unwrap_or_else(|| vanilla_versions_id.to_string());
    let profile_official =
        version_layout::profile_dir(data_dir, version_id).join(format!("client-{mc_stem}-official.jar"));
    if !profile_official.exists() {
        if let Some(p) = profile_official.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(jar_path, &profile_official)?;
        println!(
            "[DEBUG] Client-official в папку профиля: {:?}",
            profile_official
        );
    }
    Ok(())
}

fn jar_signature_meta_path(name: &str) -> bool {
    if !name.starts_with("META-INF/") {
        return false;
    }
    let f = name.trim_start_matches("META-INF/");
    f.ends_with(".SF")
        || f.ends_with(".RSA")
        || f.ends_with(".DSA")
        || f.ends_with(".EC")
}

fn patched_installer_output_path(installer_path: &Path) -> PathBuf {
    let stem = installer_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("installer");
    installer_path.with_file_name(format!("{stem}.patched-unsigned.jar"))
}

pub(crate) fn clear_native_artifact_files(dir: &Path) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let drop = p
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|ext| matches!(ext, "so" | "dll" | "dylib"));
        if drop {
            let _ = fs::remove_file(p);
        }
    }
}

fn manifest_without_named_entry_section(manifest: &str, entry_logical_path: &str) -> String {
    let needle = format!("Name: {}", entry_logical_path);
    let mut out: Vec<&str> = Vec::new();
    let mut skipping = false;
    for line in manifest.lines() {
        if line.starts_with("Name:") {
            if skipping {
                skipping = false;
            }
            if line.trim_end() == needle.as_str() {
                skipping = true;
                continue;
            }
        }
        if skipping {
            continue;
        }
        out.push(line);
    }
    let s = out.join("\n");
    if s.is_empty() {
        "Manifest-Version: 1.0\n".to_string()
    } else {
        format!("{}\n", s.trim_end())
    }
}

/// Патчит `installer.jar`: в `install_profile.json` у всех процессоров очищает `"outputs"`,
/// чтобы процессорный фреймворк Forge не удалял файлы с несовпавшим хешем перед запуском
/// (именно это ломает FART, когда input == output == `client-<MC>-official.jar`).
/// Пересборка JAR без снятия PKCS7/`META-INF/*.SF` даёт `SecurityException: SHA-384 digest error`
/// при чтении `install_profile.json` — подпись и содержимое расходятся.
/// Возвращает путь к пропатченному installer (рядом с оригиналом, `*.patched-unsigned.jar`).
pub fn patch_installer_remove_processor_outputs(installer_path: &Path) -> Result<PathBuf> {
    let patched = patched_installer_output_path(installer_path);
    if patched.is_file() {
        return Ok(patched);
    }
    use zip::{CompressionMethod, ZipArchive, ZipWriter, write::FileOptions};

    let file = fs::File::open(installer_path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut profile_raw = String::new();
    let mut has_profile = false;
    if let Ok(mut entry) = archive.by_name("install_profile.json") {
        entry.read_to_string(&mut profile_raw)?;
        has_profile = true;
    }

    if !has_profile || profile_raw.is_empty() {
        let _ = fs::copy(installer_path, &patched);
        return Ok(patched);
    }

    let mut profile: Value = serde_json::from_str(&profile_raw)?;
    let mut changed = false;
    if let Some(processors) = profile.get_mut("processors").and_then(|v| v.as_array_mut()) {
        for proc_val in processors.iter_mut() {
            if let Some(obj) = proc_val.as_object_mut() {
                if obj.contains_key("outputs") {
                    obj.insert("outputs".to_string(), serde_json::json!({}));
                    changed = true;
                }
            }
        }
    }
    if !changed {
        let _ = fs::copy(installer_path, &patched);
        return Ok(patched);
    }

    let new_profile = serde_json::to_string_pretty(&profile)?;

    let out_file = fs::File::create(&patched)?;
    let mut zip_out = ZipWriter::new(out_file);
    let mut buf = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if jar_signature_meta_path(&name) {
            continue;
        }
        let method = match entry.compression() {
            zip::CompressionMethod::Stored => CompressionMethod::Stored,
            _ => CompressionMethod::Deflated,
        };
        let opts = FileOptions::default().compression_method(method);
        buf.clear();
        entry.read_to_end(&mut buf)?;

        if name == "install_profile.json" {
            let mo = FileOptions::default().compression_method(CompressionMethod::Deflated);
            zip_out.start_file(&name, mo)?;
            zip_out.write_all(new_profile.as_bytes())?;
        } else if name == "META-INF/MANIFEST.MF" {
            let s = String::from_utf8_lossy(&buf);
            let filtered = manifest_without_named_entry_section(&s, "install_profile.json");
            let mo = FileOptions::default().compression_method(CompressionMethod::Deflated);
            zip_out.start_file(&name, mo)?;
            zip_out.write_all(filtered.as_bytes())?;
        } else {
            zip_out.start_file(&name, opts)?;
            zip_out.write_all(&buf)?;
        }
    }
    zip_out.finish()?;
    eprintln!(
        "[JentleMemes] Forge: installer.jar → пропатчен (очищены outputs процессоров): {:?}",
        patched
    );
    Ok(patched)
}

const MODLAUNCHER_LAUNCH_HANDLER_SERVICE: &str =
    "META-INF/services/cpw.mods.modlauncher.api.ILaunchHandlerService";

fn jar_contains_modlauncher_fml_service(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let Ok(mut z) = archive.by_name(MODLAUNCHER_LAUNCH_HANDLER_SERVICE) else {
        return false;
    };
    let mut buf = String::new();
    z.read_to_string(&mut buf).is_ok()
        && buf.contains("FMLClientLaunchProvider")
}

fn installer_forge_universal_entry_len(installer: &Path, ver: &str) -> Option<u64> {
    let inner = format!("forge-{ver}-universal.jar");
    let file = std::fs::File::open(installer).ok()?;
    let mut z = zip::ZipArchive::new(file).ok()?;
    z.by_name(&inner).ok().map(|e| e.size())
}

fn minecraftforge_forge_universal_maven_url(forge_version: &str) -> String {
    format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{v}/forge-{v}-universal.jar",
        v = forge_version
    )
}

fn extract_forge_universal_jar_from_installer_zip(
    installer_bytes: &[u8],
    lib_dir: &Path,
    forge_maven_name: &str,
) -> Result<bool> {
    let parts: Vec<&str> = forge_maven_name.split(':').collect();
    if parts.len() != 3 || parts[1] != "forge" {
        return Ok(false);
    }
    if parts[0] != "net.minecraftforge" && parts[0] != "net.neoforged" {
        return Ok(false);
    }
    let dest = lib_dir.join(maven_to_path(forge_maven_name, Some("universal")));
    let cursor = std::io::Cursor::new(installer_bytes);
    let mut zip = zip::ZipArchive::new(cursor)
        .map_err(|e| Error::Custom(format!("forge installer zip: {e}")))?;
    let ver = parts[2].split('@').next().unwrap_or(parts[2]);
    let group_path = parts[0].replace('.', "/");
    let artifact = parts[1];
    let candidates = [
        format!("forge-{ver}-universal.jar"),
        format!("maven/{group_path}/{artifact}/{ver}/forge-{ver}-universal.jar"),
    ];
    for cand in &candidates {
        let Ok(mut entry) = zip.by_name(cand) else {
            continue;
        };
        let mut out = Vec::new();
        entry.read_to_end(&mut out)?;
        if let Some(p) = dest.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(&dest, &out)?;
        eprintln!(
            "[JentleMemes] Forge legacy: из установщика {} → {}",
            cand,
            dest.display()
        );
        return Ok(true);
    }
    Ok(false)
}

fn forge_universal_bytes_from_installer_zip(
    installer_bytes: &[u8],
    forge_maven_name: &str,
) -> Result<Option<Vec<u8>>> {
    let parts: Vec<&str> = forge_maven_name.split(':').collect();
    if parts.len() != 3 || parts[1] != "forge" {
        return Ok(None);
    }
    if parts[0] != "net.minecraftforge" && parts[0] != "net.neoforged" {
        return Ok(None);
    }
    let ver = parts[2].split('@').next().unwrap_or(parts[2]);
    let group_path = parts[0].replace('.', "/");
    let artifact = parts[1];
    let cursor = std::io::Cursor::new(installer_bytes);
    let mut zip = zip::ZipArchive::new(cursor)
        .map_err(|e| Error::Custom(format!("forge installer zip: {e}")))?;
    let candidates = [
        format!("forge-{ver}-universal.jar"),
        format!("maven/{group_path}/{artifact}/{ver}/forge-{ver}-universal.jar"),
    ];
    for cand in &candidates {
        let Ok(mut entry) = zip.by_name(cand) else {
            continue;
        };
        let mut out = Vec::new();
        entry.read_to_end(&mut out)?;
        return Ok(Some(out));
    }
    Ok(None)
}

fn file_sha1_hex(path: &Path) -> Result<String> {
    use sha1::{Digest, Sha1};
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub async fn ensure_vanilla_versions_jar_for_legacy_fml(
    data_dir: &Path,
    lib_dir: &Path,
    jar_id: &str,
    log: &impl Fn(&str),
) -> Result<()> {
    let json_path = data_dir
        .join("versions")
        .join(jar_id)
        .join(format!("{jar_id}.json"));
    if !json_path.is_file() {
        return Ok(());
    }
    let raw: Value = serde_json::from_str(&fs::read_to_string(&json_path)?)?;
    let sha1_expected = raw
        .pointer("/downloads/client/sha1")
        .and_then(|v| v.as_str())
        .or_else(|| {
            raw.pointer("/mainJar/downloads/artifact/sha1")
                .and_then(|v| v.as_str())
        })
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let Some(sha1_expected) = sha1_expected else {
        return Ok(());
    };
    let jar_path = data_dir
        .join("versions")
        .join(jar_id)
        .join(format!("{jar_id}.jar"));
    let skip_flag = data_dir
        .join("versions")
        .join(jar_id)
        .join(format!("{jar_id}.jar.allow-non-vanilla"));
    if skip_flag.is_file() {
        log(&format!(
            "Legacy FML: проверка {}.jar пропущена (есть {}).",
            jar_id,
            skip_flag.display()
        ));
        return Ok(());
    }
    let mut official_paths: Vec<PathBuf> = vec![lib_dir
        .join("net/minecraft/client")
        .join(jar_id)
        .join(format!("client-{jar_id}-official.jar"))];
    let resolved_dir = resolve_net_minecraft_client_dir_name(jar_id, lib_dir);
    if resolved_dir != jar_id {
        official_paths.push(
            lib_dir
                .join("net/minecraft/client")
                .join(&resolved_dir)
                .join(format!("client-{resolved_dir}-official.jar")),
        );
    }
    official_paths.sort();
    official_paths.dedup();

    if jar_path.is_file() {
        let actual = file_sha1_hex(&jar_path)?;
        if actual.eq_ignore_ascii_case(sha1_expected) {
            for p in &official_paths {
                if p.is_file() && !file_sha1_hex(p)?.eq_ignore_ascii_case(sha1_expected) {
                    log(&format!(
                        "Legacy FML: удалён устаревший client-official (не Mojang SHA1): {}",
                        p.display()
                    ));
                    let _ = fs::remove_file(p);
                }
            }
            return Ok(());
        }
        log(&format!(
            "Legacy FML: {}.jar не совпадает с Mojang SHA1 (ожид. {}…, файл {}…) — binpatch FML иначе даёт ASM crash. Перекачиваем.",
            jar_id,
            &sha1_expected[..8.min(sha1_expected.len())],
            &actual[..8.min(actual.len())],
        ));
        let _ = fs::remove_file(&jar_path);
    }
    for p in &official_paths {
        if p.is_file() {
            log(&format!(
                "Legacy FML: удалён client-official до перекачки vanilla: {}",
                p.display()
            ));
            let _ = fs::remove_file(p);
        }
    }
    let url = raw
        .pointer("/downloads/client/url")
        .and_then(|v| v.as_str())
        .or_else(|| {
            raw.pointer("/mainJar/downloads/artifact/url")
                .and_then(|v| v.as_str())
        });
    let Some(url) = url.map(str::trim).filter(|s| !s.is_empty()) else {
        return Err(Error::Custom(format!(
            "Не удалось перекачать {}.jar: в {} нет URL client.",
            jar_id,
            json_path.display()
        )));
    };
    let bytes = download_file(url, Some(sha1_expected)).await?;
    if let Some(p) = jar_path.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(&jar_path, &bytes)?;
    log(&format!(
        "Legacy FML: записан эталонный {}.jar (SHA1 Mojang проверен).",
        jar_id
    ));
    Ok(())
}

pub async fn ensure_legacy_forge_universal_jar_resolved(
    installer_jar: Option<&Path>,
    lib_dir: &Path,
    forge_maven_name: &str,
) -> Result<()> {
    let parts: Vec<&str> = forge_maven_name.split(':').collect();
    if parts.len() != 3 || parts[1] != "forge" {
        return Ok(());
    }
    if parts[0] != "net.minecraftforge" && parts[0] != "net.neoforged" {
        return Ok(());
    }
    let ver = parts[2].split('@').next().unwrap_or(parts[2]);
    let dest = lib_dir.join(maven_to_path(forge_maven_name, Some("universal")));

    if parts[0] == "net.neoforged" {
        let expected_len_installer = installer_jar
            .filter(|p| p.is_file())
            .and_then(|p| installer_forge_universal_entry_len(p, ver));
        if dest.is_file() {
            let len = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
            if let Some(exp) = expected_len_installer {
                if len == exp {
                    return Ok(());
                }
                let _ = fs::remove_file(&dest);
            } else {
                return Ok(());
            }
        }
        if let Some(inst) = installer_jar.filter(|p| p.is_file()) {
            let buf = fs::read(inst)?;
            let _ = extract_forge_universal_jar_from_installer_zip(&buf, lib_dir, forge_maven_name)?;
        }
        return Ok(());
    }

    let url = minecraftforge_forge_universal_maven_url(ver);

    let authoritative: bytes::Bytes =
        match crate::core::utils::download::download_file(&url, None).await {
            Ok(b) => b,
            Err(e) => {
                let Some(inst) = installer_jar.filter(|p| p.is_file()) else {
                    return Err(Error::Custom(format!(
                        "Forge universal: не удалось скачать {} ({e}) и нет установщика",
                        url
                    )));
                };
                let buf = fs::read(inst)?;
                let Some(raw) = forge_universal_bytes_from_installer_zip(&buf, forge_maven_name)? else {
                    return Err(Error::Custom(
                        "Forge universal: в установщике нет forge-*-universal.jar".into(),
                    ));
                };
                bytes::Bytes::from(raw)
            }
        };

    if dest.is_file() {
        if let Ok(cur) = fs::read(&dest) {
            if cur.len() == authoritative.len() && cur == authoritative.as_ref() {
                return Ok(());
            }
        }
        let _ = fs::remove_file(&dest);
    }

    if let Some(p) = dest.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(&dest, &authoritative)?;
    eprintln!(
        "[JentleMemes] Forge legacy: записан эталонный universal {} → {}",
        url,
        dest.display()
    );
    Ok(())
}

pub fn ensure_legacy_forge_universal_jar(
    installer_jar: Option<&Path>,
    lib_dir: &Path,
    forge_maven_name: &str,
) -> Result<()> {
    let parts: Vec<&str> = forge_maven_name.split(':').collect();
    if parts.len() != 3 || parts[1] != "forge" {
        return Ok(());
    }
    if parts[0] != "net.minecraftforge" && parts[0] != "net.neoforged" {
        return Ok(());
    }
    let ver = parts[2].split('@').next().unwrap_or(parts[2]);
    let dest = lib_dir.join(maven_to_path(forge_maven_name, Some("universal")));

    let expected = installer_jar
        .filter(|p| p.is_file())
        .and_then(|p| installer_forge_universal_entry_len(p, ver));

    if dest.is_file() {
        let len = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        if let Some(e) = expected {
            if len == e {
                return Ok(());
            }
        } else {
            return Ok(());
        }
        let _ = fs::remove_file(&dest);
    }

    let Some(inst) = installer_jar.filter(|p| p.is_file()) else {
        return Ok(());
    };
    let buf = fs::read(inst)?;
    let _ = extract_forge_universal_jar_from_installer_zip(&buf, lib_dir, forge_maven_name)?;
    Ok(())
}

pub fn ensure_forge_no_classifier_jar_for_classpath(
    installer_jar: Option<&Path>,
    lib_dir: &Path,
    maven_name: &str,
) -> Result<()> {
    let parts: Vec<&str> = maven_name.split(':').collect();
    if parts.len() != 3 {
        return Ok(());
    }
    let group = parts[0];
    let artifact = parts[1];
    let ver = parts[2];
    if artifact != "forge" {
        return Ok(());
    }
    let (rel_dir, zip_inner_prefix) = if group == "net.minecraftforge" {
        (
            format!("net/minecraftforge/forge/{ver}"),
            format!("maven/net/minecraftforge/forge/{ver}/forge-{ver}.jar"),
        )
    } else if group == "net.neoforged" {
        (
            format!("net/neoforged/forge/{ver}"),
            format!("maven/net/neoforged/forge/{ver}/forge-{ver}.jar"),
        )
    } else {
        return Ok(());
    };
    let dest = lib_dir.join(&rel_dir).join(format!("forge-{ver}.jar"));
    if dest.is_file() && jar_contains_modlauncher_fml_service(&dest) {
        return Ok(());
    }
    let _ = ensure_legacy_forge_universal_jar(installer_jar, lib_dir, maven_name);
    if let Some(inst) = installer_jar.filter(|p| p.is_file()) {
        let extracted: Result<Vec<u8>> = (|| {
            let file = fs::File::open(inst)?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut entry = archive.by_name(&zip_inner_prefix)?;
            let mut v = Vec::new();
            entry.read_to_end(&mut v)?;
            Ok(v)
        })();
        if let Ok(bytes) = extracted {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&dest, bytes)?;
            eprintln!(
                "[JentleMemes] Forge: из installer.jar → {}",
                dest.display()
            );
            return Ok(());
        }
    }
    Ok(())
}

fn neoforge_client_srg_jar_path(data_dir: &Path, mcp_ver: &str) -> PathBuf {
    data_dir
        .join("libraries")
        .join("net")
        .join("minecraft")
        .join("client")
        .join(mcp_ver)
        .join(format!("client-{}-srg.jar", mcp_ver))
}

fn manifest_main_section_has_minecraft_dists(manifest: &str) -> bool {
    let lines: Vec<&str> = manifest.lines().collect();
    let split_at = lines
        .iter()
        .position(|line| line.starts_with("Name:"))
        .unwrap_or(lines.len());
    lines[..split_at]
        .iter()
        .any(|l| l.trim_start().starts_with("Minecraft-Dists:"))
}

/// FML: если `minecraft-client-patched` на -cp без `Minecraft-Dists` в главной секции MANIFEST.MF, срабатывает ветка
/// dev/joined и `NeoForgeDevDistCleaner` падает. Production jar с Maven часто без этого атрибута — добавляем один раз.
fn patch_manifest_insert_minecraft_dists_client(raw: &str) -> String {
    if manifest_main_section_has_minecraft_dists(raw) {
        return raw.to_string();
    }
    let uses_crlf = raw.contains("\r\n");
    let lines: Vec<&str> = raw.lines().collect();
    let split_at = lines
        .iter()
        .position(|line| line.starts_with("Name:"))
        .unwrap_or(lines.len());
    let sep = if uses_crlf { "\r\n" } else { "\n" };
    let mut main_lines: Vec<&str> = lines[..split_at].to_vec();
    while main_lines.last().copied() == Some("") {
        main_lines.pop();
    }
    let mut out: Vec<String> = main_lines.iter().map(|s| (*s).to_string()).collect();
    out.push("Minecraft-Dists: client".to_string());
    out.push(String::new());
    let main_str = out.join(sep);
    if split_at < lines.len() {
        format!("{}{}{}", main_str, sep, lines[split_at..].join(sep))
    } else {
        format!("{}{}", main_str, sep)
    }
}

pub fn ensure_neoforge_patched_jar_minecraft_dists(jar_path: &Path) -> Result<()> {
    use std::fs::File;
    use zip::write::FileOptions;
    use zip::{CompressionMethod, ZipArchive, ZipWriter};

    if !jar_path.is_file() {
        return Ok(());
    }

    let file = File::open(jar_path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut manifest_raw = String::new();
    {
        let mut ent = archive.by_name("META-INF/MANIFEST.MF").map_err(|e| {
            Error::Custom(format!(
                "minecraft-client-patched ({}): нет META-INF/MANIFEST.MF: {e}",
                jar_path.display()
            ))
        })?;
        ent.read_to_string(&mut manifest_raw)?;
    }

    if manifest_main_section_has_minecraft_dists(&manifest_raw) {
        return Ok(());
    }

    let new_manifest = patch_manifest_insert_minecraft_dists_client(&manifest_raw);

    let temp_path = jar_path.with_extension("jar.tmp_dists");
    let out_file = File::create(&temp_path).map_err(|e| {
        Error::Custom(format!(
            "minecraft-client-patched: не удалось создать временный файл: {e}"
        ))
    })?;
    let mut zip_out = ZipWriter::new(out_file);
    let mut buf = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| Error::Custom(format!("minecraft-client-patched: чтение zip: {e}")))?;
        let name = entry.name().to_string();
        let method = match entry.compression() {
            zip::CompressionMethod::Stored => CompressionMethod::Stored,
            _ => CompressionMethod::Deflated,
        };
        let opts = FileOptions::default().compression_method(method);
        buf.clear();
        entry
            .read_to_end(&mut buf)
            .map_err(|e| Error::Custom(format!("minecraft-client-patched: {e}")))?;

        if name == "META-INF/MANIFEST.MF" {
            let mo = FileOptions::default().compression_method(CompressionMethod::Deflated);
            zip_out
                .start_file(&name, mo)
                .map_err(|e| Error::Custom(format!("zip: {e}")))?;
            zip_out
                .write_all(new_manifest.as_bytes())
                .map_err(|e| Error::Custom(format!("zip: {e}")))?;
        } else {
            zip_out
                .start_file(&name, opts)
                .map_err(|e| Error::Custom(format!("zip: {e}")))?;
            zip_out
                .write_all(&buf)
                .map_err(|e| Error::Custom(format!("zip: {e}")))?;
        }
    }

    zip_out
        .finish()
        .map_err(|e| Error::Custom(format!("zip finish: {e}")))?;
    drop(archive);

    fs::remove_file(jar_path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        Error::Custom(format!(
            "minecraft-client-patched: удаление старого jar: {e}"
        ))
    })?;
    fs::rename(&temp_path, jar_path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        Error::Custom(format!("minecraft-client-patched: подмена jar: {e}"))
    })?;

    eprintln!(
        "[JentleMemes] NeoForge: в {} добавлен Minecraft-Dists: client (MANIFEST для FML на classpath).",
        jar_path.display()
    );

    Ok(())
}

/// Все библиотеки из `version.json` внутри установщика, подходящие под текущую ОС, должны уже лежать в `libraries/`.
/// Иначе перевод на Bootstrap даёт запуск с отсутствующими srg/extra/forge-client jar и падение FML (`Invalid paths argument`).
fn inner_bootstrap_library_files_exist(data_dir: &Path, inner: &Value, installer_jar: &Path) -> bool {
    let lib_dir = data_dir.join("libraries");
    let Some(arr) = inner.get("libraries").and_then(|v| v.as_array()) else {
        return true;
    };
    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(target_os = "linux")]
    let os = "linux";
    #[cfg(target_os = "macos")]
    let os = "osx";
    for lib_val in arr {
        let Ok(lib) = serde_json::from_value::<Library>(lib_val.clone()) else {
            continue;
        };
        let name = lib.name.trim();
        if name.is_empty() {
            continue;
        }
        if !check_lib_rules(&lib.rules, os) {
            continue;
        }
        let rel = library_rel_path_under_libraries(&lib);
        if !lib_dir.join(&rel).is_file() {
            if name.starts_with("net.minecraftforge:forge:") || name.starts_with("net.neoforged:forge:")
            {
                let _ = ensure_forge_no_classifier_jar_for_classpath(
                    Some(installer_jar),
                    &lib_dir,
                    name,
                );
            }
        }
    }

    let mut missing: Vec<String> = Vec::new();
    for lib_val in arr {
        let Ok(lib) = serde_json::from_value::<Library>(lib_val.clone()) else {
            continue;
        };
        if lib.name.trim().is_empty() {
            continue;
        }
        if !check_lib_rules(&lib.rules, os) {
            continue;
        }
        let rel = library_rel_path_under_libraries(&lib);
        if !lib_dir.join(&rel).is_file() {
            missing.push(rel.replace('\\', "/"));
        }
    }
    if missing.is_empty() {
        return true;
    }
    let preview: String = missing
        .iter()
        .take(3)
        .map(|s| format!("«{s}»"))
        .collect::<Vec<_>>()
        .join(", ");
    let tail = if missing.len() > 3 {
        format!(" (+ещё {})", missing.len() - 3)
    } else {
        String::new()
    };
    eprintln!(
        "[JentleMemes] Forge promote: не хватает {} {} в libraries ({preview}{tail}). \
Переключение на Bootstrap отложено — это ожидаемо, пока не отработает установщик Forge (ForgeWrapper / постпроцессоры). \
Запустите игру ещё раз после успешной подготовки или «Починить ядро».",
        missing.len(),
        if missing.len() == 1 {
            "файла"
        } else {
            "файлов"
        },
    );
    false
}

/// Патч с ForgeWrapper держит `mainClass` = ForgeWrapper и тащит installer/FART/лишний asm в один `-cp`, из‑за чего после
/// постпроцессоров `BootstrapLauncher` падает с `ResolutionException` (два модуля экспортируют `org.objectweb.asm.tree`).
/// Внутри `installer.jar` лежит каноничный `version.json` с `cpw.mods.bootstraplauncher.BootstrapLauncher` и коротким списком библиотек.
pub fn promote_forge_wrapper_to_bootstrap(version_id: &str) -> Result<()> {
    let data_dir = get_data_dir();
    let v_path = version_layout::profile_json_path(&data_dir, version_id);
    if !v_path.is_file() {
        return Ok(());
    }
    let current: Value = serde_json::from_str(&fs::read_to_string(&v_path)?)?;
    let main = current
        .get("mainClass")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !main.to_lowercase().contains("forgewrapper") {
        return Ok(());
    }
    if !(is_forge_profile_id(version_id) || is_neoforge_profile_id(version_id)) {
        return Ok(());
    }
    let Some(jar) = forge_installer_jar_path(version_id, &current, &data_dir) else {
        eprintln!(
            "[JentleMemes] Forge: installer.jar не найден — профиль остаётся ForgeWrapper (нужна загрузка файлов)."
        );
        return Ok(());
    };
    let inner = match read_version_json_from_installer_jar(&jar) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[JentleMemes] Forge: {}", e);
            return Ok(());
        }
    };
    let Some(mc) = inner.get("mainClass").and_then(|v| v.as_str()) else {
        return Ok(());
    };
    if mc.to_lowercase().contains("forgewrapper") {
        return Ok(());
    }

    if !inner_bootstrap_library_files_exist(&data_dir, &inner, &jar) {
        return Ok(());
    }

    let lib_dir = data_dir.join("libraries");
    if !forge_processor_outputs_exist(&jar, &lib_dir) {
        eprintln!(
            "[JentleMemes] Forge: профиль остаётся ForgeWrapper — процессорные артефакты ещё не сгенерированы."
        );
        return Ok(());
    }

    if is_neoforge_profile_id(version_id) {
        let install_profile = match read_install_profile_from_installer_jar(&jar) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[JentleMemes] NeoForge: {}", e);
                return Ok(());
            }
        };
        if let Some(mcp) = neoforge_mcp_client_version_from_install_profile(&install_profile) {
            let srg = neoforge_client_srg_jar_path(&data_dir, &mcp);
            if !srg.is_file() {
                eprintln!(
                    "[JentleMemes] NeoForge: нет «{}» — профиль остаётся ForgeWrapper до постпроцессора (первый успешный запуск).",
                    srg.display()
                );
                return Ok(());
            }
        }
    }

    let inherits = current.get("inheritsFrom").cloned().unwrap_or(Value::Null);

    let mut promoted = serde_json::Map::new();
    promoted.insert("id".to_string(), Value::String(version_id.to_string()));
    promoted.insert("inheritsFrom".to_string(), inherits);
    promoted.insert("mainClass".to_string(), inner["mainClass"].clone());
    if let Some(args) = inner.get("arguments") {
        promoted.insert("arguments".to_string(), args.clone());
    }
    if let Some(libs) = inner.get("libraries") {
        promoted.insert("libraries".to_string(), libs.clone());
    }
    if let Some(a) = inner.get("assets") {
        promoted.insert("assets".to_string(), a.clone());
    } else if let Some(a) = current.get("assets") {
        promoted.insert("assets".to_string(), a.clone());
    }
    if let Some(ai) = inner.get("assetIndex") {
        promoted.insert("assetIndex".to_string(), ai.clone());
    } else if let Some(ai) = current.get("assetIndex") {
        promoted.insert("assetIndex".to_string(), ai.clone());
    }
    if let Some(jv) = inner.get("javaVersion") {
        promoted.insert("javaVersion".to_string(), jv.clone());
    } else if let Some(jv) = current.get("javaVersion") {
        promoted.insert("javaVersion".to_string(), jv.clone());
    }
    if let Some(l) = inner.get("logging") {
        promoted.insert("logging".to_string(), l.clone());
    }

    save_profile(version_id, &Value::Object(promoted))?;
    eprintln!(
        "[JentleMemes] Forge: профиль «{}» переведён на {} (version.json из установщика).",
        version_id, mc
    );
    Ok(())
}

pub fn revert_broken_promote_if_needed(version_id: &str) -> Result<bool> {
    if !(is_forge_profile_id(version_id) || is_neoforge_profile_id(version_id)) {
        return Ok(false);
    }
    let data_dir = get_data_dir();
    let v_path = version_layout::profile_json_path(&data_dir, version_id);
    if !v_path.is_file() {
        return Ok(false);
    }
    let current: Value = serde_json::from_str(&fs::read_to_string(&v_path)?)?;
    let main = current
        .get("mainClass")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if main.to_lowercase().contains("forgewrapper") {
        return Ok(false);
    }
    let Some(jar) = forge_installer_jar_path(version_id, &current, &data_dir) else {
        return Ok(false);
    };
    let lib_dir = data_dir.join("libraries");
    if forge_processor_outputs_exist(&jar, &lib_dir) {
        return Ok(false);
    }

    let inner = read_version_json_from_installer_jar(&jar)?;
    let install_profile = read_install_profile_from_installer_jar(&jar)?;

    let inherits = current
        .get("inheritsFrom")
        .cloned()
        .unwrap_or(Value::Null);

    let wrapper_lib = serde_json::json!({
        "name": crate::core::loader_meta::jentlewrapper::JENTLE_WRAPPER_COORD,
        "downloads": {
            "artifact": {
                "sha1": crate::core::loader_meta::jentlewrapper::JENTLE_WRAPPER_SHA1,
                "size": crate::core::loader_meta::jentlewrapper::JENTLE_WRAPPER_SIZE
            }
        }
    });

    let mut libs: Vec<Value> = vec![wrapper_lib];
    if let Some(arr) = inner.get("libraries").and_then(|v| v.as_array()) {
        libs.extend(arr.iter().cloned());
    }

    let mut maven_files: Vec<Value> = Vec::new();
    if let Some(arr) = install_profile.get("libraries").and_then(|v| v.as_array()) {
        maven_files.extend(arr.iter().cloned());
    }

    let mut reverted = serde_json::Map::new();
    reverted.insert("id".to_string(), Value::String(version_id.to_string()));
    reverted.insert("inheritsFrom".to_string(), inherits);
    reverted.insert(
        "mainClass".to_string(),
        Value::String("io.github.zekerzhayard.forgewrapper.installer.Main".to_string()),
    );
    reverted.insert("libraries".to_string(), Value::Array(libs));
    if !maven_files.is_empty() {
        reverted.insert("mavenFiles".to_string(), Value::Array(maven_files));
    }
    if let Some(args) = inner.get("arguments") {
        reverted.insert("arguments".to_string(), args.clone());
    }
    if let Some(a) = current.get("assets").or_else(|| inner.get("assets")) {
        reverted.insert("assets".to_string(), a.clone());
    }
    if let Some(ai) = current.get("assetIndex").or_else(|| inner.get("assetIndex")) {
        reverted.insert("assetIndex".to_string(), ai.clone());
    }
    if let Some(jv) = current.get("javaVersion").or_else(|| inner.get("javaVersion")) {
        reverted.insert("javaVersion".to_string(), jv.clone());
    }

    save_profile(version_id, &Value::Object(reverted))?;

    materialize_embedded_vendor_library(
        &data_dir,
        &serde_json::from_value::<Library>(serde_json::json!({
            "name": crate::core::loader_meta::jentlewrapper::JENTLE_WRAPPER_COORD
        }))
        .unwrap(),
    );

    eprintln!(
        "[JentleMemes] Forge: профиль «{}» ОТКАТ на ForgeWrapper — процессорные артефакты отсутствуют. \
         Следующий запуск через ForgeWrapper сгенерирует их.",
        version_id
    );
    Ok(true)
}

pub fn ensure_neoforge_client_srg_or_error(
    data_dir: &Path,
    version_id: &str,
    main_class: &str,
) -> Result<()> {
    if !is_neoforge_profile_id(version_id) {
        return Ok(());
    }
    if main_class.to_lowercase().contains("forgewrapper") {
        return Ok(());
    }
    let v_path = version_layout::profile_json_path(data_dir, version_id);
    if !v_path.is_file() {
        return Ok(());
    }
    let current: Value = serde_json::from_str(&fs::read_to_string(&v_path)?)?;
    let Some(jar) = forge_installer_jar_path(version_id, &current, data_dir) else {
        return Ok(());
    };
    let install_profile = read_install_profile_from_installer_jar(&jar)?;
    let Some(mcp) = neoforge_mcp_client_version_from_install_profile(&install_profile) else {
        return Ok(());
    };
    let srg = neoforge_client_srg_jar_path(data_dir, &mcp);
    if srg.is_file() {
        return Ok(());
    }
    Err(Error::Custom(format!(
        "NeoForge: нет файла «{}» (постпроцессор установщика не выполнялся). В настройках сборки нажмите «Починить ядро», затем «Играть» — сначала отработает ForgeWrapper и создаст srg.",
        srg.display()
    )))
}

fn check_lib_rules(rules: &Option<Vec<serde_json::Value>>, current_os: &str) -> bool {
    let Some(rules) = rules else {
        return true;
    };
    if rules.is_empty() {
        return true;
    }

    let mut allowed = false;
    let mut disallowed = false;

    for rule in rules {
        let action = rule
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("allow");
        let os_match = if let Some(os) = rule.get("os").and_then(|v| v.as_object()) {
            os.get("name")
                .and_then(|v| v.as_str())
                .map_or(true, |n| n == current_os)
        } else {
            true
        };
        let is_os_specific = rule.get("os").is_some();

        if action == "allow" {
            if os_match {
                if is_os_specific {
                    return true;
                }
                allowed = true;
            }
        } else if action == "disallow" && os_match {
            disallowed = true;
        }
    }

    allowed && !disallowed
}

/// Метаданные завершённой установки. Служит sentinel'ом, чтобы `download_game_files`
/// не пробегался по ~100 библиотекам и ~3000 ассетам `stat()`-проверками, если
/// цепочка профиля на диске не менялась с последнего успеха.
///
/// Инвалидация: любой `install_version` / `install_loader` (когда JSON реально
/// переписан) ломает `chain_hash` через mtime — sentinel перестаёт совпадать.
#[derive(serde::Serialize, serde::Deserialize)]
struct InstallState {
    version: u32,
    chain_hash: String,
}

fn install_state_path(data_dir: &Path, version_id: &str) -> PathBuf {
    version_layout::profile_dir(data_dir, version_id).join("install_state.json")
}

fn profile_chain_paths(data_dir: &Path, version_id: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut cur = version_id.to_string();
    loop {
        let p = version_layout::profile_json_path(data_dir, &cur);
        if !p.is_file() {
            break;
        }
        let raw = match fs::read_to_string(&p) {
            Ok(s) => s,
            Err(_) => break,
        };
        out.push(p);
        let v: Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => break,
        };
        if let Some(parent) = v.get("inheritsFrom").and_then(|v| v.as_str()) {
            cur = parent.to_string();
        } else {
            break;
        }
    }
    out
}

fn install_state_is_fresh(data_dir: &Path, version_id: &str) -> bool {
    let sentinel = install_state_path(data_dir, version_id);
    let raw = match fs::read_to_string(&sentinel) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let state: InstallState = match serde_json::from_str(&raw) {
        Ok(s) => s,
        Err(_) => return false,
    };
    if state.version != 1 {
        return false;
    }
    let chain = profile_chain_paths(data_dir, version_id);
    if chain.is_empty() {
        return false;
    }
    let current = crate::core::fluxcore::launch_cache::compute_chain_hash(&chain);
    state.chain_hash == current
}

fn install_state_write(data_dir: &Path, version_id: &str) {
    let chain = profile_chain_paths(data_dir, version_id);
    if chain.is_empty() {
        return;
    }
    let chain_hash = crate::core::fluxcore::launch_cache::compute_chain_hash(&chain);
    let state = InstallState { version: 1, chain_hash };
    let dest = install_state_path(data_dir, version_id);
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(serialized) = serde_json::to_string(&state) {
        let _ = crate::core::utils::atomic_fs::write_atomic_string(&dest, &serialized);
    }
}

pub async fn download_game_files(
    app: AppHandle,
    version_id: &str,
    instance_id: Option<&str>,
) -> Result<String> {
    let data_dir = get_data_dir();
    let inst_id_opt = instance_id.map(String::from);

    // Warm-path fast exit: если прошлый успешный запуск записал sentinel с тем же
    // chain_hash, пропускаем все stat()-обходы (~3100 файлов: libraries + assets).
    // На холодном диске это экономит 100-300 мс перед «Играть».
    if install_state_is_fresh(&data_dir, version_id) {
        return Ok("Все файлы уже загружены (install_state sentinel).".into());
    }

    let mut all_libraries: Vec<Library> = Vec::new();
    let mut current_id = version_id.to_string();
    let mut main_v_info: Option<VersionInfo> = None;

    // Собираем все библиотеки по цепочке наследования
    loop {
        let v_path = version_layout::profile_json_path(&data_dir, &current_id);
        if !v_path.exists() {
            break;
        }

        let v_info: VersionInfo = serde_json::from_str(&fs::read_to_string(&v_path)?)?;
        all_libraries.extend(v_info.libraries.clone());
        // Forge/NeoForge: jopt-simple, часть постпроцессоров и т.д. только в mavenFiles — их тоже качаем здесь.
        all_libraries.extend(v_info.maven_files.clone());
        if main_v_info.is_none() {
            main_v_info = Some(v_info.clone());
        }
        if let Some(parent) = v_info.inherits_from {
            current_id = parent;
        } else {
            break;
        }
    }

    let mc_for_forge = version_layout::minecraft_version_from_profile_id(version_id)
        .filter(|s| version_layout::looks_like_minecraft_game_version(s));
    let all_libraries: Vec<Library> = all_libraries
        .into_iter()
        .map(|mut lib| {
            if let Some(ref mc) = mc_for_forge {
                if let Some(name) = normalize_legacy_forge_minecraftforge_coord(&lib.name, mc) {
                    lib.name = name;
                }
            }
            lib
        })
        .collect();

    let v_info = main_v_info.ok_or_else(|| Error::Custom("Version not found".into()))?;

    // --- Скачивание библиотек ---
    let mut tasks = Vec::new();

    #[cfg(target_os = "linux")]
    let os_name = "linux";
    #[cfg(target_os = "windows")]
    let os_name = "windows";
    #[cfg(target_os = "macos")]
    let os_name = "osx";

    for lib in &all_libraries {
        materialize_embedded_vendor_library(&data_dir, lib);
        if !check_lib_rules(&lib.rules, os_name) {
            continue;
        }

        // Determine whether to download the base (non-native) artifact.
        // Skip ONLY when `downloads` exists but has no `artifact` key
        // (explicitly native-only libs like lwjgl-platform).
        // When `downloads` is absent entirely (old format, e.g. 1.7.10),
        // we still need to download using Maven coordinates.
        let has_downloads = lib.downloads.is_some();
        let has_artifact = lib
            .downloads
            .as_ref()
            .and_then(|d| d.get("artifact"))
            .is_some();
        let skip_fake_platform_jar = lib.natives.is_some()
            && !has_artifact
            && (lib.name.contains("lwjgl-platform") || lib.name.contains("jinput-platform"));
        let should_download_base =
            (has_artifact || !has_downloads) && !skip_fake_platform_jar;

        if should_download_base {
            let mut url = String::new();
            let mut path = library_rel_path_under_libraries(lib);
            if lib.name.starts_with("net.minecraftforge:forge:")
                && lib.name.split(':').count() == 3
                && lib.downloads.is_none()
            {
                path = maven_to_path(&lib.name, Some("universal"));
            }
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
                    if let Some(rest) = lib
                        .name
                        .strip_prefix("io.github.zekerzhayard:ForgeWrapper:")
                    {
                        let ver = rest.split('@').next().unwrap_or(rest);
                        url = format!(
                            "https://github.com/ZekerZhayard/ForgeWrapper/releases/download/{ver}/ForgeWrapper-{ver}.jar"
                        );
                    } else {
                        let base_url = if let Some(base) = &lib.url {
                            if base.ends_with('/') {
                                base.clone()
                            } else {
                                format!("{}/", base)
                            }
                        } else if lib.name.starts_with("net.minecraftforge") {
                            "https://maven.minecraftforge.net/".to_string()
                        } else if lib.name.starts_with("net.neoforged") {
                            "https://maven.neoforged.net/releases/".to_string()
                        } else {
                            "https://libraries.minecraft.net/".to_string()
                        };
                        url = format!("{}{}", base_url, path);
                    }
                }
                tasks.push((url, dest, None));
            }
        }

        if let Some(natives) = &lib.natives {
            if let Some(classifier_key) = natives.get(os_name).and_then(|v| v.as_str()) {
                let from_classifiers = lib
                    .downloads
                    .as_ref()
                    .and_then(|d| d.get("classifiers"))
                    .and_then(|c| c.get(classifier_key))
                    .and_then(|v| v.as_object());

                if let Some(classifiers) = from_classifiers {
                    if let Some(url) = classifiers.get("url").and_then(|v| v.as_str()) {
                        let native_path =
                            if let Some(p) = classifiers.get("path").and_then(|v| v.as_str()) {
                                p.to_string()
                            } else {
                                maven_to_path(&lib.name, Some(classifier_key))
                            };
                        let dest = data_dir.join("libraries").join(&native_path);
                        if !dest.exists() {
                            tasks.push((url.to_string(), dest, None));
                        }
                    }
                } else {
                    let native_path = maven_to_path(&lib.name, Some(classifier_key));
                    let dest = data_dir.join("libraries").join(&native_path);
                    if !dest.exists() {
                        let central = format!("https://repo1.maven.org/maven2/{native_path}");
                        let mojang = format!("https://libraries.minecraft.net/{native_path}");
                        tasks.push((central, dest.clone(), Some((mojang, dest))));
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

        emit_download_progress(
            &app,
            DownloadProgress {
                task_name: format!("Скачивание библиотек (0/{})...", total_tasks),
                downloaded: 0,
                total: total_tasks,
                instance_id: inst_id_opt.clone(),
                ..Default::default()
            },
        );

        for (url, dest, fallback) in tasks {
            let dl_counter = downloaded.clone();
            let app_clone = app.clone();
            let sem = semaphore.clone();
            let inst_id = inst_id_opt.clone();

            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                if let Some(p) = dest.parent() {
                    let _ = tokio::fs::create_dir_all(p).await;
                }
                let result = match download_file(&url, None).await {
                    Ok(bytes) => Ok(bytes),
                    Err(e) => {
                        if let Some((fb_url, fb_dest)) = &fallback {
                            eprintln!("Fallback: {} → {}", url, fb_url);
                            if let Some(p) = fb_dest.parent() {
                                let _ = tokio::fs::create_dir_all(p).await;
                            }
                            match download_file(fb_url, None).await {
                                Ok(bytes) => {
                                    let _ = tokio::fs::write(fb_dest, &bytes).await;
                                    Ok(bytes)
                                }
                                Err(e2) => Err(e2),
                            }
                        } else if let Some(mirror) = libraries_minecraft_to_maven_central(&url) {
                            eprintln!("Fallback Maven Central: {} → {}", url, mirror);
                            download_file(&mirror, None).await.map_err(|_| e)
                        } else {
                            Err(e)
                        }
                    }
                };
                match result {
                    Ok(bytes) => {
                        if !dest.exists() {
                            if let Err(e) = tokio::fs::write(&dest, bytes).await {
                                eprintln!("Ошибка записи библиотеки {}: {}", dest.display(), e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Ошибка скачивания {}: {}", url, e);
                    }
                }

                let current = dl_counter.fetch_add(1, Ordering::SeqCst) + 1;
                emit_download_progress(
                    &app_clone,
                    DownloadProgress {
                        task_name: format!("Скачивание библиотек ({}/{})...", current, total_tasks),
                        downloaded: current,
                        total: total_tasks,
                        instance_id: inst_id.clone(),
                        ..Default::default()
                    },
                );
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
            let vp = version_layout::profile_json_path(&data_dir, &root_id);
            if !vp.exists() {
                break;
            }
            let raw: Value = serde_json::from_str(&fs::read_to_string(&vp)?)?;
            if let Some(parent) = raw.get("inheritsFrom").and_then(|v| v.as_str()) {
                root_id = parent.to_string();
            } else {
                break;
            }
        }
        let root_path = version_layout::profile_json_path(&data_dir, &root_id);
        if root_path.exists() {
            let root_raw: Value = serde_json::from_str(&fs::read_to_string(&root_path)?)?;
            if let Some(ai) = root_raw.get("assetIndex") {
                let index_id = ai.get("id").and_then(|v| v.as_str()).unwrap_or("legacy");
                let index_url = ai.get("url").and_then(|v| v.as_str());
                let index_path = assets_dir
                    .join("indexes")
                    .join(format!("{}.json", index_id));

                if let Some(url) = index_url {
                    if !index_path.exists() {
                        match download_file(url, None).await {
                            Ok(bytes) => {
                                fs::write(&index_path, &bytes)?;
                            }
                            Err(e) => {
                                eprintln!("Ошибка скачивания asset index: {}", e);
                            }
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
                                    let url = format!(
                                        "https://resources.download.minecraft.net/{}/{}",
                                        prefix, hash
                                    );
                                    asset_tasks.push((url, dest));
                                }
                            }
                        }

                        let total_assets = asset_tasks.len();
                        if total_assets > 0 {
                            let downloaded = Arc::new(AtomicUsize::new(0));
                            let sem = Arc::new(Semaphore::new(30));
                            let mut handles = Vec::new();

                            emit_download_progress(
                                &app,
                                DownloadProgress {
                                    task_name: format!(
                                        "Скачивание ассетов (0/{})...",
                                        total_assets
                                    ),
                                    downloaded: 0,
                                    total: total_assets,
                                    instance_id: inst_id_opt.clone(),
                                    ..Default::default()
                                },
                            );

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
                                        Ok(bytes) => {
                                            let _ = tokio::fs::write(&dest, bytes).await;
                                        }
                                        Err(e) => {
                                            eprintln!("Ошибка ассета {}: {}", url, e);
                                        }
                                    }
                                    let cur = dl.fetch_add(1, Ordering::SeqCst) + 1;
                                    if cur % 20 == 0 || cur == total {
                                        emit_download_progress(
                                            &app_c,
                                            DownloadProgress {
                                                task_name: format!(
                                                    "Скачивание ассетов ({}/{})...",
                                                    cur, total
                                                ),
                                                downloaded: cur,
                                                total,
                                                instance_id: inst_id.clone(),
                                                ..Default::default()
                                            },
                                        );
                                    }
                                }));
                            }
                            for h in handles {
                                let _ = h.await;
                            }
                        }
                    }
                }
            }
        }
    }

    // --- Определяем ID версии, от которой нужно взять ядро (client.jar) ---
    // Как в launch: id ванили — корень цепочки inheritsFrom, а не inheritsFrom листа (NeoForge).
    let mut cur_chain = version_id.to_string();
    let mut chain_rev: Vec<VersionInfo> = Vec::new();
    loop {
        let vp = version_layout::profile_json_path(&data_dir, &cur_chain);
        if !vp.is_file() {
            break;
        }
        let vi: VersionInfo = serde_json::from_str(&fs::read_to_string(&vp)?)?;
        let next = vi.inherits_from.clone();
        chain_rev.push(vi);
        match next {
            Some(p) => cur_chain = p,
            None => break,
        }
    }
    chain_rev.reverse();
    let jar_id: String = chain_rev
        .first()
        .and_then(|v| {
            v.id.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
        .unwrap_or_else(|| {
            v_info
                .inherits_from
                .as_deref()
                .unwrap_or(version_id)
                .to_string()
        });
    let jar_path = data_dir
        .join("versions")
        .join(&jar_id)
        .join(format!("{}.jar", jar_id));

    println!("[DEBUG] jar_id = {}, jar_path = {:?}", jar_id, jar_path);

    if !jar_path.exists() {
        // Загружаем JSON целевой версии как сырое значение (чтобы получить доступ к mainJar)
        let target_json_path = version_layout::profile_json_path(&data_dir, &jar_id);
        if !target_json_path.exists() {
            return Err(Error::Custom(format!(
                "Файл описания версии {} не найден",
                jar_id
            )));
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
                    let parent_path = version_layout::profile_json_path(&data_dir, parent);
                    if parent_path.exists() {
                        let parent_raw: Value =
                            serde_json::from_str(&fs::read_to_string(parent_path)?)?;
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

        emit_download_progress(
            &app,
            DownloadProgress {
                task_name: "Скачивание ядра Minecraft...".into(),
                downloaded: 0,
                total: 1,
                instance_id: inst_id_opt.clone(),
                ..Default::default()
            },
        );

        println!("[DEBUG] Начинаем скачивание с URL: {}", url);

        match download_file(&url, None).await {
            Ok(bytes) => {
                println!("[DEBUG] Скачано {} байт", bytes.len());
                fs::write(&jar_path, bytes)?;
                println!("[DEBUG] Файл сохранён: {:?}", jar_path);
                emit_download_progress(
                    &app,
                    DownloadProgress {
                        task_name: "Скачивание ядра Minecraft...".into(),
                        downloaded: 1,
                        total: 1,
                        instance_id: inst_id_opt.clone(),
                        ..Default::default()
                    },
                );
            }
            Err(e) => {
                eprintln!("[ERROR] Ошибка скачивания ядра: {}", e);
                return Err(Error::Custom(format!("Ошибка скачивания ядра: {}", e)));
            }
        }
    } else {
        println!("[DEBUG] Ядро уже существует, пропускаем скачивание.");
    }

    crate::core::loader_meta::modloader_alpha::apply_modloader_merge_to_jar(
        &data_dir, version_id, &jar_path,
    )
    .await?;

    // --- Скачивание mavenFiles (для Forge/NeoForge ForgeWrapper) ---
    let mut all_maven_files: Vec<Library> = Vec::new();
    {
        let mut cur_id = version_id.to_string();
        loop {
            let vp = version_layout::profile_json_path(&data_dir, &cur_id);
            if !vp.exists() {
                break;
            }
            let vi: VersionInfo = serde_json::from_str(&fs::read_to_string(&vp)?)?;
            all_maven_files.extend(vi.maven_files.clone());
            if let Some(parent) = vi.inherits_from {
                cur_id = parent;
            } else {
                break;
            }
        }
    }

    if !all_maven_files.is_empty() {
        let installer_dest =
            version_layout::profile_dir(&data_dir, version_id).join("installer.jar");
        let mut maven_tasks = Vec::new();
        let mut installer_url_for_copy: Option<(String, Option<String>)> = None;

        for mf in &all_maven_files {
            let path = library_rel_path_under_libraries(mf);
            let dest = data_dir.join("libraries").join(&path);

            let classifier = mf
                .name
                .split(':')
                .nth(3)
                .unwrap_or("")
                .split('@')
                .next()
                .unwrap_or("");
            let is_installer = classifier == "installer";

            let is_forge_coord = mf.name.starts_with("net.minecraftforge:forge:")
                || mf.name.starts_with("net.neoforged:neoforge:")
                || mf.name.starts_with("net.neoforged:forge:");
            let is_mc_client = mf.name.starts_with("net.minecraft:client:");
            let is_processor_output = (is_forge_coord && (classifier.is_empty() || classifier == "client"))
                || ((is_forge_coord || is_mc_client)
                    && (classifier == "slim" || classifier == "extra" || classifier == "srg"));
            if is_processor_output && !dest.exists() {
                continue;
            }

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
                        if base.ends_with('/') {
                            base.clone()
                        } else {
                            format!("{}/", base)
                        }
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
                installer_url_for_copy =
                    Some((String::new(), Some(dest.to_string_lossy().to_string())));
            }
        }

        let total_maven = maven_tasks.len();
        if total_maven > 0 {
            let downloaded = Arc::new(AtomicUsize::new(0));
            let mut handles = Vec::new();

            emit_download_progress(
                &app,
                DownloadProgress {
                    task_name: "Скачивание файлов Forge...".into(),
                    downloaded: 0,
                    total: total_maven,
                    instance_id: inst_id_opt.clone(),
                    ..Default::default()
                },
            );

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
                        emit_download_progress(
                            &app_clone,
                            DownloadProgress {
                                task_name: "Скачивание файлов Forge...".into(),
                                downloaded: current,
                                total,
                                instance_id: inst_id.clone(),
                                ..Default::default()
                            },
                        );
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
                        Ok(bytes) => {
                            fs::write(&installer_dest, bytes)?;
                        }
                        Err(e) => {
                            eprintln!("[ERROR] Ошибка скачивания installer.jar: {}", e);
                        }
                    }
                }
            }
        }
    }

    // --- Копирование / скачивание vanilla client.jar под Maven-путь Forge (после mavenFiles!) ---
    // Только для современного Forge (1.17+) / NeoForge с installer.jar (процессоры FART/binarypatcher).
    // Для Legacy Forge (LaunchWrapper, ≤1.16) installer не нужен.
    {
        let inst_jar = version_layout::profile_dir(&data_dir, version_id).join("installer.jar");
        let has_installer = inst_jar.is_file();
        if has_installer && (is_forge_profile_id(version_id) || is_neoforge_profile_id(version_id)) {
            let mut hints = collect_mc_hints(&jar_id, &[]);
            if let Some(m) = forge_minecraft_version_from_installer_jar(&inst_jar) {
                if !hints.contains(&m) {
                    hints.push(m);
                }
            }
            if let Some(m) = version_layout::minecraft_version_from_profile_id(version_id) {
                if !hints.contains(&m) {
                    hints.push(m);
                }
            }
            let downloaded = download_official_client_jar_to_libraries(&data_dir, &hints).await?;
            ensure_official_client_jar_for_forge_libraries(&data_dir, version_id, &downloaded, &hints)?;
        }
    }

    // --- Распаковка нативных библиотек ---
    {
        let natives_dir = version_layout::profile_dir(&data_dir, version_id).join("natives");
        fs::create_dir_all(&natives_dir)?;
        clear_native_artifact_files(&natives_dir);

        #[cfg(target_os = "linux")]
        let os_key = "linux";
        #[cfg(target_os = "windows")]
        let os_key = "windows";
        #[cfg(target_os = "macos")]
        let os_key = "osx";

        for lib in &all_libraries {
            let classifier_key = if let Some(natives) = &lib.natives {
                natives
                    .get(os_key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };

            let Some(classifier) = classifier_key else {
                continue;
            };

            let native_jar_path = data_dir
                .join("libraries")
                .join(maven_to_path(&lib.name, Some(&classifier)));
            if !native_jar_path.exists() {
                continue;
            }

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
                if name.starts_with("META-INF") || entry.is_dir() {
                    continue;
                }

                let out_path =
                    natives_dir.join(std::path::Path::new(&name).file_name().unwrap_or_default());
                if let Ok(mut out_file) = fs::File::create(&out_path) {
                    let _ = std::io::copy(&mut entry, &mut out_file);
                }
            }
        }
    }

    // FluxCore v3 / Phase 1.4: переносим идемпотентный promote ForgeWrapper → Bootstrap
    // в install-время. При повторном запуске игры это уже не нужно делать, warm-path
    // LaunchCache обходит промоушен полностью. При промахе кэша функция всё равно быстра
    // (profile JSON читается один раз и тут же возвращается, если mainClass уже не ForgeWrapper).
    if let Err(e) = promote_forge_wrapper_to_bootstrap(version_id) {
        eprintln!(
            "[JentleMemes] Forge/Bootstrap промо после download_game_files: {}",
            e
        );
    }

    // Помечаем установку завершённой. При следующем запуске того же профиля
    // (без install_version / install_loader между ними) весь download_game_files
    // короткозамкнётся на `install_state_is_fresh`. См. комментарий у `InstallState`.
    install_state_write(&data_dir, version_id);

    Ok("Файлы скачаны".into())
}

const FML_ENTRY_CORE_MOD: &str = "cpw/mods/fml/relauncher/CoreModManager.class";
const FML_ENTRY_SORT_FIX: &str = "cpw/mods/fml/relauncher/FmlSort172Fix.class";
const FML_CORE_MOD_STOCK_172_SHA256: &str =
    "3dc11ae9bc0e8f377078685c3b8f4adecf2f395c2884c228da321274e7b261b1";

fn forge172_fml_sort_patch_bytes() -> (&'static [u8], &'static [u8]) {
    (
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/forge172_fml_sort/CoreModManager.class"
        )),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/forge172_fml_sort/FmlSort172Fix.class"
        )),
    )
}

pub fn patch_forge_jar_fml_sort_if_needed(jar_path: &Path) -> Result<bool> {
    let fname = jar_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if !fname.ends_with("-universal.jar") {
        return Ok(false);
    }
    let file = match fs::File::open(jar_path) {
        Ok(f) => f,
        Err(_) => return Ok(false),
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return Ok(false),
    };

    let mut fix_present = false;
    let mut cm_bytes: Option<Vec<u8>> = None;
    for i in 0..archive.len() {
        let mut e = match archive.by_index(i) {
            Ok(x) => x,
            Err(_) => continue,
        };
        let name = e.name().to_string();
        if name == FML_ENTRY_SORT_FIX {
            fix_present = true;
        }
        if name == FML_ENTRY_CORE_MOD {
            let mut v = Vec::new();
            if e.read_to_end(&mut v).is_err() {
                continue;
            }
            cm_bytes = Some(v);
        }
    }
    if fix_present {
        return Ok(false);
    }
    let Some(cm) = cm_bytes else {
        return Ok(false);
    };
    let h = format!("{:x}", Sha256::digest(&cm));
    if h != FML_CORE_MOD_STOCK_172_SHA256 {
        return Ok(false);
    }

    let (patch_cm, patch_fix) = forge172_fml_sort_patch_bytes();
    let tmp = jar_path.with_extension("jar.jentle_fml_sort_tmp");
    let out_f = fs::File::create(&tmp)?;
    let mut zip_out = zip::ZipWriter::new(out_f);
    let mut buf = Vec::new();

    let file2 = fs::File::open(jar_path)?;
    let mut ar2 = zip::ZipArchive::new(file2)?;
    for i in 0..ar2.len() {
        let mut entry = ar2.by_index(i)?;
        let name = entry.name().to_string();
        if name == FML_ENTRY_CORE_MOD || name == FML_ENTRY_SORT_FIX {
            continue;
        }
        let method = entry.compression();
        buf.clear();
        entry.read_to_end(&mut buf)?;
        let opts = zip::write::FileOptions::default().compression_method(method);
        zip_out.start_file(&name, opts)?;
        zip_out.write_all(&buf)?;
    }
    zip_out.start_file(
        FML_ENTRY_CORE_MOD,
        zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )?;
    zip_out.write_all(patch_cm)?;
    zip_out.start_file(
        FML_ENTRY_SORT_FIX,
        zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )?;
    zip_out.write_all(patch_fix)?;
    if let Err(e) = zip_out.finish() {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    if let Err(e) = fs::rename(&tmp, jar_path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(true)
}

#[allow(dead_code)]
pub fn patch_legacy_forge_libraries_fml_sort(lib_dir: &Path) -> Result<usize> {
    let base = lib_dir.join("net/minecraftforge/forge");
    if !base.is_dir() {
        return Ok(0);
    }
    let mut n = 0usize;
    for ver in fs::read_dir(&base).into_iter().flatten().flatten() {
        let p = ver.path();
        if !p.is_dir() {
            continue;
        }
        for e in fs::read_dir(&p).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with("forge-") || !name.ends_with(".jar") {
                continue;
            }
            if name.ends_with("-client.jar") || name.ends_with("-installer.jar") {
                continue;
            }
            if patch_forge_jar_fml_sort_if_needed(&e.path())? {
                n += 1;
            }
        }
    }
    Ok(n)
}

#[cfg(test)]
mod install_state_tests {
    //! Проверяем, что sentinel `install_state.json` работает корректно:
    //! `is_fresh` → `false`, когда sentinel'а нет, `true` после записи с тем же
    //! chain_hash и снова `false`, когда profile JSON переписан с новым mtime.
    //! Это гарант, что warm-path short-circuit в `download_game_files` не замыкается
    //! на устаревшей установке (#1 в списке core weaknesses).

    use super::*;
    use crate::core::game::version_layout;

    fn fresh_data_dir() -> PathBuf {
        let tmp = std::env::temp_dir().join(format!(
            "jm_install_state_test_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&tmp).unwrap();
        tmp
    }

    fn write_minimal_profile(data_dir: &Path, id: &str, inherits: Option<&str>) {
        let dir = version_layout::profile_dir(data_dir, id);
        fs::create_dir_all(&dir).unwrap();
        let path = version_layout::profile_json_path(data_dir, id);
        let json = if let Some(parent) = inherits {
            format!(r#"{{"id":"{id}","inheritsFrom":"{parent}"}}"#)
        } else {
            format!(r#"{{"id":"{id}"}}"#)
        };
        fs::write(&path, json).unwrap();
    }

    #[test]
    fn install_state_is_absent_by_default() {
        let data = fresh_data_dir();
        write_minimal_profile(&data, "1.20.1", None);
        assert!(!install_state_is_fresh(&data, "1.20.1"));
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn install_state_after_write_matches() {
        let data = fresh_data_dir();
        write_minimal_profile(&data, "1.20.1", None);
        install_state_write(&data, "1.20.1");
        assert!(install_state_is_fresh(&data, "1.20.1"));
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn install_state_is_stale_when_profile_touched() {
        let data = fresh_data_dir();
        write_minimal_profile(&data, "1.20.1", None);
        install_state_write(&data, "1.20.1");
        assert!(install_state_is_fresh(&data, "1.20.1"));

        // Ждём, чтобы mtime точно поменялся (большинство FS резолвят до ms, но на
        // быстром SSD sub-ms запись может повторить тот же timestamp).
        std::thread::sleep(std::time::Duration::from_millis(10));
        let path = version_layout::profile_json_path(&data, "1.20.1");
        fs::write(&path, r#"{"id":"1.20.1","touched":true}"#).unwrap();

        assert!(!install_state_is_fresh(&data, "1.20.1"));
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn install_state_tracks_inherited_chain() {
        let data = fresh_data_dir();
        write_minimal_profile(&data, "1.20.1", None);
        write_minimal_profile(&data, "fabric-loader-1.20.1", Some("1.20.1"));
        install_state_write(&data, "fabric-loader-1.20.1");
        assert!(install_state_is_fresh(&data, "fabric-loader-1.20.1"));

        // Переписываем ванильный (родительский) профиль — sentinel должен инвалидироваться.
        std::thread::sleep(std::time::Duration::from_millis(10));
        let vp = version_layout::profile_json_path(&data, "1.20.1");
        fs::write(&vp, r#"{"id":"1.20.1","bumped":true}"#).unwrap();

        assert!(!install_state_is_fresh(&data, "fabric-loader-1.20.1"));
        let _ = fs::remove_dir_all(&data);
    }
}

#[cfg(test)]
mod save_profile_tests {
    //! `save_profile`-like логика: при тех же байтах JSON не переписывать файл,
    //! иначе `LaunchCache.chain_hash` инвалидируется на каждом запуске (минус
    //! #9/10 в списке core weaknesses). Здесь тестируем чистую логику
    //! «equal text → skip write», `save_profile` в проде использует get_data_dir()
    //! и плохо мокается, поэтому проверяем вспомогательную версию.
    use super::*;

    fn save_profile_at(dest: &Path, json: &Value) -> Result<bool> {
        let new_text = serde_json::to_string_pretty(json)?;
        if let Ok(existing) = fs::read_to_string(dest) {
            if existing == new_text {
                return Ok(false);
            }
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = dest.with_extension("json.tmp");
        fs::write(&tmp, &new_text)?;
        fs::rename(&tmp, dest).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            Error::Custom(format!("rename: {e}"))
        })?;
        Ok(true)
    }

    #[test]
    fn save_profile_identical_content_is_noop() {
        let tmp = std::env::temp_dir().join(format!(
            "jm_save_profile_noop_test_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&tmp).unwrap();
        let dest = tmp.join("profile.json");
        let json = serde_json::json!({"id": "x", "version": 1});

        // Первая запись — файл создан.
        assert!(save_profile_at(&dest, &json).unwrap());
        let mtime1 = fs::metadata(&dest).unwrap().modified().unwrap();

        std::thread::sleep(std::time::Duration::from_millis(20));

        // Та же запись — no-op.
        assert!(!save_profile_at(&dest, &json).unwrap());
        let mtime2 = fs::metadata(&dest).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2, "no-op переписал файл");

        // Изменённый JSON — файл действительно перезаписан.
        let json2 = serde_json::json!({"id": "x", "version": 2});
        assert!(save_profile_at(&dest, &json2).unwrap());
        let mtime3 = fs::metadata(&dest).unwrap().modified().unwrap();
        assert_ne!(mtime2, mtime3, "перезапись с новым JSON не тронула mtime");

        let _ = fs::remove_dir_all(&tmp);
    }
}
