use uuid::Uuid;
use reqwest::Client;
use crate::config::{AccountInfo, DeviceCodeResponse};
use crate::error::{Result, Error};

pub async fn login_offline(username: &str) -> Result<AccountInfo> {
    let uuid = Uuid::new_v4().to_string();
    Ok(AccountInfo {
        id: format!("offline-{}", uuid),
        username: username.to_string(),
        uuid,
        token: "0".into(),
        acc_type: "offline".into(),
        active_skin_id: "".into(),
    })
}

pub async fn login_elyby(email: &str, password: &str) -> Result<AccountInfo> {
    let client = Client::new();
    let payload = serde_json::json!({
        "agent": { "name": "Minecraft", "version": 1 },
        "username": email,
        "password": password,
        "clientToken": Uuid::new_v4().to_string(),
        "requestUser": true
    });
    let res = client
        .post("https://authserver.ely.by/auth/authenticate")
        .json(&payload)
        .send()
        .await?;
    if !res.status().is_success() {
        return Err(Error::Custom("Неверный логин или пароль Ely.by".into()));
    }
    let data: serde_json::Value = res.json().await?;
    Ok(AccountInfo {
        id: format!("elyby-{}", data["selectedProfile"]["id"].as_str().unwrap_or("")),
        username: data["selectedProfile"]["name"].as_str().unwrap_or("").into(),
        uuid: data["selectedProfile"]["id"].as_str().unwrap_or("").into(),
        token: data["accessToken"].as_str().unwrap_or("").into(),
        acc_type: "elyby".into(),
        active_skin_id: "".into(),
    })
}

pub async fn ms_init_device_code() -> Result<DeviceCodeResponse> {
    let client = Client::new();
    let res = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[
            ("client_id", "747bf062-ab9c-4690-842d-a77d18d4cf82"),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await?;
    Ok(res.json().await?)
}

pub async fn ms_login_poll(device_code: &str, interval: u64) -> Result<AccountInfo> {
    let client = Client::new();
    let mut attempts = 0;
    let ms_token = loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        attempts += 1;
        if attempts > 60 {
            return Err(Error::Custom("Время ожидания истекло".into()));
        }
        let res = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", "747bf062-ab9c-4690-842d-a77d18d4cf82"),
                ("device_code", device_code),
            ])
            .send()
            .await?;
        let data: serde_json::Value = res.json().await?;
        if let Some(err) = data.get("error").and_then(|e| e.as_str()) {
            if err != "authorization_pending" {
                return Err(Error::Custom(err.into()));
            }
        } else if let Some(t) = data.get("access_token").and_then(|t| t.as_str()) {
            break t.to_string();
        }
    };

    let xbl_res: serde_json::Value = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .json()
        .await?;
    let xbl_token = xbl_res["Token"].as_str().unwrap();
    let uhs = xbl_res["DisplayClaims"]["xui"][0]["uhs"].as_str().unwrap();

    let xsts_res: serde_json::Value = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .json()
        .await?;
    let xsts_token = xsts_res["Token"].as_str().unwrap();

    let mc_res: serde_json::Value = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({
            "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token)
        }))
        .send()
        .await?
        .json()
        .await?;
    let mc_token = mc_res["access_token"].as_str().unwrap();

    let profile_res: serde_json::Value = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_token)
        .send()
        .await?
        .json()
        .await?;
    let uuid = profile_res["id"].as_str().unwrap();
    let username = profile_res["name"].as_str().unwrap();

    Ok(AccountInfo {
        id: format!("ms-{}", uuid),
        username: username.into(),
        uuid: uuid.into(),
        token: mc_token.into(),
        acc_type: "microsoft".into(),
        active_skin_id: "".into(),
    })
}