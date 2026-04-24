use crate::error::{Error, Result};

const ALLOWED_HOSTS: &[&str] = &[
    "mojang.com",
    "minecraft.net",
    "minecraftservices.com",
    "launchermeta.mojang.com",
    "piston-meta.mojang.com",
    "piston-data.mojang.com",
    "libraries.minecraft.net",
    "resources.download.minecraft.net",
    "fabricmc.net",
    "meta.fabricmc.net",
    "maven.fabricmc.net",
    "quiltmc.org",
    "meta.quiltmc.org",
    "maven.quiltmc.org",
    "files.minecraftforge.net",
    "maven.minecraftforge.net",
    "maven.neoforged.net",
    "api.modrinth.com",
    "cdn.modrinth.com",
    "api.curseforge.com",
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
    "github.com",
    "raw.githubusercontent.com",
    "objects.githubusercontent.com",
    "adoptium.net",
    "api.adoptium.net",
    "download.eclipse.org",
    "ely.by",
    "authserver.ely.by",
    "mc-auth.com",
    "login.microsoftonline.com",
    "login.live.com",
    "user.auth.xboxlive.com",
    "xsts.auth.xboxlive.com",
];

const BLOCKED_HOSTS: &[&str] = &[
    "localhost",
    "127.0.0.1",
    "0.0.0.0",
    "[::1]",
    "169.254.",
    "10.",
    "192.168.",
    "172.16.",
    "172.17.",
    "172.18.",
    "172.19.",
    "172.20.",
    "172.21.",
    "172.22.",
    "172.23.",
    "172.24.",
    "172.25.",
    "172.26.",
    "172.27.",
    "172.28.",
    "172.29.",
    "172.30.",
    "172.31.",
];

pub fn validate_download_url(url: &str) -> Result<()> {
    let scheme_ok = url.starts_with("https://") || url.starts_with("http://");
    if !scheme_ok {
        return Err(Error::Custom(format!(
            "URL blocked (bad scheme): {url}"
        )));
    }

    let host = extract_host(url);

    for blocked in BLOCKED_HOSTS {
        if host == *blocked || host.starts_with(blocked) {
            return Err(Error::Custom(format!(
                "URL blocked (private/loopback): {url}"
            )));
        }
    }

    for allowed in ALLOWED_HOSTS {
        if host == *allowed || host.ends_with(&format!(".{allowed}")) {
            return Ok(());
        }
    }

    Err(Error::Custom(format!(
        "URL blocked (host not in allowlist): {url} (host: {host})"
    )))
}

pub fn validate_user_url_relaxed(url: &str) -> Result<()> {
    let scheme_ok = url.starts_with("https://") || url.starts_with("http://");
    if !scheme_ok {
        return Err(Error::Custom(format!(
            "URL blocked (bad scheme): {url}"
        )));
    }
    let host = extract_host(url);
    for blocked in BLOCKED_HOSTS {
        if host == *blocked || host.starts_with(blocked) {
            return Err(Error::Custom(format!(
                "URL blocked (private/loopback): {url}"
            )));
        }
    }
    Ok(())
}

fn extract_host(url: &str) -> String {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host_port = without_scheme.split('/').next().unwrap_or("");
    let host = host_port.split(':').next().unwrap_or("");
    host.to_ascii_lowercase()
}
