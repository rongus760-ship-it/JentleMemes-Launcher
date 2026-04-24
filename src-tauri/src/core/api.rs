use crate::core::loader_meta::{
    forge_profile_patch_from_installer, neoforge_profile_patch_from_installer,
};
use crate::error::{Error, Result};
use reqwest::Client;
use serde_json::Value;
use std::sync::Mutex;
use std::time::Duration;

static HTTP_CLIENT_CACHE: Mutex<Option<(String, Client)>> = Mutex::new(None);

fn build_http_client(proxy_trim: &str) -> Result<Client> {
    let mut b = Client::builder()
        .user_agent("JentleMemesLauncher/1.0")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(5);
    if !proxy_trim.is_empty() {
        let proxy = reqwest::Proxy::all(proxy_trim)
            .map_err(|e| Error::Custom(format!("Некорректный прокси «{proxy_trim}»: {e}")))?;
        b = b.proxy(proxy);
    }
    b.build().map_err(|e| e.into())
}

/// Клиент для HTTP-запросов лаунчера; учитывает `download_proxy_url` из настроек (кэш по строке прокси).
pub fn http_client() -> Client {
    let proxy = crate::config::load_settings()
        .map(|s| s.download_proxy_url.trim().to_string())
        .unwrap_or_default();
    let mut guard = HTTP_CLIENT_CACHE.lock().expect("http client cache mutex");
    if let Some((p, c)) = guard.as_ref() {
        if p == &proxy {
            return c.clone();
        }
    }
    let c = build_http_client(&proxy).unwrap_or_else(|_| {
        Client::builder()
            .user_agent("JentleMemesLauncher/1.0")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(120))
            .pool_max_idle_per_host(5)
            .build()
            .expect("reqwest default client")
    });
    *guard = Some((proxy, c.clone()));
    c
}

/// Сбросить кэш клиента (после сохранения настроек прокси из UI).
pub fn reset_http_client_cache() {
    if let Ok(mut g) = HTTP_CLIENT_CACHE.lock() {
        *g = None;
    }
}

/// Текст ответа → JSON; понятная ошибка вместо «expected value at line 1 column 1».
pub async fn json_from_response(res: reqwest::Response, context: &str) -> Result<Value> {
    let status = res.status();
    let url = res.url().clone();
    let text = res.text().await?;
    let t = text.trim();
    if !status.is_success() {
        let preview: String = t.chars().take(200).collect();
        return Err(Error::Custom(format!(
            "{}: HTTP {} — {}",
            context, status, preview
        )));
    }
    if t.is_empty() {
        return Err(Error::Custom(format!(
            "{}: пустой ответ ({})",
            context, url
        )));
    }
    serde_json::from_str(t).map_err(|e| {
        let preview: String = t.chars().take(160).collect();
        Error::Custom(format!(
            "{}: невалидный JSON ({}) — начало ответа: {:?}",
            context, e, preview
        ))
    })
}

pub async fn get_vanilla_version(version: &str) -> Result<Value> {
    let manifest_res = http_client()
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send()
        .await?;
    let manifest = json_from_response(manifest_res, "Minecraft version manifest").await?;

    let versions = manifest["versions"]
        .as_array()
        .ok_or_else(|| Error::Custom("Invalid version manifest".into()))?;

    let ver_entry = versions
        .iter()
        .find(|v| v["id"].as_str() == Some(version))
        .ok_or_else(|| Error::Custom(format!("Version {} not found in manifest", version)))?;

    let url = ver_entry["url"]
        .as_str()
        .ok_or_else(|| Error::Custom("Missing version URL in manifest".into()))?;

    let ver_res = http_client().get(url).send().await?;
    json_from_response(ver_res, &format!("Minecraft {}", version)).await
}

/// Версия лоадера из UI/JSON: без пробелов и управляющих символов.
pub fn normalize_loader_version(loader_version: &str) -> String {
    loader_version
        .trim()
        .trim_matches(|c: char| c.is_control())
        .to_string()
}

/// Патч профиля для merge с vanilla. Fabric/Quilt — официальный Meta profile JSON; Forge/NeoForge — официальный `*-installer.jar` с Maven (version.json + install_profile).
pub async fn get_loader_patch(
    game_version: &str,
    loader: &str,
    loader_version: &str,
) -> Result<Value> {
    let alpha = crate::config::load_settings()
        .map(|s| s.enable_alpha_loaders)
        .unwrap_or(false);
    if matches!(loader, "liteloader" | "modloader") && !alpha {
        return Err(Error::Custom(
            "Включите «Альфа: LiteLoader и ModLoader» в расширенных настройках лаунчера.".into(),
        ));
    }
    let lv = normalize_loader_version(loader_version);
    if lv.is_empty() {
        return Err(Error::Custom(
            "Версия загрузчика пустая — выберите версию Fabric/Quilt/Forge в настройках сборки."
                .into(),
        ));
    }
    if lv.chars().any(|c| c.is_whitespace()) {
        return Err(Error::Custom(format!(
            "Некорректная версия загрузчика (есть пробелы): {:?}",
            lv
        )));
    }

    let enc_lv = urlencoding::encode(&lv);
    let gv = game_version.trim();
    if gv.is_empty() && (loader == "fabric" || loader == "quilt") {
        return Err(Error::Custom(
            "Не указана версия Minecraft для профиля Fabric/Quilt.".into(),
        ));
    }
    let enc_mc = urlencoding::encode(gv);

    let url = match loader {
        "fabric" => {
            format!("https://meta.fabricmc.net/v2/versions/loader/{enc_mc}/{enc_lv}/profile/json")
        }
        "quilt" => {
            format!("https://meta.quiltmc.org/v3/versions/loader/{enc_mc}/{enc_lv}/profile/json")
        }
        "forge" => {
            return forge_profile_patch_from_installer(gv, &lv).await;
        }
        "neoforge" => {
            return neoforge_profile_patch_from_installer(gv, &lv).await;
        }
        "liteloader" => {
            return crate::core::loader_meta::liteloader::patch_for_loader_version(&lv);
        }
        "modloader" => {
            if !crate::core::loader_meta::modloader_alpha::is_known_build(gv, &lv) {
                return Err(Error::Custom(format!(
                    "ModLoader: версия «{lv}» не описана в loader_meta/data/modloader.json для Minecraft {gv}."
                )));
            }
            return Ok(serde_json::json!({}));
        }
        _ => return Err(Error::Custom("Неизвестный лоадер".into())),
    };

    let res = http_client().get(&url).send().await?;
    json_from_response(res, &format!("Loader patch ({loader} {lv} for MC {gv})")).await
}
