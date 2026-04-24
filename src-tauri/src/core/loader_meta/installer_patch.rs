//! Патч Forge/NeoForge для merge с vanilla: официальный `*-installer.jar` с Maven (без Prism Meta).
//! Для 1.13+ classpath берётся из `version.json` внутри установщика + ForgeWrapper; `mavenFiles` —
//! установщик и библиотеки из `install_profile.json` (постпроцессоры). Для Launchwrapper (≤1.12.x)
//! в установщике часто только `install_profile.versionInfo` — патч строится напрямую из него.

use std::io::{Cursor, Read};

use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use zip::ZipArchive;

use crate::core::utils::download::download_file;
use crate::core::utils::modlauncher::{
    pick_published_modlauncher_version, MODLAUNCHER_METADATA_URL,
};
use crate::error::{Error, Result};

use super::forge::split_forge_maven_version;
use super::version_sort::{is_release_style_mc_version, mc_version_precedes};
use super::jentlewrapper;
use super::neoforge::{neoforge_artifact_matches_game, neoforge_artifact_to_minecraft};

fn forge_maven_coordinate(game_version: &str, forge_version: &str) -> String {
    let gv = game_version.trim();
    let fv = forge_version.trim();
    // Только «чистая» версия Forge (`47.2.0`) склеивается с MC; строка с `-` считается полным Maven-id (`1.20.1-47.2.0`, легаси `…-1.7.10`).
    if !fv.contains('-') {
        format!("{gv}-{fv}")
    } else {
        fv.to_string()
    }
}

fn sha1_hex(bytes: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn read_zip_entry(archive: &mut ZipArchive<Cursor<&[u8]>>, path: &str) -> Result<String> {
    let mut f = archive
        .by_name(path)
        .map_err(|e| Error::Custom(format!("В установщике нет «{path}»: {e}")))?;
    let mut s = String::new();
    f.read_to_string(&mut s)?;
    Ok(s)
}

fn parse_installer_jar(bytes: &[u8]) -> Result<(Value, Value)> {
    let c = Cursor::new(bytes);
    let mut zip = ZipArchive::new(c).map_err(|e| Error::Custom(format!("JAR установщика: {e}")))?;

    let install_profile_s = read_zip_entry(&mut zip, "install_profile.json")?;
    let install_profile: Value = serde_json::from_str(&install_profile_s)
        .map_err(|e| Error::Custom(format!("install_profile.json: {e}")))?;

    let version_inner = if zip.by_name("version.json").is_ok() {
        let s = read_zip_entry(&mut zip, "version.json")?;
        serde_json::from_str(&s).map_err(|e| Error::Custom(format!("version.json: {e}")))?
    } else {
        install_profile.get("versionInfo").cloned().ok_or_else(|| {
            Error::Custom("В установщике нет version.json и install_profile.versionInfo".into())
        })?
    };

    Ok((version_inner, install_profile))
}

fn forgewrapper_library() -> Value {
    json!({
        "name": jentlewrapper::JENTLE_WRAPPER_COORD,
        "downloads": {
            "artifact": {
                "sha1": jentlewrapper::JENTLE_WRAPPER_SHA1,
                "size": jentlewrapper::JENTLE_WRAPPER_SIZE
            }
        }
    })
}

fn installer_maven_artifact(name: &str, url: &str, bytes: &[u8]) -> Value {
    json!({
        "name": name,
        "downloads": {
            "artifact": {
                "sha1": sha1_hex(bytes),
                "size": bytes.len() as u64,
                "url": url
            }
        }
    })
}

fn is_launchwrapper_profile(version_inner: &Value) -> bool {
    version_inner
        .get("mainClass")
        .and_then(|v| v.as_str())
        .map(|m| m.to_lowercase().contains("launchwrapper"))
        .unwrap_or(false)
}

fn legacy_patch_from_version(version_inner: &Value) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("formatVersion".to_string(), json!(1));
    if let Some(mc) = version_inner.get("mainClass") {
        m.insert("mainClass".to_string(), mc.clone());
    }
    if let Some(a) = version_inner.get("minecraftArguments") {
        m.insert("minecraftArguments".to_string(), a.clone());
    }
    if let Some(libs) = version_inner.get("libraries") {
        m.insert("libraries".to_string(), libs.clone());
    }
    Value::Object(m)
}

