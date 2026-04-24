//! Прямой Server List Ping (Java) + оформленный ответ для UI (MOTD, иконка, скины sample).
//! SRV `_minecraft._tcp`, если порт не указан явно в строке адреса.

mod address;
mod motd;
mod slp;
mod srv;
mod varint;

use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use serde_json::{json, Value};
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::error::Result;

use self::motd::description_to_clean_lines;
use self::slp::{fetch_status_json, parse_server_address_legacy};

const SKIN_FETCH_CAP: usize = 12;
const SKIN_FETCH_PARALLEL: usize = 4;

/// Ответ в форме, близкой к старому mcsrvstat + расширения для лаунчера.
pub async fn ping_java_server(addr: &str) -> Result<Value> {
    let (raw, target) = match fetch_status_json(addr).await {
        Ok(pair) => pair,
        Err(e) => {
            let (h, p) = parse_server_address_legacy(addr);
            return Ok(json!({
                "online": false,
                "ip": h,
                "port": p,
                "address": addr,
                "error": e.to_string(),
                "motd": { "clean": [], "chat": null },
                "players": { "online": 0, "max": 0, "list": [] },
                "srv": { "used": false },
            }));
        }
    };

    let description = raw.get("description").cloned().unwrap_or(Value::Null);
    let clean = description_to_clean_lines(&description);

    let players_online = raw
        .pointer("/players/online")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as i64;
    let players_max = raw
        .pointer("/players/max")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as i64;

    let sample = raw
        .pointer("/players/sample")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let sem = Arc::new(Semaphore::new(SKIN_FETCH_PARALLEL));
    let tasks: Vec<_> = sample
        .into_iter()
        .take(SKIN_FETCH_CAP)
        .map(|p| {
            let name = p
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string();
            let id = p
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let sem = sem.clone();
            async move {
                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(_) => {
                        return json!({
                            "name": name,
                            "id": id,
                            "uuid": id,
                            "skin_url": Value::Null,
                        });
                    }
                };
                let hex_len = id.chars().filter(|c| *c != '-').count();
                let skin = if hex_len >= 32 {
                    match timeout(
                        Duration::from_secs(4),
                        crate::core::auth::fetch_session_skin_url(&id, false),
                    )
                    .await
                    {
                        Ok(Ok(Some((u, _)))) => Some(u),
                        _ => None,
                    }
                } else {
                    None
                };
                json!({
                    "name": name,
                    "id": id,
                    "uuid": id,
                    "skin_url": skin,
                })
            }
        })
        .collect();

    let list: Vec<Value> = join_all(tasks).await;

    let favicon = raw.get("favicon").and_then(|v| v.as_str()).map(|s| {
        if s.starts_with("data:image") {
            s.to_string()
        } else {
            format!("data:image/png;base64,{s}")
        }
    });

    let version = raw.get("version").cloned().unwrap_or(Value::Null);
    let protocol = raw
        .pointer("/version/protocol")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let srv_info = if target.srv_used {
        json!({
            "used": true,
            "target": target.srv_target,
            "port": target.connect_port,
        })
    } else {
        json!({ "used": false })
    };

    Ok(json!({
        "online": true,
        "ip": target.logical_host,
        "port": target.connect_port,
        "address": addr,
        "connect_host": target.connect_host,
        "srv": srv_info,
        "version": version,
        "protocol": protocol,
        "players": {
            "online": players_online,
            "max": players_max,
            "list": list,
        },
        "motd": {
            "clean": clean,
            "chat": description,
        },
        "icon": favicon,
    }))
}
