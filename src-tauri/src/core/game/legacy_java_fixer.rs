use std::fs;
use std::io::Write;
use std::path::Path;

use crate::core::game::version_layout::looks_like_minecraft_game_version;

const EMBEDDED_JAR: &[u8] = include_bytes!("../../../resources/legacy-java-fixer-1.0.jar");
const MATERIAL_NAME: &str = "LegacyJavaFixer-1.0.jar";

fn parse_mc_tuple(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it
        .next()
        .and_then(|x| x.parse().ok())
        .unwrap_or(0u32);
    Some((a, b, c))
}

pub fn needs_legacy_java_fixer(mc_raw: &str) -> bool {
    let base = mc_raw.split('-').next().unwrap_or(mc_raw).trim();
    if !looks_like_minecraft_game_version(base) {
        return false;
    }
    let Some(v) = parse_mc_tuple(base) else {
        return false;
    };
    v >= (1, 6, 2) && v <= (1, 7, 2)
}

pub fn sync_to_instance_mods(game_dir: &Path) -> std::io::Result<()> {
    let mods = game_dir.join("mods");
    fs::create_dir_all(&mods)?;
    let out = mods.join(MATERIAL_NAME);
    let want = EMBEDDED_JAR.len() as u64;
    let mut ok = false;
    if let Ok(meta) = fs::metadata(&out) {
        if meta.is_file() && meta.len() == want {
            ok = true;
        }
    }
    if !ok {
        let tmp = out.with_extension("jar.tmp");
        let mut f = fs::File::create(&tmp)?;
        f.write_all(EMBEDDED_JAR)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, &out)?;
    }
    Ok(())
}
