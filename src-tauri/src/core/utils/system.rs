use serde_json::Value;
use sysinfo::System;
use crate::error::Result;
use crate::core::api::HTTP_CLIENT;

pub fn get_system_ram() -> u64 {
    let mut sys = System::new_all();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}

pub async fn ping_server(ip: &str) -> Result<Value> {
    let url = format!("https://api.mcsrvstat.us/3/{}", ip);
    let res = HTTP_CLIENT.get(&url).send().await?.json::<Value>().await?;
    Ok(res)
}

/// Только релизные ID вида `1.21` / `1.21.5` (без pre/rc/snapshot/недельных).
fn is_release_style_mc_version(v: &str) -> bool {
    let mut parts = v.split('.');
    let a = parts.next();
    let b = parts.next();
    let c = parts.next();
    if parts.next().is_some() {
        return false;
    }
    let seg_ok =
        |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    match (a, b, c) {
        (Some(x), Some(y), None) => seg_ok(x) && seg_ok(y),
        (Some(x), Some(y), Some(z)) => seg_ok(x) && seg_ok(y) && seg_ok(z),
        _ => false,
    }
}

/// `include_snapshots: None` — как раньше: все версии для модпак-лоадеров, для vanilla только release.
pub async fn get_loader_versions(loader: &str, include_snapshots: Option<bool>) -> Result<Vec<String>> {
    let include_snapshots = include_snapshots.unwrap_or(true);

    if loader == "fabric" {
        let res = HTTP_CLIENT
            .get("https://meta.fabricmc.net/v2/versions/game")
            .send()
            .await?
            .json::<Value>()
            .await?;
        let arr = res.as_array().unwrap();
        let iter = arr.iter().filter(|v| {
            include_snapshots || v["stable"].as_bool() == Some(true)
        });
        return Ok(iter
            .filter_map(|v| v["version"].as_str().map(|s| s.to_string()))
            .collect());
    }
    if loader == "quilt" {
        let res = HTTP_CLIENT
            .get("https://meta.quiltmc.org/v3/versions/game")
            .send()
            .await?
            .json::<Value>()
            .await?;
        let arr = res.as_array().unwrap();
        let iter = arr.iter().filter(|v| {
            include_snapshots || v["stable"].as_bool() == Some(true)
        });
        return Ok(iter
            .filter_map(|v| v["version"].as_str().map(|s| s.to_string()))
            .collect());
    }
    if loader == "forge" || loader == "neoforge" {
        let uid = if loader == "forge" {
            "net.minecraftforge"
        } else {
            "net.neoforged"
        };
        let url = format!("https://meta.prismlauncher.org/v1/{}/", uid);
        let res = HTTP_CLIENT.get(&url).send().await?.json::<Value>().await?;
        let mut versions: Vec<String> = Vec::new();
        if let Some(arr) = res["versions"].as_array() {
            for item in arr {
                if let Some(reqs) = item["requires"].as_array() {
                    if let Some(mc) = reqs
                        .iter()
                        .find(|r| r["uid"].as_str() == Some("net.minecraft"))
                    {
                        if let Some(ver) = mc["equals"].as_str() {
                            if include_snapshots || is_release_style_mc_version(ver) {
                                if !versions.contains(&ver.to_string()) {
                                    versions.push(ver.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        versions.sort_by(|a, b| b.cmp(a));
        return Ok(versions);
    }
    let res = HTTP_CLIENT
        .get("https://api.modrinth.com/v2/tag/game_version")
        .send()
        .await?
        .json::<Vec<Value>>()
        .await?;
    let iter = res.into_iter().filter(|v| {
        include_snapshots || v["version_type"].as_str().unwrap_or("") == "release"
    });
    Ok(iter
        .filter_map(|v| v["version"].as_str().map(|s| s.to_string()))
        .collect())
}

pub async fn get_specific_loader_versions(loader: &str, game_version: &str) -> Result<Vec<String>> {
    if loader == "fabric" || loader == "quilt" {
        let domain = if loader == "fabric" { "meta.fabricmc.net/v2" } else { "meta.quiltmc.org/v3" };
        let url = format!("https://{}/versions/loader/{}", domain, game_version);
        let res = HTTP_CLIENT.get(&url).send().await?.json::<Value>().await?;
        return Ok(res.as_array().unwrap_or(&vec![]).iter().filter_map(|v| v["loader"]["version"].as_str().map(|s| s.to_string())).collect());
    }
    if loader == "forge" || loader == "neoforge" {
        let uid = if loader == "forge" { "net.minecraftforge" } else { "net.neoforged" };
        let url = format!("https://meta.prismlauncher.org/v1/{}/", uid);
        let res = HTTP_CLIENT.get(&url).send().await?.json::<Value>().await?;
        let mut v = Vec::new();
        if let Some(arr) = res["versions"].as_array() {
            for item in arr {
                if let Some(reqs) = item["requires"].as_array() {
                    if reqs.iter().any(|req| req["uid"].as_str() == Some("net.minecraft") && req["equals"].as_str() == Some(game_version)) {
                        if let Some(ver) = item["version"].as_str() { v.push(ver.to_string()); }
                    }
                }
            }
        }
        return Ok(v);
    }
    Ok(vec![])
}