use crate::core::api::HTTP_CLIENT;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::path::{Path, PathBuf};

const NEWS_URL: &str = "https://jentlememes.ru/launcher/news.json";
const VERSION_URL: &str = "https://jentlememes.ru/launcher/version.json";

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub body: String,
    pub image: Option<String>,
    pub date: String,
    pub tag: Option<String>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlatformRelease {
    pub url: String,
    pub size: u64,
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VersionInfo {
    pub version: String,
    pub changelog: Option<String>,
    pub release_date: Option<String>,
    pub platforms: std::collections::HashMap<String, PlatformRelease>,
}

fn platform_key() -> &'static str {
    #[cfg(target_os = "windows")]
    { "windows" }
    #[cfg(target_os = "linux")]
    { "linux" }
    #[cfg(target_os = "macos")]
    { "macos" }
}

pub async fn fetch_news() -> Result<Vec<NewsItem>, String> {
    let resp = HTTP_CLIENT
        .get(NEWS_URL)
        .send().await.map_err(|e| format!("Network error: {e}"))?;
    let items: Vec<NewsItem> = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
    Ok(items)
}

pub async fn check_update() -> Result<Option<VersionInfo>, String> {
    let info: VersionInfo = HTTP_CLIENT
        .get(VERSION_URL)
        .send().await.map_err(|e| format!("Network error: {e}"))?
        .json().await.map_err(|e| format!("Parse error: {e}"))?;

    let current = parse_version(CURRENT_VERSION);
    let latest = parse_version(&info.version);

    if latest > current && info.platforms.contains_key(platform_key()) {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}

/// Last URL segment may lack an extension (some CDNs / redirects). Add a sane extension per OS.
fn update_filename_from_url(url: &str) -> String {
    let raw = url
        .rsplit('/')
        .next()
        .unwrap_or("jentlememes-launcher-update")
        .split('?')
        .next()
        .unwrap_or("jentlememes-launcher-update")
        .trim()
        .to_string();
    let base = if raw.is_empty() {
        "jentlememes-launcher-update".to_string()
    } else {
        raw
    };
    let p = Path::new(&base);
    if p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| !e.is_empty())
        .unwrap_or(false)
    {
        return base;
    }
    #[cfg(target_os = "windows")]
    {
        format!("{base}.exe")
    }
    #[cfg(target_os = "linux")]
    {
        let lower = url.to_lowercase();
        if lower.contains("appimage") {
            format!("{base}.AppImage")
        } else {
            format!("{base}.bin")
        }
    }
    #[cfg(target_os = "macos")]
    {
        format!("{base}.dmg")
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        base
    }
}

pub async fn download_update(release: &PlatformRelease) -> Result<PathBuf, String> {
    let update_dir = crate::config::get_data_dir().join("updates");
    std::fs::create_dir_all(&update_dir).map_err(|e| e.to_string())?;

    let filename = update_filename_from_url(&release.url);
    let dest = update_dir.join(filename);

    let resp = HTTP_CLIENT.get(&release.url)
        .send().await.map_err(|e| e.to_string())?;
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;

    tokio::fs::write(&dest, &bytes).await.map_err(|e| e.to_string())?;

    if let Some(expected_hash) = &release.sha256 {
        if !expected_hash.is_empty() {
            let actual = {
                let mut hasher = sha2::Sha256::new();
                sha2::Digest::update(&mut hasher, &bytes);
                format!("{:x}", sha2::Digest::finalize(hasher))
            };
            if actual != expected_hash.to_lowercase() {
                let _ = std::fs::remove_file(&dest);
                return Err(format!("SHA-256 mismatch: expected {expected_hash}, got {actual}"));
            }
        }
    }

    Ok(dest)
}

fn parse_version(s: &str) -> Vec<u32> {
    s.chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .split('.')
        .filter_map(|p| p.parse::<u32>().ok())
        .collect()
}