fn attach_installer_maven_to_legacy_patch(
    patch: &mut Value,
    installer_name: &str,
    url: &str,
    bytes: &[u8],
) {
    let Some(obj) = patch.as_object_mut() else {
        return;
    };
    obj.insert(
        "mavenFiles".to_string(),
        json!([installer_maven_artifact(installer_name, url, bytes)]),
    );
}

fn extract_legacy_forge_universal_from_installer(
    installer_bytes: &[u8],
    version_inner: &Value,
) -> Result<()> {
    let data_dir = crate::config::get_data_dir();
    let lib_dir = data_dir.join("libraries");

    let libs = match version_inner.get("libraries").and_then(|v| v.as_array()) {
        Some(l) => l,
        None => return Ok(()),
    };

    let forge_name = libs
        .iter()
        .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
        .find(|n| n.starts_with("net.minecraftforge:forge:"));
    let Some(forge_name) = forge_name else {
        return Ok(());
    };

    let rel_path = super::super::utils::maven::maven_to_path(forge_name, Some("universal"));
    let dest = lib_dir.join(&rel_path);
    if dest.is_file() {
        return Ok(());
    }

    let parts: Vec<&str> = forge_name.split(':').collect();
    if parts.len() < 3 {
        return Ok(());
    }
    let artifact = parts[1];
    let version = parts[2].split('@').next().unwrap_or(parts[2]);

    let c = Cursor::new(installer_bytes);
    let mut zip =
        ZipArchive::new(c).map_err(|e| Error::Custom(format!("installer zip: {e}")))?;

    let candidates = [
        format!("forge-{version}-universal.jar"),
        format!(
            "maven/{}/{artifact}/{version}/{artifact}-{version}-universal.jar",
            parts[0].replace('.', "/")
        ),
    ];

    for cand in &candidates {
        if let Ok(mut entry) = zip.by_name(cand) {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::write(&dest, &bytes)?;
            eprintln!(
                "[JentleMemes] Forge legacy: {} → {}",
                cand,
                dest.display()
            );
            return Ok(());
        }
    }

    eprintln!(
        "[JentleMemes] Forge legacy: universal jar not found inside installer for {forge_name}"
    );
    Ok(())
}

/// Версия `net.neoforged.fancymodloader:loader` из `version.json` установщика (подсказка для `cpw.mods:modlauncher`, не всегда 1:1).
fn fancymodloader_loader_version(version_inner: &Value) -> Option<String> {
    let arr = version_inner.get("libraries")?.as_array()?;
    let prefix = "net.neoforged.fancymodloader:loader:";
    for lib in arr {
        let name = lib.get("name")?.as_str()?;
        let rest = name.strip_prefix(prefix)?;
        let ver = rest.split('@').next()?.trim();
        if !ver.is_empty() {
            return Some(ver.to_string());
        }
    }
    None
}

/// ForgeWrapper (`Main`) создаёт URLClassLoader с `cpw.mods.modlauncher.Launcher`; в NeoForge 26+ этого артефакта нет в `version.json`.
fn neoforge_modlauncher_library(ml_ver: &str) -> Value {
    let v = ml_ver.trim();
    json!({
        "name": format!("cpw.mods:modlauncher:{v}"),
        "downloads": {
            "artifact": {
                "url": format!(
                    "https://maven.neoforged.net/releases/cpw/mods/modlauncher/{v}/modlauncher-{v}.jar"
                )
            }
        }
    })
}

fn explicit_modlauncher_in_version_inner(version_inner: &Value) -> bool {
    let Some(arr) = version_inner.get("libraries").and_then(|v| v.as_array()) else {
        return false;
    };
    arr.iter()
        .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
        .any(|n| n.starts_with("cpw.mods:modlauncher:"))
}

fn inject_neoforge_modlauncher_for_forgewrapper(
    patch: &mut Value,
    version_inner: &Value,
    modlauncher_metadata_xml: Option<&str>,
) {
    if explicit_modlauncher_in_version_inner(version_inner) {
        return;
    }
    let Some(hint) = fancymodloader_loader_version(version_inner) else {
        return;
    };
    let ml_ver = modlauncher_metadata_xml
        .map(|xml| pick_published_modlauncher_version(&hint, xml))
        .unwrap_or(hint);
    let Some(libs) = patch.get_mut("libraries").and_then(|v| v.as_array_mut()) else {
        return;
    };
    if libs
        .iter()
        .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
        .any(|n| n.starts_with("cpw.mods:modlauncher:"))
    {
        return;
    }
    let entry = neoforge_modlauncher_library(&ml_ver);
    let insert_at = if libs
        .first()
        .and_then(|l| l.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.contains("wrapper"))
        .unwrap_or(false)
    {
        1
    } else {
        0
    };
    libs.insert(insert_at.min(libs.len()), entry);
}

