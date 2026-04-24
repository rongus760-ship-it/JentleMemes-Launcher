use crate::core::game::version_layout::{
    is_forge_profile_id, is_neoforge_profile_id, minecraft_version_from_profile_id, profile_dir,
};
use crate::core::game::version_layout::looks_like_minecraft_game_version;
use crate::core::types::VersionInfo;
use crate::core::utils::maven::{
    library_rel_path_effective_for_os, library_rel_path_for_classpath, maven_to_path,
    resolve_net_minecraft_client_dir_name,
};
use crate::error::{Error, Result};
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

use crate::core::fluxcore::launch_cache::{self, LaunchCache};
use crate::core::fluxcore::v3::bus::{FluxTraceScope, PulseSpan};
use crate::core::fluxcore::v3::types::HelixStageId;

/// Игровые аргументы из одного профиля (legacy `minecraftArguments` или `arguments.game` со строками).
fn extract_profile_game_args(v: &VersionInfo) -> Vec<String> {
    if let Some(legacy) = &v.minecraft_arguments {
        let t = legacy.trim();
        if !t.is_empty() {
            return t.split_whitespace().map(String::from).collect();
        }
    }
    if let Some(arr) = v
        .arguments
        .as_ref()
        .and_then(|a| a.get("game"))
        .and_then(|a| a.as_array())
    {
        let strings: Vec<String> = arr
            .iter()
            .filter_map(|x| x.as_str().map(String::from))
            .collect();
        if !strings.is_empty() {
            return strings;
        }
    }
    Vec::new()
}

/// Без `--accessToken` и `--version` joptsimple в Forge/NeoForge падает (`MissingRequiredOptionsException`).
fn game_args_have_access_token_and_version(args: &[String]) -> bool {
    let mut has_token = false;
    let mut has_ver = false;
    for a in args {
        if a == "--accessToken" || a.starts_with("--accessToken=") {
            has_token = true;
        }
        if a == "--version" || a.starts_with("--version=") {
            has_ver = true;
        }
    }
    has_token && has_ver
}

fn game_arg_option_name(token: &str) -> Option<&str> {
    let t = token.strip_prefix("--")?;
    Some(t.split('=').next().unwrap_or(t))
}

/// Имена long-опций (`--accessToken`, `version` из `--version`) в списке argv.
fn game_args_declared_keys(args: &[String]) -> std::collections::HashSet<String> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut i = 0usize;
    while i < args.len() {
        let a = &args[i];
        if let Some(name) = game_arg_option_name(a) {
            seen.insert(name.to_string());
            if !a.contains('=') && i + 1 < args.len() && !args[i + 1].starts_with('-') {
                i += 2;
                continue;
            }
        }
        i += 1;
    }
    seen
}

fn existing_tweak_class_values(args: &[String]) -> HashSet<String> {
    let mut s = HashSet::new();
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == "--tweakClass" && i + 1 < args.len() {
            s.insert(args[i + 1].clone());
            i += 2;
            continue;
        }
        if let Some(r) = args[i].strip_prefix("--tweakClass=") {
            if !r.is_empty() {
                s.insert(r.to_string());
            }
        }
        i += 1;
    }
    s
}

fn dedupe_tweak_class_options(args: &mut Vec<String>) {
    let mut seen = HashSet::<String>::new();
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == "--tweakClass" && i + 1 < args.len() {
            let val = args[i + 1].clone();
            if !seen.insert(val) {
                args.remove(i + 1);
                args.remove(i);
                continue;
            }
            i += 2;
            continue;
        }
        if let Some(r) = args[i].strip_prefix("--tweakClass=") {
            let v = r.to_string();
            if !r.is_empty() && !seen.insert(v) {
                args.remove(i);
                continue;
            }
        }
        i += 1;
    }
}

fn strip_fml_cascaded_tweak_classes_from_argv(args: &mut Vec<String>) {
    const FML_TWEAKER: &str = "cpw.mods.fml.common.launcher.FMLTweaker";
    if !existing_tweak_class_values(args).contains(FML_TWEAKER) {
        return;
    }
    const DROP: &[&str] = &[
        "cpw.mods.fml.common.launcher.FMLInjectionAndSortingTweaker",
        "cpw.mods.fml.common.launcher.FMLDeobfTweaker",
        "cpw.mods.fml.common.launcher.TerminalTweaker",
        "net.minecraft.launchwrapper.VanillaTweaker",
    ];
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == "--tweakClass" && i + 1 < args.len() {
            let val = args[i + 1].as_str();
            if DROP.contains(&val) {
                args.remove(i + 1);
                args.remove(i);
                continue;
            }
            i += 2;
            continue;
        }
        if let Some(rest) = args[i].strip_prefix("--tweakClass=") {
            if DROP.contains(&rest) {
                args.remove(i);
                continue;
            }
        }
        i += 1;
    }
}

fn forge_classpath_entry_prefer_universal_on_disk(entry: &Path) -> String {
    let name = match entry.file_name().and_then(|s| s.to_str()) {
        Some(n) => n,
        None => return entry.to_string_lossy().replace('\\', "/"),
    };
    if !name.starts_with("forge-") || !name.ends_with(".jar") {
        return entry.to_string_lossy().replace('\\', "/");
    }
    if name.contains("-universal")
        || name.ends_with("-client.jar")
        || name.ends_with("-installer.jar")
    {
        return entry.to_string_lossy().replace('\\', "/");
    }
    let stem = match entry.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return entry.to_string_lossy().replace('\\', "/"),
    };
    let universal = match entry.parent() {
        Some(pa) => pa.join(format!("{stem}-universal.jar")),
        None => return entry.to_string_lossy().replace('\\', "/"),
    };
    if universal.is_file() {
        universal.to_string_lossy().replace('\\', "/")
    } else {
        entry.to_string_lossy().replace('\\', "/")
    }
}

fn legacy_forge_resolve_forge_jars_to_universal(classpath: &mut Vec<String>) {
    use std::collections::HashSet;
    let mut out: Vec<String> = Vec::with_capacity(classpath.len());
    let mut seen: HashSet<String> = HashSet::new();
    for p in classpath.iter() {
        let chosen = forge_classpath_entry_prefer_universal_on_disk(Path::new(p));
        if seen.insert(chosen.clone()) {
            out.push(chosen);
        }
    }
    *classpath = out;
}

/// Фрагменты vanilla `arguments.game`, которых ещё нет в листе (без дублей — joptsimple / FML).
fn vanilla_game_tokens_not_yet_in(existing: &[String], vanilla: &[String]) -> Vec<String> {
    let keys = game_args_declared_keys(existing);
    let tweak_have = existing_tweak_class_values(existing);
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < vanilla.len() {
        let a = &vanilla[i];
        if a == "--tweakClass" && i + 1 < vanilla.len() {
            let val = vanilla[i + 1].as_str();
            if tweak_have.contains(val) {
                i += 2;
            } else {
                out.push(a.clone());
                out.push(vanilla[i + 1].clone());
                i += 2;
            }
            continue;
        }
        if let Some(rest) = a.strip_prefix("--tweakClass=") {
            if rest.is_empty() {
                i += 1;
                continue;
            }
            if tweak_have.contains(rest) {
                i += 1;
            } else {
                out.push(a.clone());
                i += 1;
            }
            continue;
        }
        if let Some(name) = game_arg_option_name(a) {
            if keys.contains(name) {
                if !a.contains('=') && i + 1 < vanilla.len() && !vanilla[i + 1].starts_with('-') {
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
        }
        out.push(a.clone());
        if game_arg_option_name(a).is_some()
            && !a.contains('=')
            && i + 1 < vanilla.len()
            && !vanilla[i + 1].starts_with('-')
        {
            out.push(vanilla[i + 1].clone());
            i += 2;
            continue;
        }
        i += 1;
    }
    out
}

fn game_arg_value_needs_version_fix(val: &str) -> bool {
    let t = val.trim();
    t.is_empty() || t.starts_with("${")
}

/// NeoForge/FML ожидают непустой `--version` (иначе `NullPointerException` на `version.toLowerCase()`).
fn repair_game_args_minecraft_version(game: &mut Vec<String>, mc_version: &str) {
    let ver = mc_version.trim();
    if ver.is_empty() {
        return;
    }

    let mut i = 0usize;
    while i < game.len() {
        if game[i] == "--version" {
            let next_bad = i + 1 >= game.len()
                || game[i + 1].starts_with('-')
                || game_arg_value_needs_version_fix(&game[i + 1]);
            if next_bad {
                if i + 1 < game.len() && !game[i + 1].starts_with('-') {
                    game[i + 1] = ver.to_string();
                } else {
                    game.insert(i + 1, ver.to_string());
                }
            }
            return;
        }
        if let Some(rest) = game[i].strip_prefix("--version=") {
            if game_arg_value_needs_version_fix(rest) {
                game[i] = format!("--version={ver}");
            }
            return;
        }
        i += 1;
    }

    let pos = game
        .iter()
        .position(|x| x.starts_with("--fml"))
        .unwrap_or(game.len());
    game.insert(pos, ver.to_string());
    game.insert(pos, "--version".to_string());
}

/// `neoforge/<neo_artifact>-<game_version>` (или легаси с дефисом) → артефакт NeoForge (`26.1.2.7-beta`).
fn neo_forge_artefact_from_version_folder(version_id: &str, mc_game: &str) -> Option<String> {
    let parts: Vec<&str> = version_id.split('/').collect();
    if parts.len() == 3 && parts[0] == "neoforge" {
        return Some(parts[2].to_string());
    }
    let rest = version_id
        .strip_prefix("neoforge/")
        .or_else(|| version_id.strip_prefix("neoforge-"))?;
    let suf = format!("-{}", mc_game.trim());
    if rest.ends_with(&suf) {
        Some(rest[..rest.len() - suf.len()].to_string())
    } else {
        None
    }
}

/// Как в `version_layout`: `neoforge/…`, `forge/…` и легаси с дефисом.
fn minecraft_version_from_modded_version_folder(version_id: &str) -> Option<String> {
    crate::core::game::version_layout::minecraft_version_from_profile_id(version_id)
}

fn push_unique_mc_hint(hints: &mut Vec<String>, s: &str) {
    let t = s.trim();
    if !t.is_empty() && !hints.iter().any(|x| x == t) {
        hints.push(t.to_string());
    }
}

/// Источник для `client-*-official`: `versions` или уже скачанный official в libraries (timestamp-каталог).
fn forge_wrapper_client_source_candidates(
    data_dir: &Path,
    lib_dir: &Path,
    mc: &str,
) -> [PathBuf; 2] {
    let resolved = resolve_net_minecraft_client_dir_name(mc, lib_dir);
    [
        data_dir.join("versions").join(mc).join(format!("{mc}.jar")),
        lib_dir
            .join("net/minecraft/client")
            .join(&resolved)
            .join(format!("client-{resolved}-official.jar")),
    ]
}

fn forge_wrapper_official_destinations_for_mc(lib_dir: &Path, mc: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.push(
        lib_dir
            .join("net/minecraft/client")
            .join(mc)
            .join(format!("client-{mc}-official.jar")),
    );
    let resolved = resolve_net_minecraft_client_dir_name(mc, lib_dir);
    if resolved != mc {
        out.push(
            lib_dir
                .join("net/minecraft/client")
                .join(&resolved)
                .join(format!("client-{resolved}-official.jar")),
        );
    }
    out
}

fn read_fml_kv_slice(game: &[String], key: &str) -> Option<String> {
    let flag = format!("--{key}");
    let mut i = 0usize;
    while i < game.len() {
        if game[i] == flag {
            if i + 1 < game.len() && !game[i + 1].starts_with('-') {
                return Some(game[i + 1].clone());
            }
            return None;
        }
        if let Some(rest) = game[i].strip_prefix(&format!("{flag}=")) {
            return Some(rest.to_string());
        }
        i += 1;
    }
    None
}

fn strip_fml_keys(game: &[String], keys: &[&str]) -> Vec<String> {
    let mut out = Vec::with_capacity(game.len());
    let mut i = 0usize;
    while i < game.len() {
        let g = &game[i];
        let mut consumed = false;
        for k in keys {
            let flag = format!("--{k}");
            if g.as_str() == flag.as_str() {
                let takes_val = i + 1 < game.len() && !game[i + 1].starts_with('-');
                i += 1;
                if takes_val {
                    i += 1;
                }
                consumed = true;
                break;
            }
            if g.strip_prefix(&format!("{flag}=")).is_some() {
                i += 1;
                consumed = true;
                break;
            }
        }
        if consumed {
            continue;
        }
        out.push(g.clone());
        i += 1;
    }
    out
}

fn neoforge_neoform_suffix_from_mcp(mcp_client: &str, mc_version: &str) -> Option<String> {
    let prefix = format!("{mc_version}-");
    mcp_client
        .strip_prefix(&prefix)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// FancyModLoader: `VersionSupportMatrix` строится из `VersionInfo(mcVersion, …)`; при `null` Maven `DefaultArtifactVersion` падает (`toLowerCase`).
/// `ProgramArgs` при дублях `--fml.*` оставляет только **первый** ключ — пустой выигрывает → NPE.
fn repair_neoforge_fml_program_args(
    game: &mut Vec<String>,
    version_id: &str,
    mc_version_id: &str,
    data_dir: &Path,
) {
    const KEYS: [&str; 3] = ["fml.neoForgeVersion", "fml.mcVersion", "fml.neoFormVersion"];
    let mc = mc_version_id.trim();
    if mc.is_empty() {
        return;
    }

    let neo_from_args = read_fml_kv_slice(game, "fml.neoForgeVersion");
    let mc_from_args = read_fml_kv_slice(game, "fml.mcVersion");
    let form_from_args = read_fml_kv_slice(game, "fml.neoFormVersion");

    let mut neo = neo_from_args
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("${"))
        .map(String::from);
    if neo.is_none() {
        neo = neo_forge_artefact_from_version_folder(version_id, mc);
    }

    let mc_resolved = mc_from_args
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("${"))
        .map(String::from)
        .unwrap_or_else(|| mc.to_string());

    let form = form_from_args
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("${"))
        .map(String::from)
        .or_else(|| {
            crate::core::game::install::neoforge_mcp_client_from_version_folder(
                data_dir, version_id,
            )
            .and_then(|mcp| neoforge_neoform_suffix_from_mcp(&mcp, mc_resolved.as_str()))
        })
        .unwrap_or_else(|| "1".to_string());

    let mut stripped = strip_fml_keys(game, &KEYS);

    let mut insert = Vec::new();
    if let Some(n) = neo {
        insert.push("--fml.neoForgeVersion".to_string());
        insert.push(n);
    }
    insert.push("--fml.mcVersion".to_string());
    insert.push(mc_resolved);
    insert.push("--fml.neoFormVersion".to_string());
    insert.push(form);

    let pos = stripped
        .iter()
        .position(|x| x.starts_with("--quickPlay") || x == "--server")
        .unwrap_or(stripped.len());
    for (off, item) in insert.into_iter().enumerate() {
        stripped.insert(pos + off, item);
    }
    *game = stripped;
}

