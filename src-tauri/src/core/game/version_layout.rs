//! Размещение JSON-профилей: vanilla и легаси `forge-…` — `versions/<id>/<id>.json`;
//! модлоадеры — `versions/<loader>/<gameVer>/<loaderVer>/<loaderVer>.json`;
//! логический `id` в JSON — `<loader>/<gameVer>/<loaderVer>`.
//! Старые установки: `versions/<loader>/<loaderVer-gameVer>/<slug>.json` — подхватываются при чтении.

use std::path::{Path, PathBuf};

const LOADERS_SUBDIR: &[&str] = &[
    "forge",
    "neoforge",
    "fabric",
    "quilt",
    "liteloader",
    "modloader",
];

/// Сегмент пути на диске (редкие символы из версий MC).
pub fn sanitize_path_segment(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '<' | '>' | '|' | '?' | '*' => '-',
            _ => c,
        })
        .collect()
}

/// Новый id: `forge/1.21.6/56.0.9`, `fabric/1.21.1/0.16.9`, …
pub fn modded_profile_id(
    loader: &str,
    normalized_loader_version: &str,
    game_version: &str,
) -> String {
    format!(
        "{}/{}/{}",
        loader.trim(),
        game_version.trim(),
        normalized_loader_version.trim()
    )
}

/// `loader/game/slug` где slug — один сегмент «lv-gv» (легаси двухсегментный id).
fn two_segment_loader_slug(version_id: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = version_id.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let loader = parts[0];
    let slug = parts[1];
    if LOADERS_SUBDIR.contains(&loader) && !slug.is_empty() {
        Some((loader, slug))
    } else {
        None
    }
}

/// Три сегмента: `loader/gameVer/loaderVer`.
fn three_segment_modded(version_id: &str) -> Option<(&str, &str, &str)> {
    let parts: Vec<&str> = version_id.split('/').collect();
    if parts.len() != 3 {
        return None;
    }
    let loader = parts[0];
    if LOADERS_SUBDIR.contains(&loader) {
        Some((parts[0], parts[1], parts[2]))
    } else {
        None
    }
}

pub fn looks_like_minecraft_game_version(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("1.") && t.contains('.') && t.chars().all(|c| c.is_ascii_digit() || c == '.')
}

/// Из легаси slug `lv-gv` → `(game_ver, loader_ver)`.
fn parse_legacy_lv_gv_slug(loader: &str, slug: &str) -> Option<(String, String)> {
    if loader == "neoforge" {
        let (neo, mc) = slug.rsplit_once('-')?;
        let mc = mc.trim();
        if mc.is_empty() {
            return None;
        }
        return Some((mc.to_string(), neo.trim().to_string()));
    }
    if loader == "forge" {
        if let Some((a, b)) = slug.rsplit_once('-') {
            if looks_like_minecraft_game_version(b) {
                return Some((b.to_string(), a.to_string()));
            }
        }
        if let Some((a, b)) = slug.split_once('-') {
            if looks_like_minecraft_game_version(a) {
                return Some((a.to_string(), b.to_string()));
            }
        }
        return None;
    }
    // fabric, quilt, liteloader, modloader: последний сегмент — MC `1.x`
    if let Some((lv, gv)) = slug.rsplit_once('-') {
        let gv = gv.trim();
        if looks_like_minecraft_game_version(gv) {
            return Some((gv.to_string(), lv.trim().to_string()));
        }
    }
    None
}

/// Каталог для **новой** раскладки (запись файлов).
fn primary_profile_dir(data_dir: &Path, version_id: &str) -> PathBuf {
    if let Some((l, g, v)) = three_segment_modded(version_id) {
        return data_dir
            .join("versions")
            .join(sanitize_path_segment(l))
            .join(sanitize_path_segment(g))
            .join(sanitize_path_segment(v));
    }
    if let Some((loader, slug)) = two_segment_loader_slug(version_id) {
        if let Some((gv, lv)) = parse_legacy_lv_gv_slug(loader, slug) {
            return data_dir
                .join("versions")
                .join(sanitize_path_segment(loader))
                .join(sanitize_path_segment(&gv))
                .join(sanitize_path_segment(&lv));
        }
        return data_dir
            .join("versions")
            .join(sanitize_path_segment(loader))
            .join(sanitize_path_segment(slug));
    }
    data_dir
        .join("versions")
        .join(sanitize_path_segment(version_id))
}

fn primary_json_stem(version_id: &str) -> String {
    if let Some((_, _, lv)) = three_segment_modded(version_id) {
        return lv.to_string();
    }
    if let Some((loader, slug)) = two_segment_loader_slug(version_id) {
        if let Some((_, lv)) = parse_legacy_lv_gv_slug(loader, slug) {
            return lv;
        }
        return slug.to_string();
    }
    version_id.to_string()
}

/// Легаси: `versions/<loader>/<slug>/<slug>.json` (одна папка slug).
fn legacy_slug_profile_json_path(data_dir: &Path, version_id: &str) -> Option<PathBuf> {
    let (loader, slug) = two_segment_loader_slug(version_id)?;
    if parse_legacy_lv_gv_slug(loader, slug).is_none() {
        return None;
    }
    let dir = data_dir
        .join("versions")
        .join(sanitize_path_segment(loader))
        .join(sanitize_path_segment(slug));
    Some(dir.join(format!("{}.json", slug)))
}

pub fn profile_json_path(data_dir: &Path, version_id: &str) -> PathBuf {
    let new_dir = primary_profile_dir(data_dir, version_id);
    let stem = primary_json_stem(version_id);
    let new_path = new_dir.join(format!("{stem}.json"));
    if new_path.is_file() {
        return new_path;
    }
    if let Some(old) = legacy_slug_profile_json_path(data_dir, version_id) {
        if old.is_file() {
            return old;
        }
    }
    new_path
}

/// Каталог профиля (json, installer.jar, natives): совпадает с родителем актуального json.
pub fn profile_dir(data_dir: &Path, version_id: &str) -> PathBuf {
    profile_json_path(data_dir, version_id)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| primary_profile_dir(data_dir, version_id))
}

pub fn is_neoforge_profile_id(version_id: &str) -> bool {
    version_id.starts_with("neoforge/") || version_id.starts_with("neoforge-")
}

pub fn is_forge_profile_id(version_id: &str) -> bool {
    version_id.starts_with("forge/") || version_id.starts_with("forge-")
}

/// Версия Minecraft из id профиля.
pub fn minecraft_version_from_profile_id(version_id: &str) -> Option<String> {
    if let Some((_, g, _)) = three_segment_modded(version_id) {
        return Some(g.to_string());
    }
    if let Some((loader, slug)) = two_segment_loader_slug(version_id) {
        if let Some((gv, _)) = parse_legacy_lv_gv_slug(loader, slug) {
            return Some(gv);
        }
    }
    if let Some(r) = version_id
        .strip_prefix("neoforge/")
        .or_else(|| version_id.strip_prefix("neoforge-"))
    {
        let (_, last) = r.rsplit_once('-')?;
        let last = last.trim();
        return (!last.is_empty()).then(|| last.to_string());
    }
    if let Some(r) = version_id
        .strip_prefix("forge/")
        .or_else(|| version_id.strip_prefix("forge-"))
    {
        let (_, last) = r.rsplit_once('-')?;
        let last = last.trim();
        if !last.is_empty() && looks_like_minecraft_game_version(last) {
            return Some(last.to_string());
        }
        let (first, _) = r.split_once('-')?;
        let first = first.trim();
        return looks_like_minecraft_game_version(first).then(|| first.to_string());
    }
    None
}