fn modern_forge_like_patch(
    version_inner: &Value,
    install_profile: &Value,
    installer_name: &str,
    installer_url: &str,
    installer_bytes: &[u8],
) -> Result<Value> {
    let mut libs: Vec<Value> = vec![forgewrapper_library()];
    if let Some(arr) = version_inner.get("libraries").and_then(|v| v.as_array()) {
        libs.extend(arr.iter().cloned());
    }

    let mut maven_files: Vec<Value> = vec![installer_maven_artifact(
        installer_name,
        installer_url,
        installer_bytes,
    )];
    if let Some(arr) = install_profile.get("libraries").and_then(|v| v.as_array()) {
        maven_files.extend(arr.iter().cloned());
    }

    Ok(json!({
        "formatVersion": 1,
        "mainClass": "io.github.zekerzhayard.forgewrapper.installer.Main",
        "libraries": libs,
        "mavenFiles": maven_files
    }))
}

async fn fetch_installer_bytes(url: &str, ctx: &str) -> Result<Vec<u8>> {
    let res = crate::core::api::http_client().get(url).send().await?;
    if !res.status().is_success() {
        return Err(Error::Custom(format!(
            "{ctx}: HTTP {} — {}",
            res.status(),
            res.url()
        )));
    }
    Ok(res.bytes().await?.to_vec())
}

fn forge_installer_jar_url(coord: &str) -> String {
    format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{coord}/forge-{coord}-installer.jar"
    )
}

fn mc_versions_loosely_match(artifact_mc: &str, selected_mc: &str) -> bool {
    if artifact_mc == selected_mc {
        return true;
    }
    if selected_mc.starts_with(&format!("{artifact_mc}.")) {
        return true;
    }
    if artifact_mc.starts_with(&format!("{selected_mc}.")) {
        return true;
    }
    false
}

fn forge_coord_matches_game_and_loader(full: &str, gv: &str, fv: &str) -> bool {
    if full.starts_with(&format!("{gv}-")) {
        let rest = full[gv.len().saturating_add(1)..].trim();
        if rest == fv {
            return true;
        }
        if rest == format!("{fv}-{gv}") {
            return true;
        }
        if let Some(mid) = rest.strip_suffix(&format!("-{gv}")) {
            if mid == fv {
                return true;
            }
        }
    }
    if let Some((mc, forge_v)) = split_forge_maven_version(full) {
        return forge_v == fv && mc_versions_loosely_match(&mc, gv);
    }
    false
}

async fn fetch_forge_installer_jar_resolved(
    gv: &str,
    forge_version: &str,
    primary_coord: &str,
) -> Result<(Vec<u8>, String, String)> {
    let fv = forge_version.trim();
    let meta = super::forge::maven_full_versions().await.unwrap_or_default();
    let meta_has = |c: &str| meta.iter().any(|x| x == c);

    let mut coords: Vec<String> = Vec::new();
    let push = |c: &str, v: &mut Vec<String>| {
        let t = c.trim();
        if !t.is_empty() && !v.iter().any(|x| x == t) {
            v.push(t.to_string());
        }
    };

    push(primary_coord, &mut coords);

    let triple = format!("{primary_coord}-{gv}");
    if triple != primary_coord && meta_has(&triple) {
        push(&triple, &mut coords);
    }

    let mut meta_extra: Vec<String> = Vec::new();
    for full in &meta {
        if coords.iter().any(|c| c == full) {
            continue;
        }
        if forge_coord_matches_game_and_loader(full, gv, fv) {
            meta_extra.push(full.clone());
        }
    }
    meta_extra.sort_by_key(|s| {
        if let Some((mc, _)) = split_forge_maven_version(s) {
            if mc == gv {
                return 0usize;
            }
        }
        s.len()
    });
    for e in meta_extra {
        push(&e, &mut coords);
    }

    let mut last_err = String::new();
    for c in &coords {
        let url = forge_installer_jar_url(c);
        match fetch_installer_bytes(&url, "Forge installer").await {
            Ok(b) => return Ok((b, c.clone(), url)),
            Err(e) => last_err = e.to_string(),
        }
    }

    Err(Error::Custom(format!(
        "Установщик Forge не найден (координаты: {}). \
Maven вернул ошибку для всех вариантов. Последняя: {last_err}. \
Очень старые инсталлеры могли быть удалены с maven.minecraftforge.net — выберите другую версию Forge.",
        coords.join(", ")
    )))
}

