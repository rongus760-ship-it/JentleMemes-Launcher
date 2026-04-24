use crate::core::fluxcore::v3::{run_game_launch, LaunchIntent};
use crate::error::Result;
use tauri::AppHandle;

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
) -> Result<String> {
    let intent = LaunchIntent {
        instance_id: instance_id.to_string(),
        version_id: version_id.to_string(),
        username: username.to_string(),
        uuid: uuid.to_string(),
        token: token.to_string(),
        acc_type: acc_type.to_string(),
        server_ip: server_ip.map(String::from),
        world_name: world_name.map(String::from),
    };
    run_game_launch(app, intent).await
}
