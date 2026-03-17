use reqwest::Client;
use serde_json::Value;
use crate::error::{Result, Error};
use std::sync::LazyLock;
use std::time::Duration;

pub static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent("JentleMemesLauncher/1.0")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(5)
        .build()
        .unwrap()
});

pub async fn get_vanilla_version(version: &str) -> Result<Value> {
    let manifest: Value = HTTP_CLIENT
        .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
        .send().await?.json().await?;

    let versions = manifest["versions"].as_array()
        .ok_or_else(|| Error::Custom("Invalid version manifest".into()))?;

    let ver_entry = versions.iter()
        .find(|v| v["id"].as_str() == Some(version))
        .ok_or_else(|| Error::Custom(format!("Version {} not found in manifest", version)))?;

    let url = ver_entry["url"].as_str()
        .ok_or_else(|| Error::Custom("Missing version URL in manifest".into()))?;

    let res = HTTP_CLIENT.get(url).send().await?.json::<Value>().await?;
    Ok(res)
}

pub async fn get_loader_patch(loader: &str, loader_version: &str) -> Result<Value> {
    let uid = match loader {
        "fabric" => "net.fabricmc.fabric-loader",
        "quilt" => "org.quiltmc.quilt-loader",
        "forge" => "net.minecraftforge",
        "neoforge" => "net.neoforged",
        _ => return Err(Error::Custom("Неизвестный лоадер".into())),
    };
    
    let url = format!("https://meta.prismlauncher.org/v1/{}/{}.json", uid, loader_version);
    let res = HTTP_CLIENT.get(&url).send().await?.json::<Value>().await?;
    Ok(res)
}