/// Патч профиля Forge: `forge-{mc}-{fv}-installer.jar` с Maven Forge.
pub async fn forge_profile_patch_from_installer(
    game_version: &str,
    forge_version: &str,
) -> Result<Value> {
    let gv = game_version.trim();
    if gv.is_empty() {
        return Err(Error::Custom(
            "Не указана версия Minecraft для установки Forge.".into(),
        ));
    }
    if is_release_style_mc_version(gv) && mc_version_precedes(gv, "1.5.2") {
        return Err(Error::Custom(format!(
            "Forge для Minecraft {gv} недоступен: официальные installer.jar для релизов ниже 1.5.2 сняты с maven.minecraftforge.net. Укажите Minecraft 1.5.2 или новее."
        )));
    }
    let coord = forge_maven_coordinate(gv, forge_version);
    let dash_count = coord.chars().filter(|&c| c == '-').count();
    if dash_count == 1 {
        if let Some((mc, _)) = split_forge_maven_version(&coord) {
            if mc != gv {
                return Err(Error::Custom(format!(
                    "Координата Forge «{coord}» относится к Minecraft {mc}, а не к {gv}."
                )));
            }
        } else {
            return Err(Error::Custom(format!(
                "Некорректная пара Minecraft/Forge для артефакта «{coord}»."
            )));
        }
    }

    let (bytes, resolved_coord, url) =
        fetch_forge_installer_jar_resolved(gv, forge_version, &coord).await?;
    let (version_inner, install_profile) = parse_installer_jar(&bytes)?;

    if is_launchwrapper_profile(&version_inner) {
        let _ = extract_legacy_forge_universal_from_installer(&bytes, &version_inner);
        let installer_name = format!("net.minecraftforge:forge:{resolved_coord}:installer");
        let mut patch = legacy_patch_from_version(&version_inner);
        attach_installer_maven_to_legacy_patch(&mut patch, &installer_name, &url, &bytes);
        return Ok(patch);
    }

    let installer_name = format!("net.minecraftforge:forge:{resolved_coord}:installer");
    modern_forge_like_patch(
        &version_inner,
        &install_profile,
        &installer_name,
        &url,
        &bytes,
    )
}

/// Патч профиля NeoForge: `neoforge-{ver}-installer.jar` с Maven NeoForged.
pub async fn neoforge_profile_patch_from_installer(
    game_version: &str,
    neo_version: &str,
) -> Result<Value> {
    let gv = game_version.trim();
    if gv.is_empty() {
        return Err(Error::Custom(
            "Не указана версия Minecraft для установки NeoForge.".into(),
        ));
    }
    let nv = neo_version.trim();
    if nv.is_empty() {
        return Err(Error::Custom("Пустая версия NeoForge.".into()));
    }
    if let Some(derived_mc) = neoforge_artifact_to_minecraft(nv) {
        if !neoforge_artifact_matches_game(gv, nv) {
            return Err(Error::Custom(format!(
                "NeoForge {nv} соответствует Minecraft {derived_mc}, а в сборке указано {gv}."
            )));
        }
    }

    let url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{nv}/neoforge-{nv}-installer.jar"
    );
    let bytes = fetch_installer_bytes(&url, "NeoForge installer").await?;
    let (version_inner, install_profile) = parse_installer_jar(&bytes)?;

    if is_launchwrapper_profile(&version_inner) {
        let installer_name = format!("net.neoforged:neoforge:{nv}:installer");
        let mut patch = legacy_patch_from_version(&version_inner);
        attach_installer_maven_to_legacy_patch(&mut patch, &installer_name, &url, &bytes);
        return Ok(patch);
    }

    let installer_name = format!("net.neoforged:neoforge:{nv}:installer");
    let mut patch = modern_forge_like_patch(
        &version_inner,
        &install_profile,
        &installer_name,
        &url,
        &bytes,
    )?;
    let modlauncher_meta = download_file(MODLAUNCHER_METADATA_URL, None)
        .await
        .ok()
        .map(|b| String::from_utf8_lossy(&b).to_string());
    inject_neoforge_modlauncher_for_forgewrapper(
        &mut patch,
        &version_inner,
        modlauncher_meta.as_deref(),
    );
    Ok(patch)
}
