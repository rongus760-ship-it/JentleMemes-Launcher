use crate::core::api::http_client;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::cmp::Ordering;
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
    {
        "windows"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
}

pub async fn fetch_news() -> Result<Vec<NewsItem>, String> {
    let resp = http_client()
        .get(NEWS_URL)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    let mut items: Vec<NewsItem> = resp.json().await.map_err(|e| format!("Parse error: {e}"))?;
    // Закреплённые сверху, дальше по дате (строка вида YYYY-MM-DD … сортируется лексикографически).
    items.sort_by(|a, b| {
        let pa = a.pinned.unwrap_or(false);
        let pb = b.pinned.unwrap_or(false);
        match pb.cmp(&pa) {
            Ordering::Equal => b.date.cmp(&a.date),
            o => o,
        }
    });
    Ok(items)
}

pub async fn check_update() -> Result<Option<VersionInfo>, String> {
    let info: VersionInfo = http_client()
        .get(VERSION_URL)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Parse error: {e}"))?;

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
    if p.extension()
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

    let resp = http_client()
        .get(&release.url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;

    tokio::fs::write(&dest, &bytes)
        .await
        .map_err(|e| e.to_string())?;

    match &release.sha256 {
        Some(expected_hash) if !expected_hash.is_empty() => {
            let actual = {
                let mut hasher = sha2::Sha256::new();
                sha2::Digest::update(&mut hasher, &bytes);
                format!("{:x}", sha2::Digest::finalize(hasher))
            };
            if actual != expected_hash.to_lowercase() {
                let _ = std::fs::remove_file(&dest);
                tracing::error!(
                    expected = %expected_hash,
                    actual = %actual,
                    url = %release.url,
                    "updater: SHA-256 mismatch, refusing to apply update"
                );
                return Err(format!(
                    "SHA-256 mismatch: expected {expected_hash}, got {actual}"
                ));
            }
            tracing::info!(
                url = %release.url,
                sha256 = %expected_hash,
                size = release.size,
                "updater: download verified"
            );
        }
        _ => {
            tracing::warn!(
                url = %release.url,
                size = release.size,
                "updater: release has no SHA-256 — download applied WITHOUT integrity check. \
                Add `sha256` field to launcher/version.json on the server."
            );
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

/// Windows: нельзя перезаписать запущенный `.exe`. После выхода лаунчера копируем скачанный файл на место текущего и снова запускаем.
#[cfg(target_os = "windows")]
pub fn schedule_replace_running_exe_with(update_path: &Path) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use std::os::windows::process::CommandExt;

    let target = std::env::current_exe().map_err(|e| e.to_string())?;
    let target_s = target.to_str().ok_or("Некорректный путь к exe лаунчера")?;
    let update_s = update_path
        .to_str()
        .ok_or("Некорректный путь к скачанному обновлению")?;

    fn ps_escape_single_quoted(s: &str) -> String {
        s.replace('\'', "''")
    }

    let ps = format!(
        "Start-Sleep -Seconds 2; Copy-Item -LiteralPath '{}' -Destination '{}' -Force; Start-Process -FilePath '{}'",
        ps_escape_single_quoted(update_s),
        ps_escape_single_quoted(target_s),
        ps_escape_single_quoted(target_s),
    );

    let utf16: Vec<u16> = ps.encode_utf16().collect();
    let mut bytes = Vec::with_capacity(utf16.len() * 2);
    for u in utf16 {
        bytes.push((u & 0xff) as u8);
        bytes.push((u >> 8) as u8);
    }
    let b64 = STANDARD.encode(&bytes);

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-EncodedCommand",
            &b64,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Не удалось запустить установщик обновления: {e}"))?;
    Ok(())
}