/// FancyModLoader `LibraryFinder` / `GameLocator` в production читают `System.getProperty("libraryDirectory")`
/// (то же, что vanilla `${library_directory}`). Поздний `-DlibraryDirectory` в пользовательских JVM-аргументах
/// может указывать не на каталог лаунчера — тогда не находится `minecraft-client-patched` и fallback.
fn force_neoforge_library_directory_jvm_arg(args: &mut Vec<String>, lib_dir_str: &str) {
    args.retain(|a| !a.starts_with("-DlibraryDirectory="));
    args.push(format!("-DlibraryDirectory={}", lib_dir_str));
}

/// `GameLocator.locateProductionMinecraft`: либо patched jar, либо три частичных client + neoforge client.
fn ensure_neoforge_patched_or_fallback_jars(
    lib_dir: &Path,
    data_dir: &Path,
    game: &[String],
    version_id: &str,
    mc_fallback: &str,
) -> Result<()> {
    let neo = read_fml_kv_slice(game, "fml.neoForgeVersion")
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("${"))
        .map(String::from)
        .or_else(|| neo_forge_artefact_from_version_folder(version_id, mc_fallback));
    let Some(neo) = neo else {
        return Err(Error::Custom(
            "NeoForge: не удалось определить версию загрузчика (--fml.neoForgeVersion). \
             Ожидается id профиля вида neoforge/<версия_neoforge>-<версия_MC> (или легаси neoforge-…)."
                .into(),
        ));
    };

    let mc_owned = read_fml_kv_slice(game, "fml.mcVersion");
    let mc = mc_owned
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("${"))
        .unwrap_or(mc_fallback.trim())
        .to_string();

    let form_owned = read_fml_kv_slice(game, "fml.neoFormVersion");
    let form = form_owned
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("${"))
        .unwrap_or("1")
        .to_string();

    let mc_nf =
        crate::core::game::install::neoforge_mcp_client_from_version_folder(data_dir, version_id)
            .unwrap_or_else(|| format!("{mc}-{form}"));
    let patched = lib_dir.join(format!(
        "net/neoforged/minecraft-client-patched/{neo}/minecraft-client-patched-{neo}.jar"
    ));
    if patched.is_file() {
        return Ok(());
    }

    let srg = lib_dir.join(format!(
        "net/minecraft/client/{mc_nf}/client-{mc_nf}-srg.jar"
    ));
    let extra = lib_dir.join(format!(
        "net/minecraft/client/{mc_nf}/client-{mc_nf}-extra.jar"
    ));
    let nf_client = lib_dir.join(format!(
        "net/neoforged/neoforge/{neo}/neoforge-{neo}-client.jar"
    ));

    if srg.is_file() && extra.is_file() && nf_client.is_file() {
        return Ok(());
    }

    Err(Error::Custom(format!(
        "NeoForge: отсутствует patched-клиент и неполный набор fallback в libraries.\n\
         Ожидался файл:\n  {}\n\
         либо одновременно:\n  {}\n  {}\n  {}\n\n\
         Нажмите «Починить ядро» или переустановите NeoForge (должны отработать постпроцессоры установщика).",
        patched.display(),
        srg.display(),
        extra.display(),
        nf_client.display(),
    )))
}

