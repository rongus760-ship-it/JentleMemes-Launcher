use uuid::Uuid;
use reqwest::Client;
use crate::config::{AccountInfo, DeviceCodeResponse};
use crate::error::{Result, Error};

const MS_CLIENT_ID: &str = "747bf062-ab9c-4690-842d-a77d18d4cf82";

pub async fn login_offline(username: &str) -> Result<AccountInfo> {
    let uuid = Uuid::new_v4().to_string();
    Ok(AccountInfo {
        id: format!("offline-{}", uuid),
        username: username.to_string(),
        uuid,
        token: "0".into(),
        acc_type: "offline".into(),
        active_skin_id: "".into(),
        ms_refresh_token: "".into(),
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
        ms_refresh_token: "".into(),
    })
}

pub async fn ms_init_device_code() -> Result<DeviceCodeResponse> {
    let client = Client::new();
    let res = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[
            ("client_id", MS_CLIENT_ID),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await?;
    Ok(res.json().await?)
}

fn mojang_services_error_summary(v: &serde_json::Value) -> String {
    if let Some(msg) = v.get("errorMessage").and_then(|x| x.as_str()) {
        return msg.to_string();
    }
    if let Some(msg) = v.get("developerMessage").and_then(|x| x.as_str()) {
        return msg.to_string();
    }
    if let Some(msg) = v.get("error").and_then(|x| x.as_str()) {
        return msg.to_string();
    }
    if let Some(msg) = v.get("title").and_then(|x| x.as_str()) {
        return msg.to_string();
    }
    let s = v.to_string();
    if s.len() > 240 {
        format!("{}…", &s[..240])
    } else {
        s
    }
}

/// Обменивает access_token Microsoft на токен Minecraft + профиль.
async fn exchange_ms_access_for_minecraft(client: &Client, ms_access_token: &str) -> Result<(String, String, String)> {
    let xbl_res: serde_json::Value = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", ms_access_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .json()
        .await?;
    let xbl_token = xbl_res["Token"].as_str().ok_or_else(|| {
        Error::Custom(format!("Xbox Live: нет токена — {}", mojang_services_error_summary(&xbl_res)))
    })?;
    let uhs = xbl_res["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or_else(|| Error::Custom("Xbox Live: нет uhs".into()))?;

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
    let xsts_token = xsts_res["Token"].as_str().ok_or_else(|| {
        Error::Custom(format!("XSTS: нет токена — {}", mojang_services_error_summary(&xsts_res)))
    })?;

    let mc_http = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&serde_json::json!({
            "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token)
        }))
        .send()
        .await?;
    let mc_status = mc_http.status();
    let mc_res: serde_json::Value = mc_http.json().await?;

    if !mc_status.is_success() {
        return Err(Error::Custom(format!(
            "Minecraft login_with_xbox HTTP {}: {}",
            mc_status.as_u16(),
            mojang_services_error_summary(&mc_res)
        )));
    }

    let mc_token = mc_res["access_token"]
        .as_str()
        .or_else(|| mc_res.get("accessToken").and_then(|x| x.as_str()))
        .ok_or_else(|| {
            Error::Custom(format!(
                "Minecraft: нет access_token — {}",
                mojang_services_error_summary(&mc_res)
            ))
        })?;

    let profile_res: serde_json::Value = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_token)
        .send()
        .await?
        .json()
        .await?;
    let uuid = profile_res["id"].as_str().ok_or_else(|| Error::Custom("Minecraft: нет id профиля".into()))?;
    let username = profile_res["name"].as_str().ok_or_else(|| Error::Custom("Minecraft: нет имени профиля".into()))?;

    Ok((uuid.into(), username.into(), mc_token.into()))
}

fn build_microsoft_account(
    uuid: String,
    username: String,
    mc_token: String,
    ms_refresh_token: String,
    previous: Option<&AccountInfo>,
) -> AccountInfo {
    let id = previous
        .map(|p| p.id.clone())
        .unwrap_or_else(|| format!("ms-{}", uuid));
    let active_skin_id = previous.map(|p| p.active_skin_id.clone()).unwrap_or_default();
    AccountInfo {
        id,
        username,
        uuid,
        token: mc_token,
        acc_type: "microsoft".into(),
        active_skin_id,
        ms_refresh_token,
    }
}

/// Новый access_token и refresh_token (Microsoft может выдать новый refresh).
async fn ms_refresh_access_token(refresh_token: &str) -> Result<(String, String)> {
    let client = Client::new();
    let res = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", MS_CLIENT_ID),
            ("refresh_token", refresh_token),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await?;
    let data: serde_json::Value = res.json().await?;
    if let Some(err) = data.get("error").and_then(|e| e.as_str()) {
        let desc = data.get("error_description").and_then(|d| d.as_str()).unwrap_or(err);
        return Err(Error::Custom(format!("Microsoft OAuth: {} — {}", err, desc)));
    }
    let access = data["access_token"].as_str().ok_or_else(|| Error::Custom("Microsoft: нет access_token".into()))?;
    let new_refresh = data
        .get("refresh_token")
        .and_then(|r| r.as_str())
        .unwrap_or(refresh_token);
    Ok((access.into(), new_refresh.into()))
}

/// После каждого запуска: обновить Minecraft-токен по сохранённому refresh_token Microsoft.
/// Старые аккаунты без `ms_refresh_token` проверяются через profile API; при истечении — ошибка (нужен повторный вход).
pub async fn refresh_microsoft_account_on_startup(old: &AccountInfo) -> Result<AccountInfo> {
    if old.acc_type != "microsoft" {
        return Ok(old.clone());
    }
    let client = Client::new();

    if old.ms_refresh_token.is_empty() {
        let res = client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .bearer_auth(&old.token)
            .send()
            .await?;
        if res.status().is_success() {
            return Ok(old.clone());
        }
        return Err(Error::Custom(
            "Сессия Microsoft устарела — удалите аккаунт и войдите снова (нужен один раз для сохранения обновления токена).".into(),
        ));
    }

    let (ms_access, ms_refresh) = ms_refresh_access_token(&old.ms_refresh_token).await?;

    match exchange_ms_access_for_minecraft(&client, &ms_access).await {
        Ok((uuid, username, mc_token)) => Ok(build_microsoft_account(
            uuid,
            username,
            mc_token,
            ms_refresh,
            Some(old),
        )),
        Err(e) => {
            // Иногда Mojang не отдаёт новый токен при том же MS refresh, хотя старый MC JWT ещё валиден.
            // Сохраняем обновлённый ms_refresh_token, иначе следующий запуск потеряет возможность refresh.
            let res = client
                .get("https://api.minecraftservices.com/minecraft/profile")
                .bearer_auth(&old.token)
                .send()
                .await?;
            if res.status().is_success() {
                let mut updated = old.clone();
                updated.ms_refresh_token = ms_refresh;
                return Ok(updated);
            }
            Err(e)
        }
    }
}

pub async fn ms_login_poll(device_code: &str, interval: u64) -> Result<AccountInfo> {
    let client = Client::new();
    let mut attempts = 0;
    let (ms_token, ms_refresh) = loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        attempts += 1;
        if attempts > 60 {
            return Err(Error::Custom("Время ожидания истекло".into()));
        }
        let res = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", MS_CLIENT_ID),
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
            let refresh = data
                .get("refresh_token")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string();
            break (t.to_string(), refresh);
        }
    };

    let (uuid, username, mc_token) = exchange_ms_access_for_minecraft(&client, &ms_token).await?;
    Ok(build_microsoft_account(
        uuid,
        username,
        mc_token,
        ms_refresh,
        None,
    ))
}
