use serde_json::Value;
use sysinfo::System;

use crate::core::loader_meta;
use crate::error::Result;

pub fn get_system_ram() -> u64 {
    let mut sys = System::new_all();
    sys.refresh_memory();
    sys.total_memory() / 1024 / 1024
}

/// Прямой Server List Ping (TCP), MOTD и sample-игроки со скинами — без сторонних HTTP API статуса.
pub async fn ping_server(ip: &str) -> Result<Value> {
    crate::core::server_ping::ping_java_server(ip).await
}

/// `None` для флагов — как `false`: только стабильные релизы в смысле Modrinth (`release`).
pub async fn get_loader_versions(
    loader: &str,
    include_snapshots: Option<bool>,
    include_alpha_beta: Option<bool>,
) -> Result<Vec<String>> {
    loader_meta::loader_game_versions(loader, include_snapshots, include_alpha_beta).await
}

pub async fn get_specific_loader_versions(loader: &str, game_version: &str) -> Result<Vec<String>> {
    loader_meta::loader_versions_for_game(loader, game_version).await
}