/// Evaluates Mojang library rules. Returns true if the library should be
/// included on the current OS. When no rules are present, the library is allowed.
pub fn check_rules(rules: &Option<Vec<Value>>, current_os: &str) -> bool {
    let Some(rules) = rules else {
        return true;
    };
    if rules.is_empty() {
        return true;
    }

    let mut dominated_allow = false;
    let mut dominated_disallow = false;

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
            true // no OS restriction → matches all
        };

        let is_os_specific = rule.get("os").is_some();

        if action == "allow" {
            if os_match {
                if is_os_specific {
                    return true;
                }
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

/// Убирает флаги JVM 9+, которые Java 8 не принимает (иначе процесс падает до загрузки модов).
fn dedupe_java_library_path_singleton(args: &mut Vec<String>, natives_dir: &str, main_class: &str) {
    const P: &str = "-Djava.library.path=";
    args.retain(|a| !a.starts_with(P));
    let v = format!("{}{}", P, natives_dir.trim_end_matches('/'));
    if let Some(i) = args.iter().position(|a| a == "-cp") {
        args.insert(i, v);
    } else if let Some(i) = args.iter().position(|a| a == main_class) {
        args.insert(i, v);
    } else {
        args.insert(0, v);
    }
}

fn patch_fml_toml_early_window_control(path: &Path, early_window: bool) -> std::io::Result<()> {
    let val = if early_window { "true" } else { "false" };
    let new_line = format!("earlyWindowControl = {val}");
    let content = if path.is_file() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };
    let mut out: Vec<String> = Vec::new();
    let mut replaced = false;
    for line in content.lines() {
        let t = line.trim_start();
        if t.starts_with('#') {
            out.push(line.to_string());
            continue;
        }
        if t.starts_with("earlyWindowControl") && t.contains('=') {
            if !replaced {
                out.push(new_line.clone());
                replaced = true;
            }
        } else {
            out.push(line.to_string());
        }
    }
    if !replaced {
        if !out.is_empty() && out.last().map(|s| !s.trim().is_empty()).unwrap_or(false) {
            out.push(String::new());
        }
        out.push(new_line);
    }
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let body = out.join("\n");
    let mut f = std::fs::File::create(path)?;
    f.write_all(body.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

fn sh_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

fn posix_sh_wrapper_script(wrapper: &str, java_path: &str, argv: &[String]) -> String {
    let mut line = wrapper.trim().to_string();
    if !line.is_empty() {
        line.push(' ');
    }
    line.push_str(&sh_single_quote(java_path));
    for a in argv {
        line.push(' ');
        line.push_str(&sh_single_quote(a));
    }
    line
}

fn strip_jvm_flags_for_java8(prefix: Vec<String>, log: &impl Fn(&str)) -> Vec<String> {
    let mut out = Vec::with_capacity(prefix.len());
    let mut i = 0;
    while i < prefix.len() {
        let a = &prefix[i];
        let drop_with_value = matches!(
            a.as_str(),
            "--add-opens" | "--add-exports" | "--add-modules" | "--patch-module"
        );
        let drop_single = a.starts_with("--add-opens=")
            || a.starts_with("--add-exports=")
            || a.starts_with("--add-modules=")
            || a.starts_with("--patch-module=")
            || a.starts_with("--enable-native-access=")
            || (a.starts_with("-D") && (a.contains("jdk.module") || a.contains("jdk.attach")));
        if drop_with_value {
            log(&format!("Пропуск JVM-аргумента (нужна Java 9+): {}", a));
            i += 1;
            if i < prefix.len() {
                log(&format!(
                    "Пропуск JVM-аргумента (нужна Java 9+): {}",
                    &prefix[i]
                ));
                i += 1;
            }
            continue;
        }
        if drop_single {
            log(&format!("Пропуск JVM-аргумента (нужна Java 9+): {}", a));
            i += 1;
            continue;
        }
        out.push(a.clone());
        i += 1;
    }
    out
}

fn iter_mod_jars(mods_dir: &Path, mut visit: impl FnMut(&Path)) {
    let Ok(rd) = std::fs::read_dir(mods_dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("jar") {
            visit(&p);
        } else if p.is_dir() {
            let Ok(rd2) = std::fs::read_dir(&p) else {
                continue;
            };
            for e2 in rd2.flatten() {
                let p2 = e2.path();
                if p2.is_file() && p2.extension().and_then(|x| x.to_str()) == Some("jar") {
                    visit(&p2);
                }
            }
        }
    }
}

fn manifest_has_mixin_tweaker(jar: &Path) -> bool {
    let Ok(file) = File::open(jar) else {
        return false;
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return false;
    };
    for name in ["META-INF/MANIFEST.MF", "META-INF/manifest.mf"] {
        let Ok(mut ent) = zip.by_name(name) else {
            continue;
        };
        let mut s = String::new();
        if ent.read_to_string(&mut s).is_err() {
            continue;
        }
        if s.contains("org.spongepowered.asm.launch.MixinTweaker") || s.contains("MixinTweaker") {
            return true;
        }
    }
    false
}

/// Forge 1.7.10 + Mixin без UniMixins почти всегда падает при загрузке MixinTweaker.
fn warn_if_mixin_mods_need_unimixins_1710(game_dir: &Path, log: &impl Fn(&str)) {
    let mods_dir = game_dir.join("mods");
    if !mods_dir.is_dir() {
        return;
    }
    let mut has_unimixins = false;
    let mut mixin_example: Option<String> = None;
    iter_mod_jars(&mods_dir, |p| {
        let low = p
            .file_name()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if low.contains("unimixins") {
            has_unimixins = true;
        }
        if manifest_has_mixin_tweaker(p) {
            mixin_example.get_or_insert_with(|| {
                p.file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.display().to_string())
            });
        }
    });
    let Some(name) = mixin_example else {
        return;
    };
    if has_unimixins {
        return;
    }
    log(&format!(
        "В mods обнаружен Mixin-мод «{}» без UniMixins. На Forge 1.7.10 для таких модов установите **UniMixins** (-all) с Modrinth или CurseForge и положите jar в mods (часто переименовывают с префиксом «!», чтобы он грузился первым). Иначе игра часто вылетает сразу после строки про MixinTweaker.",
        name
    ));
}

pub(crate) struct RunningSession {
    pub(crate) child: Arc<Mutex<Option<Child>>>,
    /// PID корневого процесса Java (и потомков — для поиска окна игры).
    pub(crate) root_pid: u32,
}

/// Подстановка `${user_type}` в профиле версии → флаг `--userType` у игры/Forge.
/// Для Ely.by и Microsoft должен быть `mojang`; значение `elyby` ломает цепочку сессии/скинов при authlib-injector.
/// Для офлайн-аккаунта раньше подставляли `legacy` — в актуальных клиентах это часто **блокирует мультиплеер**
/// (вкладка/вход на сервер), хотя с Ely.by с тем же jar всё работает. Держим `mojang` и для пиратки.
fn minecraft_launch_user_type(_acc_type: &str) -> &'static str {
    "mojang"
}

/// После изменений на стороне Mojang Session API старый Authlib в **1.16.x** при «не настоящей» сессии
/// получает JSON с ошибочным HTTP-кодом, но **не проверяет код** — клиент считает сессию битой и
/// **отключает «Мультиплеер» / Realms** на главном экране (Ely.by с authlib-injector не страдает).
/// Обход: направить API на несуществующий хост (fail fast), как в
/// <https://github.com/FabricMC/fabric-loom/issues/915#issuecomment-1609154390>
fn needs_minecraft_116_offline_api_workaround(mc_version_id: &str) -> bool {
    let base = mc_version_id
        .split('-')
        .next()
        .unwrap_or(mc_version_id)
        .trim();
    let mut parts = base.split('.');
    let major: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    major == 1 && minor == 16
}

/// `--quickPlay*` и аналоги: 1.20+ (`1.21.11`) и новая нумерация без ведущей единицы (`26.1.2`).
fn minecraft_supports_quick_play(mc_version_id: &str) -> bool {
    let base = mc_version_id
        .split('-')
        .next()
        .unwrap_or(mc_version_id)
        .trim();
    let mut parts = base.split('.');
    let Some(major_s) = parts.next() else {
        return false;
    };
    let Ok(major) = major_s.parse::<u32>() else {
        return false;
    };
    if major >= 26 {
        return true;
    }
    if major != 1 {
        return false;
    }
    let Some(minor_s) = parts.next() else {
        return false;
    };
    let Ok(minor) = minor_s.parse::<u32>() else {
        return false;
    };
    minor >= 20
}

/// Корень Yggdrasil для authlib-injector (полный URL надёжнее короткого `ely.by` для редиректа session/texture).
const ELYBY_YGGDRASIL_API_ROOT: &str = "https://authserver.ely.by";

const JVM_UNLOCK_EXPERIMENTAL: &str = "-XX:+UnlockExperimentalVMOptions";

fn jvm_args_has_openal_libname(args: &[String]) -> bool {
    args.iter()
        .any(|a| a.starts_with("-Dorg.lwjgl.openal.libname"))
}

/// ForgeWrapper: флаги для установщика Forge. Через `JAVA_TOOL_OPTIONS` попадают и во вложенные
/// `java` (PostProcessors), куда аргументы родительского процесса не передаются.
const FORGE_WRAPPER_EXTRA_JVM: &[&str] = &[
    "-Xss4m",
    "--add-opens=java.base/java.lang=ALL-UNNAMED",
    "--add-opens=java.base/java.lang.invoke=ALL-UNNAMED",
    "--add-opens=java.base/java.lang.reflect=ALL-UNNAMED",
    "--add-opens=java.base/java.util=ALL-UNNAMED",
    "--add-opens=java.base/java.util.jar=ALL-UNNAMED",
    "--add-opens=java.base/java.io=ALL-UNNAMED",
    "--add-opens=java.base/java.nio=ALL-UNNAMED",
    "--add-opens=java.base/sun.nio.ch=ALL-UNNAMED",
    "--add-opens=java.base/java.net=ALL-UNNAMED",
    "--add-opens=java.base/java.security=ALL-UNNAMED",
    "--add-opens=java.base/jdk.internal.misc=ALL-UNNAMED",
    "--add-opens=java.base/sun.util.calendar=ALL-UNNAMED",
];

/// Хранит запущенные процессы по instance_id для возможности остановки
static RUNNING: Lazy<Mutex<HashMap<String, RunningSession>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Запущена ли хотя бы одна сессия Minecraft из лаунчера (оверлей / подсказки).
pub fn any_game_session_running() -> bool {
    RUNNING.lock().map(|g| !g.is_empty()).unwrap_or(false)
}

/// ID сборок с активным процессом игры (для UI после смены вкладки / пересоздания компонента).
pub fn running_instance_ids() -> Vec<String> {
    RUNNING
        .lock()
        .map(|g| g.keys().cloned().collect())
        .unwrap_or_default()
}

/// Корневые PID запущенных сессий (Java).
pub fn running_game_root_pids() -> Vec<u32> {
    RUNNING
        .lock()
        .map(|g| g.values().map(|s| s.root_pid).collect())
        .unwrap_or_default()
}

/// (instance_id, root_pid) для статистики оверлея.
pub fn running_sessions_pid_map() -> Vec<(String, u32)> {
    RUNNING
        .lock()
        .map(|g| g.iter().map(|(id, s)| (id.clone(), s.root_pid)).collect())
        .unwrap_or_default()
}

pub async fn launch(
    app: AppHandle,
    instance_id: &str,
    version_id: &str,
    username: &str,
    uuid: &str,
    token: &str,
    acc_type: &str,
    server_ip: Option<&str>,
    world_name: Option<&str>,
    flux_trace: Option<crate::core::fluxcore::v3::types::FluxTraceContext>,
    classpath_hint: Option<crate::core::fluxcore::v3::types::ClasspathSnapshot>,
) -> Result<String> {
    // `classpath_hint` — подсказка от v3/runner.rs о fingerprint предыдущего classpath.
    // Сейчас используется как дополнительный positive-сигнал warm-path (совпадение с LaunchCache).
    // В будущих фазах может быть использована для fast-validate без mtime-чтения.
    let _ = classpath_hint;
    let data_dir = crate::config::get_data_dir();
    let settings = crate::config::load_settings().unwrap_or_default();
    let game_dir = data_dir.join("instances").join(instance_id);
    let natives_dir = game_dir.join("natives");
    let natives_pool = profile_dir(&data_dir, version_id).join("natives");
    let lib_dir = data_dir.join("libraries");

    std::fs::create_dir_all(&game_dir)?;
    let _ = std::fs::create_dir_all(game_dir.join("mods"));
    let _ = std::fs::create_dir_all(&natives_dir);

    // FluxCore v3 / Phase 1.4: раннее зондирование LaunchCache до дорогих шагов.
    // Если LaunchCache валиден (все classpath/native/java-пути существуют и версия cache = 1),
    // пропускаем clear + reflink nativeов — они уже корректны с прошлого успешного запуска.
    let early_cache_probe = LaunchCache::load(&game_dir);
    let natives_dir_has_files = std::fs::read_dir(&natives_dir)
        .map(|it| it.flatten().any(|e| e.path().is_file()))
        .unwrap_or(false);
    let skip_natives_refresh = early_cache_probe.is_some() && natives_dir_has_files;

    // Phase 1.5: тот же digest/snapshot-пробег делает `fluxcore::v3::runner::run_game_launch`
    // параллельно до вызова launch. Дублирующий проход здесь был добавлен ошибочно и стоил
    // лишние ~50–150 мс (два I/O + парс JSON) на каждом запуске. Удалено.
    if !skip_natives_refresh {
        crate::core::game::install::clear_native_artifact_files(&natives_dir);
        if natives_pool.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&natives_pool) {
                for entry in entries.flatten() {
                    let src = entry.path();
                    if src.is_file() {
                        let dst = natives_dir.join(entry.file_name());
                        let _ = crate::core::fluxcore::storage::reflink_or_copy(&src, &dst);
                    }
                }
            }
        }
    }

    let inst_json_path = data_dir
        .join("instances")
        .join(instance_id)
        .join("instance.json");
    let instance_config: Option<crate::config::InstanceConfig> =
        std::fs::read_to_string(&inst_json_path)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok());

    let instance_override_java: Option<String> = instance_config.as_ref().and_then(|conf| {
        conf.settings.as_ref().and_then(|s| {
            let t = s.custom_java_path.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        })
    });

    // Персональное ОЗУ — только если включено «Использовать персональные настройки (ОЗУ, JVM)» в сборке.
    let (ram_mb, ram_label) = match instance_config
        .as_ref()
        .and_then(|c| c.settings.as_ref())
        .filter(|s| s.override_global)
    {
        Some(s) if s.ram_mb >= 512 => (s.ram_mb, "настройки сборки"),
        _ => (settings.ram_mb, "общие настройки лаунчера"),
    };

    // Phase 1.6 (log-batching) отключён: в реальных замерах пользователя холодный запуск
    // стал на ~8 с медленнее. Вариант с Mutex + Drop-guard подозревается в лишнем jitter
    // и/или в «скрытии» прогресса от UI, что создаёт иллюзию зависшего старта. Возврат к
    // прямому `app.emit` — нулевой риск; оптимизацию вернём после профилирования с реальным
    // трейсом (см. `tracing` в main.rs). Тайминг по фазам теперь пишется в tracing.
    let launch_wall_t0 = std::time::Instant::now();
    let log = |msg: &str| {
        let _ = app.emit(
            &format!("log_{}", instance_id),
            format!("[JentleMemes] {}", msg),
        );
    };
    // Phase tracing: в UI-лог инстанса (видно пользователю без JM_LOG) + tracing (для
    // сыр/файлового лога). Формат компактный, чтобы не захламлять UI; пользователь
    // должен видеть где именно горит prep (на warm-path всё должно быть <500 мс).
    let mark_phase = |name: &'static str, t_prev: &mut std::time::Instant| {
        let now = std::time::Instant::now();
        let phase_ms = now.duration_since(*t_prev).as_millis() as u64;
        let total_ms = now.duration_since(launch_wall_t0).as_millis() as u64;
        tracing::info!(target: "fluxcore::launch", phase = name, phase_ms, total_ms);
        let _ = app.emit(
            &format!("log_{}", instance_id),
            format!(
                "[JentleMemes] ⏱ phase={name} Δ={phase_ms}ms total={total_ms}ms"
            ),
        );
        *t_prev = now;
    };

    let mut phase_t = std::time::Instant::now();
    log("▸ Сборка профилей версий...");
    mark_phase("init", &mut phase_t);

    {
        let _ft_boot = flux_trace.as_ref().map(|fx| {
            FluxTraceScope::enter(
                &app,
                instance_id,
                fx.correlation_id,
                HelixStageId::BootstrapPromote,
            )
        });
        match crate::core::game::install::revert_broken_promote_if_needed(version_id) {
            Ok(true) => log("  Forge: откат на ForgeWrapper — процессорные артефакты отсутствуют."),
            Err(e) => log(&format!("  Forge revert check: {}", e)),
            _ => {}
        }

        if let Err(e) = crate::core::game::install::promote_forge_wrapper_to_bootstrap(version_id) {
            log(&format!("  Forge/Bootstrap: {}", e));
        }
    }

    // FluxCore v3: единый вход в резолв цепочки профилей (chain_resolver).
    // Ранее парсинг делался здесь инлайном; вынесено в `fluxcore::chain_resolver::resolve`,
    // launch оставляет только шаги сверх резолвера (`neoforge_fml_startup_client` и т.п.).
    let chain = {
        let _ft_profiles = flux_trace.as_ref().map(|fx| {
            FluxTraceScope::enter(
                &app,
                instance_id,
                fx.correlation_id,
                HelixStageId::ResolveProfiles,
            )
        });
        crate::core::fluxcore::chain_resolver::resolve(&data_dir, version_id)?
    };
    let all_v_infos = chain.all_v_infos.clone();
    // Пути цепочки профиля используются LaunchCache для хеша входа (mtime-based).
    let profile_chain_paths: Vec<PathBuf> = chain.profile_json_paths.clone();
    log(&format!("  Найдено профилей: {} (цепочка inheritsFrom)", all_v_infos.len()));
    mark_phase("resolve_profiles", &mut phase_t);

    // FluxCore v3: LaunchCache warm-path. Hash цепочки профилей + пользовательские JVM-настройки.
    // При совпадении — пропускаем тяжёлые install-time шаги (resolve forge universals, промежуточные
    // проверки classpath) и используем сохранённый classpath/java_path/forge_client_jar.
    let chain_hash_current = launch_cache::compute_chain_hash(&profile_chain_paths);
    let settings_hash_current = launch_cache::compute_settings_hash(
        ram_mb,
        &settings.jvm_args,
        &settings.custom_java_path,
    );
    let cached_launch = LaunchCache::load(&game_dir);
    let cache_hit = cached_launch
        .as_ref()
        .map(|c| c.chain_hash == chain_hash_current && c.settings_hash == settings_hash_current)
        .unwrap_or(false);
    // Явное трассирование почему cache_hit=false. Видим в логе, какое из условий сломало
    // тёплый старт: файла нет, classpath-entry отсутствует, chain_hash не совпал, settings_hash не совпал.
    if cache_hit {
        log("▸ FluxCore LaunchCache: hit — тёплый запуск, пропускаем forge universal resolve.");
    } else if let Some(prev) = cached_launch.as_ref() {
        let chain_match = prev.chain_hash == chain_hash_current;
        let settings_match = prev.settings_hash == settings_hash_current;
        tracing::info!(
            target: "fluxcore::launch",
            event = "cache_miss",
            prev_chain = %prev.chain_hash,
            curr_chain = %chain_hash_current,
            prev_settings = %prev.settings_hash,
            curr_settings = %settings_hash_current,
            chain_match,
            settings_match,
            classpath_len = prev.classpath.len(),
        );
    } else {
        tracing::info!(
            target: "fluxcore::launch",
            event = "cache_miss",
            reason = "no_snapshot_on_disk",
        );
    }

    let main_v_info = all_v_infos.last().unwrap().clone();
    let vanilla_info = all_v_infos.first().unwrap().clone();

    let main_class = chain.main_class.clone();
    let is_forge_wrapper = chain.is_forge_wrapper;
    let is_bootstrap_launcher = chain.is_bootstrap_launcher;
    let neoforge_fml_startup_client = is_neoforge_profile_id(version_id)
        && !is_forge_wrapper
        && main_class.to_lowercase().contains("fml.startup.client");

    log(&format!("▸ Главный класс: {} {}",
        main_class,
        if is_forge_wrapper { "(ForgeWrapper)" }
        else if is_bootstrap_launcher { "(BootstrapLauncher)" }
        else if neoforge_fml_startup_client { "(NeoForge FML)" }
        else { "" }
    ));

    if let Err(e) = crate::core::game::install::ensure_neoforge_client_srg_or_error(
        &data_dir,
        version_id,
        &main_class,
    ) {
        return Err(e);
    }

    #[cfg(target_os = "linux")]
    let current_os = "linux";
    #[cfg(target_os = "windows")]
    let current_os = "windows";
    #[cfg(target_os = "macos")]
    let current_os = "osx";

    let lib_dir_str = lib_dir.to_string_lossy().replace('\\', "/");
    let natives_dir_str = natives_dir.to_string_lossy().replace('\\', "/");
    let game_dir_str = game_dir.to_string_lossy().replace('\\', "/");
    let assets_dir_str = data_dir.join("assets").to_string_lossy().replace('\\', "/");
    let asset_index = main_v_info
        .assets
        .clone()
        .or_else(|| vanilla_info.assets.clone())
        .unwrap_or_else(|| "legacy".into());

    let leaf_inherits = main_v_info
        .inherits_from
        .clone()
        .unwrap_or_else(|| version_id.to_string());
    let mc_from_modded = minecraft_version_from_profile_id(version_id)
        .filter(|s| looks_like_minecraft_game_version(s));
    let mc_from_root = vanilla_info.id.as_ref().and_then(|id| {
        let t = id.trim();
        (!t.is_empty() && looks_like_minecraft_game_version(t)).then(|| t.to_string())
    });
    let mc_version_id = mc_from_modded
        .or(mc_from_root)
        .unwrap_or_else(|| {
            vanilla_info
                .id
                .as_ref()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or(leaf_inherits)
        });
    // `versions/<id>/<id>.jar`: для `forge|neoforge/<MC>/<loader>` берём MC из id профиля, иначе ошибочный
    // корень (например каталог `versions/26.1.2` вместо 1.21.6) даёт неверный jar_id.
    let jar_id = mc_version_id.as_str();
    // Forge 1.20.x: артефакты client часто в `1.20.1-20230612.114412`, не в `1.20.1`.
    let client_maven_dir = resolve_net_minecraft_client_dir_name(jar_id, &lib_dir);
    let official_client_jar = data_dir
        .join("libraries")
        .join("net")
        .join("minecraft")
        .join("client")
        .join(&client_maven_dir)
        .join(format!("client-{}-official.jar", client_maven_dir));
    // install_profile постпроцессоров NeoForge/Forge ссылается на `.../client/<id>/client-<id>-official.jar`
    // с «голым» id (1.21.6), а не на каталог вроде `1.21.6-20250514...` из resolve_net_minecraft_client_dir_name.
    let plain_mc_official_jar = lib_dir
        .join("net/minecraft/client")
        .join(jar_id)
        .join(format!("client-{}-official.jar", jar_id));
    let default_client_jar = data_dir
        .join("versions")
        .join(jar_id)
        .join(format!("{}.jar", jar_id));
    let custom_installer = profile_dir(&data_dir, version_id).join("installer.jar");
    let main_v_json = serde_json::to_value(&main_v_info).unwrap_or(Value::Null);
    let resolved_installer_jar: Option<PathBuf> = if custom_installer.is_file() {
        Some(custom_installer.clone())
    } else {
        crate::core::game::install::forge_installer_jar_path(version_id, &main_v_json, &data_dir)
    };
    let forge_installer_path = resolved_installer_jar
        .as_ref()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();

    log(&format!("▸ Версия Minecraft: {jar_id}"));

    if main_class.to_lowercase().contains("launchwrapper") {
        crate::core::game::install::ensure_vanilla_versions_jar_for_legacy_fml(
            &data_dir,
            &lib_dir,
            jar_id,
            &log,
        )
        .await?;
    }

    let _ = std::fs::create_dir_all(game_dir.join("mods").join(jar_id));

    if (is_bootstrap_launcher || neoforge_fml_startup_client)
        && (is_forge_profile_id(version_id) || is_neoforge_profile_id(version_id))
    {
        let mut missing: Vec<String> = Vec::new();
        for v_info in &all_v_infos {
            for lib in v_info.libraries.iter().chain(v_info.maven_files.iter()) {
                if !check_rules(&lib.rules, current_os) {
                    continue;
                }
                if lib.name.trim().is_empty() {
                    continue;
                }
                let rel = library_rel_path_effective_for_os(lib, jar_id, current_os);
                if !lib_dir.join(&rel).is_file() {
                    missing.push(rel.replace('\\', "/"));
                }
            }
        }
        if !missing.is_empty() {
            let sample = missing
                .iter()
                .take(12)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            let tail = if missing.len() > 12 {
                format!("\n… и ещё {} файлов.", missing.len() - 12)
            } else {
                String::new()
            };
            return Err(Error::Custom(format!(
                "Для BootstrapLauncher / NeoForge FML Client не хватает jar в libraries (часто — профиль перевели на Bootstrap до ForgeWrapper или удалили libraries):\n{}{}\n\nВ настройках сборки: «Починить ядро» или переустановите загрузчик.",
                sample, tail
            )));
        }
    }

    // Fast-path: legacy-forge universals нужны только Forge/NeoForge. Для Fabric/Quilt/
    // vanilla это пустая работа — пробег по цепочке и list-dir'ы по libraries/. Раньше
    // даже на Fabric мы заходили в `resolve_legacy_forge_universals_chunked` и находили
    // пустой `forge_coords`. Short-circuit экономит 1-4 мс на каждом launch + спавн
    // тред-пула strand'ов.
    let needs_legacy_forge_resolve = !cache_hit
        && (is_forge_profile_id(version_id) || is_neoforge_profile_id(version_id));
    if needs_legacy_forge_resolve {
        let _ft_forge_uni = flux_trace.as_ref().map(|fx| {
            FluxTraceScope::enter(
                &app,
                instance_id,
                fx.correlation_id,
                HelixStageId::LegacyForgeUniversal,
            )
        });
        let mut forge_coords: Vec<String> = Vec::new();
        for v_info in &all_v_infos {
            for lib in v_info.libraries.iter().chain(v_info.maven_files.iter()) {
                let n = lib.name.trim();
                if (n.starts_with("net.minecraftforge:forge:")
                    || n.starts_with("net.neoforged:forge:"))
                    && n.split(':').count() == 3
                {
                    let coord =
                        crate::core::utils::maven::normalize_legacy_forge_minecraftforge_coord(
                            n, jar_id,
                        )
                        .unwrap_or_else(|| n.to_string());
                    if !forge_coords.contains(&coord) {
                        forge_coords.push(coord);
                    }
                }
            }
        }
        let inst_pb = resolved_installer_jar
            .as_ref()
            .filter(|p| p.is_file())
            .cloned();
        let n_strands =
            crate::core::fluxcore::v3::strand::suggested_parallel_strands(120) as usize;
        crate::core::fluxcore::v3::strand::resolve_legacy_forge_universals_chunked(
            forge_coords,
            &lib_dir,
            inst_pb,
            n_strands.max(2),
        )
        .await
        .map_err(|e| {
            Error::Custom(format!(
                "Forge legacy universal: не удалось подготовить эталонный *-universal.jar (Maven/установщик). Без него FML падает на ModDiscoverer/ASM. {e}"
            ))
        })?;
        mark_phase("legacy_forge_universals", &mut phase_t);
    } else if cache_hit {
        mark_phase("legacy_forge_universals(skip:cache)", &mut phase_t);
    } else {
        mark_phase("legacy_forge_universals(skip:non_forge)", &mut phase_t);
    }

    let mut classpath: Vec<String> = Vec::new();
    let mut lib_key_index: HashMap<String, usize> = HashMap::new();
    let mut cp_missing: Vec<String> = Vec::new();
    for v_info in &all_v_infos {
        for lib in v_info.libraries.iter().chain(v_info.maven_files.iter()) {
            if !check_rules(&lib.rules, current_os) {
                continue;
            }

            let coord_name = crate::core::utils::maven::normalize_legacy_forge_minecraftforge_coord(
                &lib.name,
                jar_id,
            )
            .unwrap_or_else(|| lib.name.clone());
            let mut lib_resolved = lib.clone();
            lib_resolved.name = coord_name.clone();
            let rel = library_rel_path_for_classpath(&lib_resolved, current_os);
            let mut full_path = lib_dir.join(&rel);
            let parts_fc: Vec<&str> = coord_name.split(':').collect();
            let is_forge_no_classifier = parts_fc.len() == 3
                && (coord_name.starts_with("net.minecraftforge:forge:")
                    || coord_name.starts_with("net.neoforged:forge:"));
            if !full_path.is_file() && is_forge_no_classifier {
                let uni = lib_dir.join(maven_to_path(&coord_name, Some("universal")));
                if uni.is_file() {
                    full_path = uni;
                }
            }
            if !full_path.is_file() {
                if is_forge_no_classifier {
                    let _ =
                        crate::core::game::install::ensure_forge_no_classifier_jar_for_classpath(
                            resolved_installer_jar.as_deref(),
                            &lib_dir,
                            coord_name.trim(),
                        );
                }
                if !full_path.is_file() && is_forge_no_classifier {
                    let uni = lib_dir.join(maven_to_path(&coord_name, Some("universal")));
                    if uni.is_file() {
                        full_path = uni;
                    }
                }
                if !full_path.is_file() {
                    cp_missing.push(coord_name);
                    continue;
                }
            }

            let parts: Vec<&str> = coord_name.split(':').collect();
            let dedup_key = if parts.len() >= 4 {
                format!(
                    "{}:{}:{}",
                    parts[0],
                    parts[1],
                    parts[3].split('@').next().unwrap_or("")
                )
            } else if parts.len() >= 2 {
                format!("{}:{}", parts[0], parts[1])
            } else {
                coord_name.clone()
            };
            let path_str = full_path.to_string_lossy().replace('\\', "/");
            if let Some(&idx) = lib_key_index.get(&dedup_key) {
                classpath[idx] = path_str;
            } else {
                lib_key_index.insert(dedup_key, classpath.len());
                classpath.push(path_str);
            }
        }
    }
    if !cp_missing.is_empty() {
        log(&format!(
            "⚠ Classpath: пропущено {} библиотек (нет файла): {}",
            cp_missing.len(),
            cp_missing.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
        ));
    }

    let legacy_forge_launchwrapper = main_class.to_lowercase().contains("launchwrapper");
    if legacy_forge_launchwrapper {
        let before = classpath.len();
        classpath.retain(|p| !p.ends_with("-installer.jar"));
        if classpath.len() != before {
            log(&format!(
                "Forge (LaunchWrapper): с classpath снято {} *-installer.jar (нужны только установщику, не клиенту; иначе возможны ложные «tampering» / minecraft.jar).",
                before - classpath.len()
            ));
        }
        let before2 = classpath.join("\n");
        legacy_forge_resolve_forge_jars_to_universal(&mut classpath);
        let after2 = classpath.join("\n");
        if before2 != after2 {
            log(
                "Forge (LaunchWrapper): в classpath подставлен forge-*-universal.jar рядом с голым forge-*.jar (на диске; FML должен грузить universal, а не повреждённый no-classifier).",
            );
        }
        if crate::core::game::legacy_java_fixer::needs_legacy_java_fixer(jar_id) {
            let _ = crate::core::game::legacy_java_fixer::sync_to_instance_mods(&game_dir);
        }
    }

    log(&format!("▸ Classpath: {} библиотек", classpath.len()));
    mark_phase("build_classpath", &mut phase_t);

    // NeoForge: ForgeWrapper тянет `cpw.mods.modlauncher.Launcher`. Версия `fancymodloader:loader` в JSON
    // не всегда совпадает с опубликованным modlauncher (см. maven-metadata.xml) — качаем с fallback.
    if is_forge_wrapper && is_neoforge_profile_id(version_id) {
        if let Some(ml_hint) =
            crate::core::utils::modlauncher::modlauncher_version_from_neoforge_chain(&all_v_infos)
        {
            let full_path =
                crate::core::utils::modlauncher::download_neoforge_modlauncher_to_libraries(
                    &lib_dir, &ml_hint, &log,
                )
                .await?;
            let path_str = full_path.to_string_lossy().replace('\\', "/");
            classpath.retain(|p| !p.contains("/cpw/mods/modlauncher/"));
            if let Some(idx) = classpath.iter().position(|ent| {
                ent.contains("jentlememes") && ent.contains("wrapper")
                    || ent.contains("/wtf/jentlememes/")
            }) {
                classpath.insert(idx + 1, path_str);
            } else {
                classpath.insert(0, path_str);
            }
        }
    }

    // NeoForge: без `minecraft-client-patched` на -cp при `neoforge-universal` FML видит «обрезанный»
    // RequiredSystemFiles → `missing_minecraft_jar` (в UI — «patched jar is missing»). С patched на -cp FML идёт
    // в dev/joined и требует в MANIFEST главной секции `Minecraft-Dists: client` (см. NeoForgeDevDistCleaner) —
    // добавляем атрибут однократно в `install::ensure_neoforge_patched_jar_minecraft_dists`.
    let patched_on_disk = is_neoforge_profile_id(version_id)
        && neo_forge_artefact_from_version_folder(version_id, &mc_version_id)
            .map(|neo| {
                lib_dir.join(format!(
                    "net/neoforged/minecraft-client-patched/{neo}/minecraft-client-patched-{neo}.jar"
                ))
                .is_file()
            })
            .unwrap_or(false);

    if patched_on_disk {
        if let Some(neo) = neo_forge_artefact_from_version_folder(version_id, &mc_version_id) {
            let pj = lib_dir.join(format!(
                "net/neoforged/minecraft-client-patched/{neo}/minecraft-client-patched-{neo}.jar"
            ));
            if pj.is_file() {
                crate::core::game::install::ensure_neoforge_patched_jar_minecraft_dists(&pj)?;
                let ps = pj.to_string_lossy().replace('\\', "/");
                if !classpath.iter().any(|c| c == &ps) {
                    let insert_at = classpath
                        .iter()
                        .position(|e| {
                            (e.contains("jentlememes") && e.contains("wrapper"))
                                || e.contains("/wtf/jentlememes/")
                        })
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    classpath.insert(insert_at, ps);
                    log("NeoForge: minecraft-client-patched на classpath (при необходимости дописан Minecraft-Dists в MANIFEST).");
                }
            }
        }
    }

    let jar_path = data_dir
        .join("versions")
        .join(jar_id)
        .join(format!("{}.jar", jar_id));

    let mut installer_plain_mc_hints: Vec<String> = Vec::new();
    if let Some(ref ij) = resolved_installer_jar {
        if ij.is_file() {
            if let Some(m) = crate::core::game::install::forge_minecraft_version_from_installer_jar(ij) {
                installer_plain_mc_hints.push(m);
            }
        }
    }
    if let Some(m) = minecraft_version_from_modded_version_folder(version_id) {
        if !installer_plain_mc_hints.contains(&m) {
            installer_plain_mc_hints.push(m);
        }
    }

    // NeoForge: никогда не кладём `versions/<mc>/<mc>.jar` на -cp. Для ForgeWrapper до постпроцессора
    // patched ещё нет — раньше сюда попадал vanilla, GameLocator помечал его claimed и после PROCESS_MINECRAFT_JAR
    // не брал `minecraft-client-patched` из libraries → «The patched Minecraft jar is missing».
    let skip_vanilla_versions_jar =
        is_bootstrap_launcher || neoforge_fml_startup_client || is_neoforge_profile_id(version_id);
    if jar_path.exists() {
        // Forge 1.17+ / NeoForge: ModLauncher грузит клиент как модуль `minecraft` из libraries (patched client).
        // Добавление `versions/1.20.1/1.20.1.jar` даёт второй модуль (`_1._20._1` по имени файла) с теми же пакетами →
        // ResolutionException: Modules minecraft and _1._20._1 export package com.mojang.blaze3d.platform to module forge.
        // NeoForge 26+ (`fml.startup.Client`) и ForgeWrapper: тот же jar на -cp ломает GameLocator/RequiredSystemFiles (см. debug.log).
        if skip_vanilla_versions_jar {
            log(&format!(
                "NeoForge / ModLauncher: пропуск {} в classpath (клиент через libraries / patched; иначе JPMS или ошибка GameLocator).",
                jar_path.display()
            ));
        } else {
            classpath.push(jar_path.to_string_lossy().replace('\\', "/"));
        }
    }
    if skip_vanilla_versions_jar {
        let vj = jar_path.to_string_lossy().replace('\\', "/");
        classpath.retain(|p| p != &vj);
    }

    let needs_official_client = is_forge_wrapper
        || (is_neoforge_profile_id(version_id) && !main_class.to_lowercase().contains("launchwrapper"));
    if needs_official_client {
        let mut all_mc_hints: Vec<String> = Vec::new();
        let push_h = |out: &mut Vec<String>, s: &str| {
            let t = s.trim();
            if !t.is_empty() && !out.iter().any(|x| x == t) {
                out.push(t.to_string());
            }
        };
        push_h(&mut all_mc_hints, jar_id);
        for h in &installer_plain_mc_hints {
            push_h(&mut all_mc_hints, h);
        }
        if let Some(m) = minecraft_version_from_modded_version_folder(version_id) {
            push_h(&mut all_mc_hints, &m);
        }

        let downloaded_jar = crate::core::game::install::download_official_client_jar_to_libraries(
            &data_dir,
            &all_mc_hints,
        )
        .await?;

        crate::core::game::install::ensure_official_client_jar_for_forge_libraries(
            &data_dir,
            version_id,
            &downloaded_jar,
            &all_mc_hints,
        )?;

        log(&format!("  client-official: готов ({})", downloaded_jar.display()));
    }

    // modlauncher требует модуль `jopt.simple`. У 6.0-alpha-3 в манифесте Automatic-Module-Name: joptsimple
    // (другое имя). SecureJarHandler даёт `jopt.simple` для 5.0.4 по Maven-пути без этого поля (McModLauncher/JarMetadata).
    if is_forge_wrapper {
        let jopt_504 = lib_dir.join("net/sf/jopt-simple/jopt-simple/5.0.4/jopt-simple-5.0.4.jar");
        if jopt_504.is_file() {
            let p = jopt_504.to_string_lossy().replace('\\', "/");
            for ent in classpath.iter_mut() {
                if ent.contains("/jopt-simple/") && ent.ends_with(".jar") {
                    *ent = p.clone();
                    log("ForgeWrapper: jopt-simple на classpath — 5.0.4 (модуль jopt.simple для modlauncher).");
                    break;
                }
            }
        } else {
            log("ForgeWrapper: нет libraries/.../jopt-simple-5.0.4.jar — при FindException jopt.simple выполните загрузку файлов или Починить ядро.");
        }
    }

    #[cfg(target_os = "windows")]
    let cp_sep = ";";
    #[cfg(not(target_os = "windows"))]
    let cp_sep = ":";
    {
        let mut seen = HashSet::<String>::new();
        classpath.retain(|p| seen.insert(p.clone()));
    }
    let classpath_str = classpath.join(cp_sep);
    let digest_res = crate::core::fluxcore::v3::coil::profile_inputs_digest(&data_dir, version_id);
    if let Some(fx) = &flux_trace {
        let sp = PulseSpan::begin(
            &app,
            instance_id,
            HelixStageId::ClasspathAssemble,
            fx.correlation_id,
        );
        if let Ok(ref d) = digest_res {
            let _ = crate::core::fluxcore::v3::coil::persist_classpath_snapshot(
                &game_dir,
                version_id,
                d,
                &classpath,
            );
        }
        sp.finish(&app, instance_id);
    } else if let Ok(ref d) = digest_res {
        let _ = crate::core::fluxcore::v3::coil::persist_classpath_snapshot(
            &game_dir,
            version_id,
            d,
            &classpath,
        );
    }
    let mc_user_type = minecraft_launch_user_type(acc_type);

    log("▸ Формирование аргументов JVM и игры...");

    let mut client_jar_path = if official_client_jar.is_file() {
        official_client_jar.to_string_lossy().replace('\\', "/")
    } else {
        default_client_jar.to_string_lossy().replace('\\', "/")
    };

    // Forge/NeoForge: установщик может быть только в libraries (mavenFiles), без versions/…/installer.jar.
    // Постпроцессоры смотрят на `minecraft` / `version` в install_profile.
    if let Some(ref inst_jar) = resolved_installer_jar {
        if inst_jar.is_file()
            && (is_forge_wrapper
                || is_forge_profile_id(version_id)
                || is_neoforge_profile_id(version_id))
        {
            log(&format!(
                "Forge: установщик для подготовки client-official: {}",
                inst_jar.display()
            ));
            let forge_mc_from_installer =
                crate::core::game::install::forge_minecraft_version_from_installer_jar(inst_jar);
            let mut mc_hints: Vec<String> = Vec::new();
            if let Some(ref m) = forge_mc_from_installer {
                push_unique_mc_hint(&mut mc_hints, m);
            }
            if let Some(m) = minecraft_version_from_modded_version_folder(version_id) {
                push_unique_mc_hint(&mut mc_hints, &m);
            }
            push_unique_mc_hint(&mut mc_hints, jar_id);

            let mut source: Option<PathBuf> = None;
            if jar_path.is_file() {
                source = Some(jar_path.clone());
            }
            if source.is_none() {
                for mc in &mc_hints {
                    let off = lib_dir
                        .join("net/minecraft/client")
                        .join(mc)
                        .join(format!("client-{mc}-official.jar"));
                    if off.is_file() {
                        source = Some(off);
                        break;
                    }
                }
            }
            if source.is_none() {
                'pick: for mc in &mc_hints {
                    for p in forge_wrapper_client_source_candidates(&data_dir, &lib_dir, mc) {
                        if p.is_file() {
                            source = Some(p);
                            break 'pick;
                        }
                    }
                }
            }

            let Some(ref src_path) = source else {
                let mut lines = Vec::new();
                for mc in &mc_hints {
                    for p in forge_wrapper_client_source_candidates(&data_dir, &lib_dir, mc) {
                        lines.push(format!("  {}", p.display()));
                    }
                }
                return Err(Error::Custom(format!(
                "ForgeWrapper: не найден vanilla client для подготовки client-<MC>-official.jar в libraries.\n\
                 Ожидался хотя бы один из путей:\n{}\n\
                 Выполните «Починить ядро» или дождитесь загрузки версии Minecraft ({}) в versions/ или libraries/.",
                lines.join("\n"),
                mc_hints.join(", ")
            )));
            };

            let mut all_dests: Vec<PathBuf> = Vec::new();
            for mc in &mc_hints {
                for d in forge_wrapper_official_destinations_for_mc(&lib_dir, mc) {
                    if !all_dests.contains(&d) {
                        all_dests.push(d);
                    }
                }
            }
            for extra in [official_client_jar.clone(), plain_mc_official_jar.clone()] {
                if !all_dests.contains(&extra) {
                    all_dests.push(extra);
                }
            }

            for dst in &all_dests {
                if dst.is_file() {
                    continue;
                }
                if let Some(p) = dst.parent() {
                    let _ = std::fs::create_dir_all(p);
                }
                match std::fs::copy(src_path, dst) {
                    Ok(_) => log(&format!(
                        "Forge: подготовлен {} (из {}).",
                        dst.display(),
                        src_path.display()
                    )),
                    Err(e) => log(&format!(
                        "Forge: не удалось скопировать {} ← {}: {e}",
                        dst.display(),
                        src_path.display()
                    )),
                }
            }

            let required_plain = if let Some(ref m) = forge_mc_from_installer {
                lib_dir
                    .join("net/minecraft/client")
                    .join(m)
                    .join(format!("client-{m}-official.jar"))
            } else {
                let primary_mc = mc_hints.first().map(|s| s.as_str()).unwrap_or(jar_id);
                lib_dir
                    .join("net/minecraft/client")
                    .join(primary_mc)
                    .join(format!("client-{primary_mc}-official.jar"))
            };
            if !required_plain.is_file() {
                return Err(Error::Custom(format!(
                    "ForgeWrapper: нет файла, который ждёт install_profile:\n  {}\n\
                 Источник: {}\n\
                 Проверьте права на запись в libraries/ и свободное место.",
                    required_plain.display(),
                    src_path.display()
                )));
            }

            if is_forge_profile_id(version_id) || is_neoforge_profile_id(version_id) {
                let mc_stem = crate::core::game::version_layout::minecraft_version_from_profile_id(
                    version_id,
                )
                .unwrap_or_else(|| {
                    forge_mc_from_installer
                        .as_deref()
                        .or_else(|| mc_hints.first().map(|s| s.as_str()))
                        .unwrap_or(jar_id)
                        .to_string()
                });
                let staged = profile_dir(&data_dir, version_id)
                    .join(format!("client-{mc_stem}-official.jar"));
                if !staged.is_file() {
                    if let Some(p) = staged.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let _ = std::fs::copy(&required_plain, &staged);
                }
            }

            client_jar_path = if official_client_jar.is_file() {
                official_client_jar.to_string_lossy().replace('\\', "/")
            } else if plain_mc_official_jar.is_file() {
                plain_mc_official_jar.to_string_lossy().replace('\\', "/")
            } else {
                required_plain.to_string_lossy().replace('\\', "/")
            };
        }
    }

    let replace_vars = |mut s: String| -> String {
        s = s
            .replace("${natives_directory}", &natives_dir_str)
            .replace("${launcher_name}", "JentleMemes")
            .replace("${launcher_version}", "1.0")
            .replace("${library_directory}", &lib_dir_str)
            .replace("${classpath_separator}", cp_sep)
            .replace("${version_name}", &mc_version_id)
            .replace("${classpath}", &classpath_str)
            .replace("${client_jar}", &client_jar_path)
            .replace("${auth_player_name}", username)
            .replace("${game_directory}", &game_dir_str)
            .replace("${assets_root}", &assets_dir_str)
            .replace("${assets_index_name}", &asset_index)
            .replace("${auth_uuid}", uuid)
            .replace("${auth_access_token}", token)
            .replace("${user_type}", mc_user_type)
            .replace(
                "${version_type}",
                main_v_info.type_.as_deref().unwrap_or("release"),
            )
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
                let path = lib_dir
                    .join(maven_to_path(inner, None))
                    .to_string_lossy()
                    .replace('\\', "/");
                s.replace_range(start..=end, &path);
            } else {
                break;
            }
        }
        s
    };

    let mut authlib_prefix_jvm: Vec<String> = Vec::new();
    if acc_type.eq_ignore_ascii_case("elyby") {
        match crate::core::authlib_injector::ensure_authlib_injector_jar(&data_dir).await {
            Ok(jar) => {
                let j = jar.to_string_lossy().replace('\\', "/");
                authlib_prefix_jvm.push(format!("-javaagent:{}={}", j, ELYBY_YGGDRASIL_API_ROOT));
                authlib_prefix_jvm.push("-Dauthlibinjector.side=client".into());
                log(&format!(
                    "Ely.by: authlib-injector → {}",
                    ELYBY_YGGDRASIL_API_ROOT
                ));
            }
            Err(e) => {
                return Err(Error::Custom(format!(
                    "Не удалось подготовить authlib-injector для Ely.by: {e}"
                )));
            }
        }
    }

    let mut args = Vec::new();
    for a in authlib_prefix_jvm {
        args.push(a);
    }

    let vanilla_mc_id = vanilla_info
        .id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(jar_id);
    if acc_type.eq_ignore_ascii_case("offline")
        && needs_minecraft_116_offline_api_workaround(vanilla_mc_id)
    {
        for (k, v) in [
            ("minecraft.api.auth.host", "https://nope.invalid"),
            ("minecraft.api.account.host", "https://nope.invalid"),
            ("minecraft.api.session.host", "https://nope.invalid"),
            ("minecraft.api.services.host", "https://nope.invalid"),
        ] {
            args.push(format!("-D{k}={v}"));
        }
        log(
            "Офлайн + Minecraft 1.16.x: JVM-обход Session API (-Dminecraft.api.*.host→nope.invalid), \
             иначе кнопка «Мультиплеер» остаётся неактивной (старый Authlib + новые ответы Mojang).",
        );
    }

    if settings.linux_jvm_unlock_experimental && !args.iter().any(|a| a == JVM_UNLOCK_EXPERIMENTAL)
    {
        args.push(JVM_UNLOCK_EXPERIMENTAL.into());
    }
    log(&format!(
        "Память Java (-Xmx): {} MB ({})",
        ram_mb, ram_label
    ));
    args.push(format!("-Xmx{}M", ram_mb));

    // -Xms: **маленький** начальный heap (512M), а не Xmx. Это 180° против первой
    // интуиции «одинаковый Xms/Xmx = меньше GC» — на практике эталонный Prism 8.x
    // передаёт `-Xms512m -Xmx16382m` и стартует за 14 с на том же модпаке, где
    // `-Xms==-Xmx` + pre-touch давал нам +5 с. Причина: JVM коммитит память лениво,
    // heap растёт по мере загрузки модов почти бесплатно (mmap), а принудительное
    // пред-выделение 4-16 GB заставляет ядро размечать страницы вперёд загрузки.
    //
    // Идентичный Prism'у дефолт: `-Xms512m` (если пользователь не указал свой Xms).
    if !settings
        .jvm_args
        .split_whitespace()
        .any(|a| a.starts_with("-Xms"))
    {
        // Минимум 512MB — если вдруг ram_mb меньше, берём Xmx.
        let xms = std::cmp::min(512, ram_mb);
        args.push(format!("-Xms{}M", xms));
    }

    // ПРИМЕЧАНИЕ: ранее здесь были `-XX:+AlwaysPreTouch`, `-XX:+ParallelRefProcEnabled`,
    // `-XX:+DisableExplicitGC`, `-XX:+UseStringDeduplication` — убраны, потому что:
    //   * AlwaysPreTouch → заставляет JVM коснуться всех Xmx-страниц на старте; при
    //     `-Xms512m` это 512 MB работы, которая раньше казалась +0.5 с, но при
    //     `-Xms==-Xmx=4 GB+` это +2-4 с реального I/O (потенциально с swap'ом).
    //   * ParallelRefProcEnabled, DisableExplicitGC, UseStringDeduplication — экономят
    //     0-50 мс на старте и дают микрооптимизацию в uptime. Prism 8 их НЕ передаёт
    //     по умолчанию и стартует быстрее нас. Доверяем JVM-дефолтам Java 21+/25.
    //
    // Если эти флаги окажутся полезны — пользователь может добавить их в
    // `settings.jvm_args` вручную. Лучше «чистый» быстрый старт по умолчанию.

    // PerfDisableSharedMem — оставляем: это безусловный микро-выигрыш (меньше sync
    // записей в /tmp/hsperfdata_<user>), без побочных эффектов на gameplay.
    if !settings
        .jvm_args
        .split_whitespace()
        .any(|a| a.contains("PerfDisableSharedMem"))
    {
        args.push("-XX:+PerfDisableSharedMem".into());
    }

    // G1 tuning Prism/Aikar-style. Применяется ТОЛЬКО для Java 17+ (Java 8/11 на legacy
    // Forge — свои дефолты). Пользовательские `-XX:G1…` не затираются.
    //
    // Зачем (замеры на modded 1.21+):
    //   * `G1NewSizePercent=20 G1ReservePercent=20` — young gen 20% от heap, +20% резерв.
    //     Без флагов JVM стартует с ~5% young → первый Full GC при загрузке модов
    //     (~500 classes/сек → stw-паузы по 100-300 мс каждая). С 20% — первый young
    //     GC откладывается до полной загрузки мира. На старте это −500-900 мс.
    //   * `MaxGCPauseMillis=50` — агрессивная цель (дефолт 200). На старте триггерит
    //     меньше mixed-collect'ов, TLAB-аллокация быстрее.
    //   * `G1HeapRegionSize=32M` — крупный regionsize (дефолт считается от Xmx).
    //     Для 4-16 GB heap JVM выбирает 4-16 MB региона, что на modded даёт 1000+ регионов
    //     и тратит время на `G1RegionHandles` при GC barrier'ах. 32 MB фиксированно =
    //     128-512 регионов = меньше метаданных, быстрее scan.
    //
    // Безопасность: все флаги — experimental, но Prism выставляет их 4+ года на миллионах
    // инсталляций. Для `-XX:+UnlockExperimentalVMOptions` не требуется (эти не experimental
    // в Java 17+; в комменте Prism их обёртка — на всякий случай).
    let mc_needs_java17 = vanilla_info
        .java_version
        .as_ref()
        .map(|j| j.major_version >= 17)
        .unwrap_or(true);
    // `legacy_forge_launchwrapper` определён выше (main_class.contains("launchwrapper")).
    // Полный `legacy_forge` с учётом FMLTweaker вычисляется ниже — для G1-тюнинга
    // лёгкой проверки достаточно: Forge ≤1.12 с LaunchWrapper идёт на Java 8 и
    // не поддерживает G1-experimental.
    if mc_needs_java17 && !legacy_forge_launchwrapper {
        // `G1NewSizePercent` и `G1ReservePercent` до Java 21 числятся experimental
        // (стали stable только в JDK 21, но в JDK 22-25 вновь периодически возвращают
        // в experimental при рефакторинге G1). HotSpot падает без `UnlockExperimentalVMOptions`:
        //   `VM option 'G1NewSizePercent' is experimental and must be enabled via -XX:+UnlockExperimentalVMOptions.`
        //   → код 1, игра даже не грузится.
        //
        // КРИТИЧНО: HotSpot требует, чтобы `-XX:+UnlockExperimentalVMOptions` стоял на
        // командной строке ДО `-XX:G1NewSizePercent=...`. Поэтому добавляем в `args`
        // ВСЕГДА (не смотрим на `settings.jvm_args`): пользовательские флаги append'ятся
        // в конец (строка ~2105), дедуп по JVM_UNLOCK_EXPERIMENTAL там уже есть. Если
        // условно пропустить — unlock окажется ПОСЛЕ G1 → JVM падает с
        // `The unlock option must precede 'G1NewSizePercent'`.
        if !args.iter().any(|a| a == JVM_UNLOCK_EXPERIMENTAL) {
            args.push(JVM_UNLOCK_EXPERIMENTAL.into());
        }
        for (probe, flag) in [
            ("G1NewSizePercent", "-XX:G1NewSizePercent=20"),
            ("G1ReservePercent", "-XX:G1ReservePercent=20"),
            ("MaxGCPauseMillis", "-XX:MaxGCPauseMillis=50"),
            ("G1HeapRegionSize", "-XX:G1HeapRegionSize=32M"),
        ] {
            if !settings.jvm_args.contains(probe) {
                args.push(flag.into());
            }
        }
    }

    // Log4j fast-init — отключение бесполезных на клиенте фич:
    //   * `formatMsgNoLookups=true` — Log4Shell-митигация; как побочный эффект отключает
    //     JNDI-lookup формат-парсер, что убирает reflection-скан на старте (−150-300 мс).
    //   * `log4j.skipJansi=true` — пропуск автодетекта ANSI-терминала на Linux (мы пишем в
    //     pipe, цвета всё равно никто не видит). −50-150 мс.
    //   * `log4j2.enable.threadlocals=false` не включаем — ломает асинхронные логгеры.
    if !settings.jvm_args.contains("log4j2.formatMsgNoLookups") {
        args.push("-Dlog4j2.formatMsgNoLookups=true".into());
    }
    if !settings.jvm_args.contains("log4j.skipJansi") {
        args.push("-Dlog4j.skipJansi=true".into());
    }

    // Netty reflection — на Java 17+ без флага Netty фолбэкает в AccessController и пишет
    // в stderr трассировку каждой попытки sun.misc.Unsafe. Флаг включает прямой reflection
    // (правомерно — мы добавляем --add-opens для java.base). −100-200 мс + чище лог.
    if !settings.jvm_args.contains("io.netty.tryReflectionSetAccessible") {
        args.push("-Dio.netty.tryReflectionSetAccessible=true".into());
    }

    // -Duser.language=en — Prism выставляет; некоторые моды завязаны на английские
    // строки для regex-матчинга (FML, Mixin), перевод системы на русский в Java
    // может ломать сопоставления. Не влияет на перевод самой игры.
    if !settings.jvm_args.contains("-Duser.language") {
        args.push("-Duser.language=en".into());
    }

    args.push(format!("-Djava.library.path={}", natives_dir_str));
    // Лаунчер забирает stdout/stderr в pipe — нет настоящего TTY; JLine иначе «Advanced terminal features…» и редкие сбои консольных путей Forge.
    args.push("-Djline.terminal.type=dumb".into());

    // Только один блок arguments.jvm: у Forge/Fabric профиль на диске уже содержит merge vanilla,
    // а родитель (1.20.1.json) дублирует тот же -cp — два -cp ломают запуск.
    for v_info in all_v_infos.iter().rev() {
        let Some(jvm_args) = v_info
            .arguments
            .as_ref()
            .and_then(|a| a.get("jvm"))
            .and_then(|a| a.as_array())
        else {
            continue;
        };
        if jvm_args.is_empty() {
            continue;
        }
        for v in jvm_args {
            if let Some(s) = v.as_str() {
                args.push(replace_vars(s.to_string()));
            } else if let Some(obj) = v.as_object() {
                let mut allow = true;
                if let Some(rules) = obj.get("rules").and_then(|r| r.as_array()) {
                    for rule in rules {
                        let action = rule
                            .get("action")
                            .and_then(|a| a.as_str())
                            .unwrap_or("allow");
                        let os_name = rule
                            .get("os")
                            .and_then(|os| os.get("name"))
                            .and_then(|n| n.as_str());
                        let os_match = match os_name {
                            Some("windows") => cfg!(target_os = "windows"),
                            Some("linux") => cfg!(target_os = "linux"),
                            Some("osx") => cfg!(target_os = "macos"),
                            _ => true,
                        };
                        if action == "allow" && !os_match {
                            allow = false;
                        }
                        if action == "disallow" && os_match {
                            allow = false;
                        }
                    }
                }
                if allow {
                    if let Some(val) = obj.get("value") {
                        if let Some(s) = val.as_str() {
                            args.push(replace_vars(s.to_string()));
                        } else if let Some(arr) = val.as_array() {
                            for item in arr {
                                if let Some(s) = item.as_str() {
                                    args.push(replace_vars(s.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
        break;
    }

    for part in settings.jvm_args.split_whitespace() {
        if part.is_empty() {
            continue;
        }
        let s = part.to_string();
        if s == JVM_UNLOCK_EXPERIMENTAL && args.iter().any(|a| a == JVM_UNLOCK_EXPERIMENTAL) {
            continue;
        }
        args.push(s);
    }

    if is_neoforge_profile_id(version_id) {
        force_neoforge_library_directory_jvm_arg(&mut args, &lib_dir_str);
    }

    #[cfg(target_os = "linux")]
    {
        if settings.linux_lwjgl_openal_libname {
            let path = settings.linux_openal_lib_path.trim();
            if !path.is_empty() && !jvm_args_has_openal_libname(&args) {
                args.push(format!("-Dorg.lwjgl.openal.libname={path}"));
            }
        }
    }

    if !args.contains(&"-cp".to_string())
        && !args.iter().any(|a| a.starts_with("-Djava.class.path"))
    {
        args.push("-cp".to_string());
        args.push(classpath_str.clone());
    }

    args.push(main_class.clone());

    // --- ОСОБАЯ ОБРАБОТКА FORGEWRAPPER ---
    if is_forge_wrapper {
        if forge_installer_path.is_empty() {
            log("КРИТИЧЕСКАЯ ОШИБКА: ForgeWrapper найден, но файл installer.jar не скачался.");
            return Err(Error::Custom(
                "Установщик Forge не найден! Нажмите 'Починить ядро' в настройках сборки.".into(),
            ));
        }

        let actual_installer_path = if let Some(ref ij) = resolved_installer_jar {
            match crate::core::game::install::patch_installer_remove_processor_outputs(ij) {
                Ok(patched) => {
                    log(&format!(
                        "ForgeWrapper: пропатченный installer (без outputs): {}",
                        patched.display()
                    ));
                    patched.to_string_lossy().replace('\\', "/")
                }
                Err(e) => {
                    log(&format!(
                        "ForgeWrapper: не удалось пропатчить installer, используем оригинал: {e}"
                    ));
                    forge_installer_path.clone()
                }
            }
        } else {
            forge_installer_path.clone()
        };

        let forge_tmp = data_dir.join("tmp").join("forge_install");
        let _ = std::fs::create_dir_all(&forge_tmp);
        let tmp_jvm = forge_tmp.to_string_lossy().replace('\\', "/");

        let idx = args
            .iter()
            .position(|a| a == &main_class)
            .unwrap_or(args.len());
        args.insert(idx, format!("-Djava.io.tmpdir={}", tmp_jvm));
        args.insert(idx + 1, "-Duser.language=en".into());
        args.insert(idx + 2, "-Duser.country=US".into());
        args.insert(
            idx + 3,
            format!("-Dforgewrapper.installer={}", actual_installer_path),
        );
        args.insert(
            idx + 4,
            format!("-Dforgewrapper.minecraft={}", client_jar_path),
        );
        args.insert(
            idx + 5,
            format!("-Dforgewrapper.librariesDir={}", lib_dir_str),
        );
        log("ForgeWrapper: java.io.tmpdir → tmp/forge_install, user.language=en (установщик Forge).");
    }

    // Игровые аргументы по цепи inheritsFrom (корень → лист): каждый уровень с непустым game **заменяет** накопленное.
    // Fabric/Quilt meta отдают полный список в листе. После promote Forge/NeoForge в листе часто только FML-флаги
    // без `--accessToken`/`--version` — без подмешивания корневого vanilla joptsimple падает (MissingRequiredOptionsException).
    let root_vanilla_game = extract_profile_game_args(&all_v_infos[0]);

    let mut game_args = Vec::new();
    for v_info in &all_v_infos {
        if let Some(legacy) = &v_info.minecraft_arguments {
            let t = legacy.trim();
            if !t.is_empty() {
                game_args.clear();
                game_args.extend(t.split_whitespace().map(String::from));
                continue;
            }
        }
        if let Some(g_args) = v_info
            .arguments
            .as_ref()
            .and_then(|a| a.get("game"))
            .and_then(|a| a.as_array())
        {
            let strings: Vec<String> = g_args
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if !strings.is_empty() {
                game_args = strings;
            }
            // Объекты с rules (демо, разрешение из features) опускаем — для запуска достаточно строковых полей vanilla.
        }
    }

    if !game_args_have_access_token_and_version(&game_args) && !root_vanilla_game.is_empty() {
        log(
            "У листа цепочки (Forge/NeoForge после promote) нет полных игровых аргументов клиента — \
             подмешиваем из корневого vanilla только недостающие ключи (без дублей), затем FML.",
        );
        let fill = vanilla_game_tokens_not_yet_in(&game_args, &root_vanilla_game);
        let mut merged = fill;
        merged.extend(game_args);
        game_args = merged;
    }

    let mut resolved_game: Vec<String> = game_args.into_iter().map(|a| replace_vars(a)).collect();
    repair_game_args_minecraft_version(&mut resolved_game, &mc_version_id);
    if is_neoforge_profile_id(version_id) {
        repair_neoforge_fml_program_args(&mut resolved_game, version_id, &mc_version_id, &data_dir);
        if !is_forge_wrapper {
            ensure_neoforge_patched_or_fallback_jars(
                &lib_dir,
                &data_dir,
                &resolved_game,
                version_id,
                &mc_version_id,
            )?;
        }
    }
    args.extend(resolved_game);

    // Прямой запуск на сервер (Quick Play). Формат: host:port (если порт не указан — :25565).
    // 1.20+: --quickPlayMultiplayer. До 1.20: --server (удалён в 23w14a).
    if let Some(ip) = server_ip {
        if !ip.is_empty() {
            let addr = if ip.contains(':') {
                ip.to_string()
            } else {
                format!("{}:25565", ip)
            };
            let use_qp = minecraft_supports_quick_play(jar_id);
            log(&format!(
                "Прямой вход на сервер: {} (версия {} — {})",
                addr,
                jar_id,
                if use_qp {
                    "quickPlayMultiplayer"
                } else {
                    "server"
                }
            ));
            if use_qp {
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
            if minecraft_supports_quick_play(jar_id) {
                args.push("--quickPlaySingleplayer".into());
                args.push(wn.to_string());
            }
        }
    }

    let legacy_forge = main_class.to_lowercase().contains("launchwrapper")
        || all_v_infos.iter().any(|v| {
            v.minecraft_arguments
                .as_deref()
                .map(|m| m.contains("FMLTweaker") || m.contains("fml.common.launcher"))
                .unwrap_or(false)
        });

    if legacy_forge {
        dedupe_tweak_class_options(&mut args);
        strip_fml_cascaded_tweak_classes_from_argv(&mut args);
        dedupe_tweak_class_options(&mut args);
    }

    if legacy_forge {
        if let Some(pos) = args.iter().position(|a| a == &main_class) {
            let tail = args.split_off(pos);
            let prefix = strip_jvm_flags_for_java8(args, &log);
            args = prefix;
            args.extend(tail);
        }
        const FML_IGNORE_MC_JAR_CERT: &str = "-Dfml.ignoreInvalidMinecraftCertificates=true";
        if !args.iter().any(|a| {
            a == FML_IGNORE_MC_JAR_CERT
                || a.starts_with("-Dfml.ignoreInvalidMinecraftCertificates=")
        }) {
            if let Some(i) = args.iter().position(|a| a == "-cp") {
                args.insert(i, FML_IGNORE_MC_JAR_CERT.into());
            } else if let Some(i) = args.iter().position(|a| a == &main_class) {
                args.insert(i, FML_IGNORE_MC_JAR_CERT.into());
            } else {
                args.insert(0, FML_IGNORE_MC_JAR_CERT.into());
            }
            log(
                "Forge LaunchWrapper: -Dfml.ignoreInvalidMinecraftCertificates=true (проверка ClientBrandRetriever иначе блокирует запуск с лаунчерным client jar).",
            );
        }
    }

    let mut required_java = vanilla_info
        .java_version
        .as_ref()
        .map(|j| j.major_version)
        .unwrap_or(8);
    if legacy_forge && required_java > 8 {
        log(&format!(
            "Профиль указывает Java {}, но старый Forge (LaunchWrapper/FML) требует Java 8. Переключаемся на Java 8.",
            required_java
        ));
        required_java = 8;
    }

    mark_phase("pre_java", &mut phase_t);
    let mut java_major_label = required_java;
    let mut java_path = if !settings.custom_java_path.is_empty() {
        settings.custom_java_path.clone()
    } else {
        log(&format!("▸ Подготовка Java {}...", required_java));
        match crate::core::java::ensure_java(&app, required_java).await {
            Ok(path) => path,
            Err(e) => {
                log(&format!("Ошибка при подготовке Java: {}", e));
                return Err(e);
            }
        }
    };
    mark_phase("ensure_java", &mut phase_t);

    if legacy_forge {
        match crate::core::java::detect_java_major(&java_path).await {
            Some(m) if m > 8 => {
                log(&format!(
                    "Выбранный java — это Java {}. LaunchWrapper (Forge 1.7–1.12) на 9+ падает (ClassCastException: URLClassLoader). Подставляем Java 8 из лаунчера.",
                    m
                ));
                java_major_label = 8;
                java_path = match crate::core::java::ensure_java(&app, 8).await {
                    Ok(path) => path,
                    Err(e) => {
                        log(&format!("Ошибка при подготовке Java 8: {}", e));
                        return Err(e);
                    }
                };
            }
            Some(m) => {
                java_major_label = m;
                if m != 8 {
                    log(&format!(
                        "Внимание: для LaunchWrapper рекомендуется Java 8, обнаружена Java {}.",
                        m
                    ));
                } else {
                    log("Проверка: Java 8 — подходит для LaunchWrapper.");
                }
            }
            None => {
                log(
                    "Не удалось выполнить `java -version` у выбранного бинарника; для старого Forge ставим Java 8 из лаунчера.",
                );
                java_major_label = 8;
                java_path = match crate::core::java::ensure_java(&app, 8).await {
                    Ok(path) => path,
                    Err(e) => {
                        log(&format!("Ошибка при подготовке Java 8: {}", e));
                        return Err(e);
                    }
                };
            }
        }
    } else if !settings.custom_java_path.is_empty() {
        if let Some(m) = crate::core::java::detect_java_major(&java_path).await {
            java_major_label = m;
        }
    }

    if let Some(p) = instance_override_java {
        java_path = p;
        log(&format!("Путь Java из настроек сборки: {}", java_path));
        if legacy_forge {
            match crate::core::java::detect_java_major(&java_path).await {
                Some(m) if m > 8 => {
                    log(&format!(
                        "Для LaunchWrapper нужна Java 8, в сборке указана {}. Подставляем JRE 8 из лаунчера.",
                        m
                    ));
                    java_major_label = 8;
                    java_path = match crate::core::java::ensure_java(&app, 8).await {
                        Ok(path) => path,
                        Err(e) => {
                            log(&format!("Ошибка при подготовке Java 8: {}", e));
                            return Err(e);
                        }
                    };
                }
                Some(m) => java_major_label = m,
                None => {}
            }
        } else if let Some(m) = crate::core::java::detect_java_major(&java_path).await {
            java_major_label = m;
        }
    }

    let (inst_display_name, inst_game_version, inst_loader, inst_loader_version, use_discrete_gpu) =
        if inst_json_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&inst_json_path) {
                if let Ok(conf) = serde_json::from_str::<crate::config::InstanceConfig>(&content) {
                    let gpu = conf
                        .settings
                        .as_ref()
                        .map(|s| s.use_discrete_gpu)
                        .unwrap_or(false);
                    (
                        conf.name,
                        conf.game_version,
                        conf.loader,
                        conf.loader_version,
                        gpu,
                    )
                } else {
                    (
                        instance_id.to_string(),
                        version_id.to_string(),
                        String::new(),
                        String::new(),
                        false,
                    )
                }
            } else {
                (
                    instance_id.to_string(),
                    version_id.to_string(),
                    String::new(),
                    String::new(),
                    false,
                )
            }
        } else {
            (
                instance_id.to_string(),
                version_id.to_string(),
                String::new(),
                String::new(),
                false,
            )
        };

    let mc_is_17 = vanilla_info
        .id
        .as_deref()
        .map(|id| id.starts_with("1.7."))
        .unwrap_or(false);
    if legacy_forge && mc_is_17 {
        warn_if_mixin_mods_need_unimixins_1710(&game_dir, &log);
    }

    #[cfg(target_os = "linux")]
    {
        let id_blob = format!("{} {} {}", jar_id, version_id, inst_loader).to_lowercase();
        let forge_like = inst_loader.eq_ignore_ascii_case("forge")
            || inst_loader.eq_ignore_ascii_case("neoforge")
            || id_blob.contains("forge")
            || id_blob.contains("neoforge")
            || is_forge_profile_id(version_id)
            || is_neoforge_profile_id(version_id)
            || main_class.contains("net.minecraftforge")
            || main_class.contains("net.neoforged");
        if forge_like && !legacy_forge && !is_forge_wrapper {
            let fml_toml = game_dir.join("config").join("fml.toml");
            match patch_fml_toml_early_window_control(&fml_toml, false) {
                Ok(()) => log(
                    "Linux + FML: config/fml.toml → earlyWindowControl=false (так FML отключает EARLYDISPLAY; только -Djvm недостаточно).",
                ),
                Err(e) => log(&format!("config/fml.toml (earlyWindowControl): {e}")),
            }

            let mc_major = jar_id
                .split('.')
                .next()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let mut extra_jvm: Vec<&'static str> = Vec::new();
            if mc_major >= 26
                && !args
                    .iter()
                    .any(|a| a.starts_with("-Dfml.earlyLoadingScreen="))
            {
                extra_jvm.push("-Dfml.earlyLoadingScreen=false");
            }
            if !args
                .iter()
                .any(|a| a.starts_with("-Dfml.earlyWindowControl="))
            {
                extra_jvm.push("-Dfml.earlyWindowControl=false");
            }
            if !extra_jvm.is_empty() {
                for flag in extra_jvm.into_iter().rev() {
                    if let Some(cp_idx) = args.iter().position(|a| a == "-cp") {
                        args.insert(cp_idx, flag.into());
                    } else if let Some(mc_idx) = args.iter().position(|a| a == &main_class) {
                        args.insert(mc_idx, flag.into());
                    } else {
                        args.insert(0, flag.into());
                    }
                }
                if mc_major >= 26 {
                    log(
                        "Linux + NeoForge/Minecraft 26+: -Dfml.earlyLoadingScreen=false и -Dfml.earlyWindowControl=false (дополнительно к fml.toml).",
                    );
                } else {
                    log(
                        "Linux + Forge/NeoForge: -Dfml.earlyWindowControl=false (дополнительно к fml.toml). \
                         Стек CommonLaunchHandler.runTarget — это Main.invoke; смотрите Caused by в логе / crash-reports.",
                    );
                }
            }
        }
    }

    dedupe_java_library_path_singleton(&mut args, &natives_dir_str, &main_class);
    mark_phase("java_and_args", &mut phase_t);

    log(&format!("▸ Java {}: {}", java_major_label, java_path));
    log(&format!("▸ Запуск ({} аргументов JVM + игра)...", args.len()));

    let wrapper_setting = settings.wrapper.trim();
    #[cfg(unix)]
    let use_sh_wrapper = !wrapper_setting.is_empty();
    #[cfg(not(unix))]
    let use_sh_wrapper = false;

    #[cfg(unix)]
    if use_sh_wrapper {
        let script = posix_sh_wrapper_script(wrapper_setting, &java_path, &args);
        log(&format!("Команда: /bin/sh -c {}", sh_single_quote(&script)));
    } else {
        log(&format!("Команда: {} {}", java_path, args.join(" ")));
    }
    #[cfg(not(unix))]
    {
        if !wrapper_setting.is_empty() {
            log("Wrapper из настроек не применён: поддерживается только POSIX (Linux/macOS, /bin/sh -c).");
        }
        log(&format!("Команда: {} {}", java_path, args.join(" ")));
    }

    let mut cmd = if use_sh_wrapper {
        let script = posix_sh_wrapper_script(wrapper_setting, &java_path, &args);
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(script);
        c
    } else {
        let mut c = Command::new(&java_path);
        c.args(&args);
        c
    };
    if is_forge_wrapper {
        let forge_tmp = data_dir.join("tmp").join("forge_install");
        let _ = std::fs::create_dir_all(&forge_tmp);
        cmd.current_dir(&data_dir);
        let tool_extra = if java_major_label <= 8 {
            "-Xss4m".to_string()
        } else {
            FORGE_WRAPPER_EXTRA_JVM.join(" ")
        };
        let java_tool_opts = match std::env::var("JAVA_TOOL_OPTIONS") {
            Ok(p) if !p.trim().is_empty() => format!("{p} {tool_extra}"),
            _ => tool_extra,
        };
        cmd.env("JAVA_TOOL_OPTIONS", &java_tool_opts);
        log(&format!(
            "ForgeWrapper: JAVA_TOOL_OPTIONS = {} (Java {})",
            if java_major_label <= 8 { "-Xss4m (без --add-opens для Java 8)" } else { "-Xss4m + --add-opens" },
            java_major_label
        ));
        #[cfg(unix)]
        {
            cmd.env("TMPDIR", forge_tmp.as_os_str());
            cmd.env("LC_ALL", "C");
            cmd.env("LANG", "C");
        }
        #[cfg(windows)]
        {
            let t = forge_tmp.as_os_str();
            cmd.env("TEMP", t);
            cmd.env("TMP", t);
        }
        log("ForgeWrapper: рабочий каталог процесса — корень данных лаунчера; TMPDIR=tmp/forge_install; LC_ALL=C.");
    } else {
        cmd.current_dir(&game_dir);
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // CREATE_NO_WINDOW (0x08000000): без отдельного консольного окна для Java на Windows
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000u32);

    #[cfg(target_os = "linux")]
    {
        cmd.env("GLFW_PLATFORM", "x11");
        cmd.env("GDK_BACKEND", "x11");
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            log("Linux + Wayland: GLFW_PLATFORM=x11 и GDK_BACKEND=x11 в процессе игры (не через JVM -D).");
        }
        if use_discrete_gpu {
            cmd.env("__NV_PRIME_RENDER_OFFLOAD", "1")
                .env("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
        } else {
            // Унаследованные PRIME/NVIDIA из сессии ломают GLX на интегрированной графике (GLXBadFBConfig).
            cmd.env_remove("__GLX_VENDOR_LIBRARY_NAME");
            cmd.env_remove("__NV_PRIME_RENDER_OFFLOAD");
        }
    }
    #[cfg(target_os = "windows")]
    if use_discrete_gpu {
        cmd.env("SHIM_MCCOMPAT", "0x800000001");
    }

    crate::core::fluxcore::process_guard::set_parent_death_signal(&mut cmd);

    let mut child = {
        let _ft_spawn = flux_trace.as_ref().map(|fx| {
            FluxTraceScope::enter(
                &app,
                instance_id,
                fx.correlation_id,
                HelixStageId::ProcessSpawn,
            )
        });
        match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                log(&format!("ОШИБКА ЗАПУСКА: {}", e));
                return Err(Error::Custom(format!("Не удалось запустить Java: {}", e)));
            }
        }
    };
    mark_phase("spawn", &mut phase_t);

    let root_pid = child.id();

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let console_log_path = game_dir.join("launcher-console.log");
    let console_file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&console_log_path)
    {
        Ok(f) => {
            log(&format!(
                "Полный вывод игры дублируется в {}",
                console_log_path.display()
            ));
            Some(Arc::new(Mutex::new(f)))
        }
        Err(e) => {
            log(&format!(
                "Не удалось открыть {} для лога: {}",
                console_log_path.display(),
                e
            ));
            None
        }
    };
    if let Some(ref cf) = console_file {
        if let Ok(mut w) = cf.lock() {
            let _ = writeln!(w);
            let _ = writeln!(w, "=== JentleMemes session ===");
            let _ = writeln!(w, "java: {}", java_path);
            let _ = writeln!(w, "=== stdout/stderr below ===");
        }
    }

    let child_handle: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(Some(child)));
    {
        let mut running = RUNNING.lock().unwrap();
        if let Some(old) = running.remove(instance_id) {
            let mut guard = old.child.lock().unwrap();
            if let Some(ref mut c) = *guard {
                eprintln!(
                    "[FluxCore] Killing stale JVM (pid={}) for instance {}",
                    old.root_pid, instance_id
                );
                let _ = c.kill();
            }
        }
        running.insert(
            instance_id.to_string(),
            RunningSession {
                child: child_handle.clone(),
                root_pid,
            },
        );
    }

    let session_start = std::time::Instant::now();
    if settings.discord_rich_presence {
        crate::core::discord_presence::set_playing_minecraft(
            &inst_display_name,
            &inst_game_version,
            &inst_loader,
            &inst_loader_version,
            server_ip,
        );
    }

    // Замер времени до первой строки stdout/stderr от JVM. Отделяет «наш prep до spawn»
    // от «JVM startup до первого log». Если prep <200 мс, а first_line >5 с — значит
    // тормозит JVM (classpath / natives / Fabric mixin scan), а не Rust-код.
    let jvm_spawned_at = std::time::Instant::now();
    let first_line_emitted: Arc<std::sync::atomic::AtomicBool> =
        Arc::new(std::sync::atomic::AtomicBool::new(false));
    let app_handle = app.clone();
    let inst_id = instance_id.to_string();
    let cf_out = console_file.clone();
    let first_out = first_line_emitted.clone();
    let jvm_t0_out = jvm_spawned_at;
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().flatten() {
            if !first_out.swap(true, std::sync::atomic::Ordering::Relaxed) {
                let dt = jvm_t0_out.elapsed().as_millis() as u64;
                let _ = app_handle.emit(
                    &format!("log_{}", inst_id),
                    format!("[JentleMemes] ⏱ JVM first_stdout Δ={dt}ms (от spawn до первой строки)"),
                );
                tracing::info!(target: "fluxcore::launch", event = "jvm_first_stdout", dt_ms = dt);
            }
            if let Some(ref f) = cf_out {
                if let Ok(mut g) = f.lock() {
                    let _ = writeln!(g, "{}", line);
                }
            }
            let _ = app_handle.emit(&format!("log_{}", inst_id), line);
        }
    });

    let app_handle2 = app.clone();
    let inst_id2 = instance_id.to_string();
    let cf_err = console_file.clone();
    let first_err = first_line_emitted.clone();
    let jvm_t0_err = jvm_spawned_at;
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().flatten() {
            if !first_err.swap(true, std::sync::atomic::Ordering::Relaxed) {
                let dt = jvm_t0_err.elapsed().as_millis() as u64;
                let _ = app_handle2.emit(
                    &format!("log_{}", inst_id2),
                    format!("[JentleMemes] ⏱ JVM first_stderr Δ={dt}ms (от spawn до первой строки)"),
                );
                tracing::info!(target: "fluxcore::launch", event = "jvm_first_stderr", dt_ms = dt);
            }
            if let Some(ref f) = cf_err {
                if let Ok(mut g) = f.lock() {
                    let _ = writeln!(g, "{}", line);
                }
            }
            let _ = app_handle2.emit(&format!("log_{}", inst_id2), line);
        }
    });

    // Поток ожидания завершения с поддержкой остановки
    let app_handle3 = app.clone();
    let inst_id3 = instance_id.to_string();
    thread::spawn(move || loop {
        let (done, exit_code) = {
            let mut guard = child_handle.lock().unwrap();
            if let Some(ref mut c) = *guard {
                match c.try_wait() {
                    Ok(Some(status)) => {
                        let code = status.code();
                        *guard = None;
                        (true, Some(code))
                    }
                    Ok(None) => (false, None),
                    Err(_) => {
                        *guard = None;
                        (true, None)
                    }
                }
            } else {
                (true, None)
            }
        };
        if let Some(code) = exit_code {
            let code_s = code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "сигнал ОС".into());
            let _ = app_handle3.emit(
                &format!("log_{}", inst_id3),
                format!(
                    "[JentleMemes] Процесс Java завершён, код выхода: {}",
                    code_s
                ),
            );
            if code == Some(0) {
                let _ = crate::core::game::install::promote_forge_wrapper_to_bootstrap(&inst_id3);
            }
        }
        if done {
            let secs = session_start.elapsed().as_secs();
            let _ = crate::core::instance::add_playtime(&inst_id3, secs);
            crate::core::discord_presence::clear();
            let _ = RUNNING.lock().unwrap().remove(&inst_id3);
            let _ = app_handle3.emit("exit_", &inst_id3);
            let _ = app_handle3.emit(&format!("exit_{}", inst_id3), ());
            break;
        }
        thread::sleep(std::time::Duration::from_millis(100));
    });

    // FluxCore v3: сохраняем snapshot запуска для следующего тёплого старта.
    // Если шаги резолва прошли без ошибок и процесс стартовал — кладём classpath/java/asset_index в LaunchCache.
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let snapshot = LaunchCache {
        version: 1,
        chain_hash: chain_hash_current.clone(),
        settings_hash: settings_hash_current.clone(),
        classpath: classpath.clone(),
        asset_index: asset_index.clone(),
        natives_extracted: Vec::new(),
        java_path: java_path.clone(),
        java_major: java_major_label,
        forge_client_jar: None,
        created_at: now_unix,
    };
    if let Err(e) = snapshot.save(&game_dir) {
        log(&format!(
            "FluxCore LaunchCache: не удалось сохранить snapshot запуска ({})",
            e
        ));
    }
    mark_phase("launch_cache_save", &mut phase_t);
    let total_ms = launch_wall_t0.elapsed().as_millis() as u64;
    tracing::info!(
        target: "fluxcore::launch",
        phase = "done",
        total_ms,
        cache_hit,
        classpath_len = classpath.len(),
        "launch::launch finished"
    );

    Ok("Игра запущена!".into())
}

pub fn stop_instance(instance_id: &str) -> Result<()> {
    let mut running = RUNNING.lock().unwrap();
    if let Some(sess) = running.remove(instance_id) {
        let mut guard = sess.child.lock().unwrap();
        if let Some(ref mut child) = *guard {
            let _ = child.kill();
            *guard = None;
        }
    }
    crate::core::discord_presence::clear();
    Ok(())
}
