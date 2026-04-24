use crate::config::{AccountInfo, DeviceCodeResponse};
use crate::core::api::http_client;
use crate::error::{Error, Result};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::time::Duration;
use uuid::Uuid;

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

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ElyYggdrasilProfile {
    id: String,
    name: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ElyAuthenticateBody {
    access_token: Option<String>,
    client_token: Option<String>,
    selected_profile: Option<ElyYggdrasilProfile>,
    available_profiles: Option<Vec<ElyYggdrasilProfile>>,
    error: Option<String>,
    #[serde(default)]
    error_message: Option<String>,
}

fn ensure_yggdrasil_client_token() -> Result<String> {
    let mut settings = crate::config::load_settings().unwrap_or_default();
    let t = settings.yggdrasil_client_token.trim().to_string();
    if !t.is_empty() {
        return Ok(t);
    }
    let new_t = Uuid::new_v4().to_string();
    settings.yggdrasil_client_token = new_t.clone();
    crate::config::save_settings(&settings)?;
    Ok(new_t)
}

pub async fn login_elyby(email: &str, password: &str) -> Result<AccountInfo> {
    let client_token = ensure_yggdrasil_client_token()?;
    let payload = serde_json::json!({
        "agent": { "name": "Minecraft", "version": 1 },
        "username": email,
        "password": password,
        "clientToken": client_token,
        "requestUser": true
    });
    let res = http_client()
        .post("https://authserver.ely.by/auth/authenticate")
        .json(&payload)
        .send()
        .await
        .map_err(|e| Error::Custom(format!("Ely.by: сеть ({e})")))?;
    let status = res.status();
    let txt = res
        .text()
        .await
        .map_err(|e| Error::Custom(format!("Ely.by: тело ответа ({e})")))?;
    let body: ElyAuthenticateBody = serde_json::from_str(&txt).map_err(|parse_err| {
        Error::Custom(format!(
            "Ely.by: неожиданный ответ (HTTP {}): {} — {}",
            status.as_u16(),
            parse_err,
            if txt.len() > 200 {
                format!("{}…", &txt[..200])
            } else {
                txt.clone()
            }
        ))
    })?;
    if let Some(err) = body.error.as_deref() {
        let msg = body.error_message.as_deref().unwrap_or(err);
        return Err(Error::Custom(format!("Ely.by: {msg}")));
    }
    if !status.is_success() {
        return Err(Error::Custom(format!(
            "Ely.by: HTTP {} — {}",
            status.as_u16(),
            body.error_message
                .as_deref()
                .unwrap_or("ошибка авторизации")
        )));
    }
    let access = body
        .access_token
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Error::Custom("Ely.by: нет accessToken в ответе".into()))?;
    let profile = body
        .selected_profile
        .or_else(|| body.available_profiles.and_then(|v| v.into_iter().next()))
        .ok_or_else(|| Error::Custom("Ely.by: нет профиля (selectedProfile)".into()))?;
    if profile.id.is_empty() || profile.name.is_empty() {
        return Err(Error::Custom("Ely.by: пустой id или имя профиля".into()));
    }
    if let Some(ref ct) = body.client_token {
        if !ct.is_empty() && ct != &client_token {
            let mut settings = crate::config::load_settings().unwrap_or_default();
            settings.yggdrasil_client_token = ct.clone();
            let _ = crate::config::save_settings(&settings);
        }
    }
    Ok(AccountInfo {
        id: format!("elyby-{}", profile.id),
        username: profile.name,
        uuid: profile.id,
        token: access,
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
async fn exchange_ms_access_for_minecraft(
    client: &Client,
    ms_access_token: &str,
) -> Result<(String, String, String)> {
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
        Error::Custom(format!(
            "Xbox Live: нет токена — {}",
            mojang_services_error_summary(&xbl_res)
        ))
    })?;
    let uhs = xbl_res["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or_else(|| Error::Custom("Xbox Live: нет uhs".into()))?;

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
        Error::Custom(format!(
            "XSTS: нет токена — {}",
            mojang_services_error_summary(&xsts_res)
        ))
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
    let uuid = profile_res["id"]
        .as_str()
        .ok_or_else(|| Error::Custom("Minecraft: нет id профиля".into()))?;
    let username = profile_res["name"]
        .as_str()
        .ok_or_else(|| Error::Custom("Minecraft: нет имени профиля".into()))?;

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
    let active_skin_id = previous
        .map(|p| p.active_skin_id.clone())
        .unwrap_or_default();
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
        let desc = data
            .get("error_description")
            .and_then(|d| d.as_str())
            .unwrap_or(err);
        return Err(Error::Custom(format!(
            "Microsoft OAuth: {} — {}",
            err, desc
        )));
    }
    let access = data["access_token"]
        .as_str()
        .ok_or_else(|| Error::Custom("Microsoft: нет access_token".into()))?;
    let new_refresh = data
        .get("refresh_token")
        .and_then(|r| r.as_str())
        .unwrap_or(refresh_token);
    Ok((access.into(), new_refresh.into()))
}

static AUTH_LOCK: once_cell::sync::Lazy<tokio::sync::Mutex<()>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(()));

pub fn ms_token_expired(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return true;
    }
    let payload = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(b) => b,
        Err(_) => match base64::engine::general_purpose::STANDARD.decode(parts[1]) {
            Ok(b) => b,
            Err(_) => return true,
        },
    };
    let json: serde_json::Value = match serde_json::from_slice(&payload) {
        Ok(v) => v,
        Err(_) => return true,
    };
    let exp = json.get("exp").and_then(|v| v.as_u64()).unwrap_or(0);
    if exp == 0 {
        return false;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now >= exp
}

pub async fn refresh_microsoft_account_on_startup(old: &AccountInfo) -> Result<AccountInfo> {
    if old.acc_type != "microsoft" {
        return Ok(old.clone());
    }
    let _guard = AUTH_LOCK.lock().await;

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
        uuid, username, mc_token, ms_refresh, None,
    ))
}

/// PKCE verifier (43–128 символов из unreserved).
pub fn oauth_pkce_verifier() -> String {
    let mut s = String::new();
    while s.len() < 64 {
        s.push_str(&Uuid::new_v4().to_string().replace('-', ""));
    }
    s.truncate(64);
    s
}

pub fn oauth_pkce_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(hash)
}

/// Redirect URI, зарегистрированный по умолчанию для public-клиентов Microsoft (не требует отдельной записи в Azure).
pub const MS_NATIVE_REDIRECT: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";

/// URL авторизации для встроенного браузера (loopback redirect).
pub fn ms_oauth_authorize_url(redirect_uri: &str, code_challenge: &str) -> String {
    format!(
        "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&prompt=select_account",
        MS_CLIENT_ID,
        urlencoding::encode(redirect_uri),
        urlencoding::encode("XboxLive.signin offline_access"),
        code_challenge
    )
}

/// Парсер authorization code / error из query-строки произвольного URL (redirect_uri не обязан быть loopback).
pub fn parse_ms_redirect_url(url: &str) -> Option<std::result::Result<String, String>> {
    let (_base, query) = url.split_once('?')?;
    // Отбрасываем фрагмент, если он есть
    let query = query.split('#').next().unwrap_or(query);
    let mut code: Option<String> = None;
    let mut err_kind: Option<String> = None;
    let mut err_desc: Option<String> = None;
    for pair in query.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next().unwrap_or("");
        let v = it.next().unwrap_or("");
        let decoded = urlencoding::decode(v).map(|c| c.into_owned()).ok();
        match k {
            "code" => code = decoded,
            "error" => err_kind = decoded,
            "error_description" => err_desc = decoded,
            _ => {}
        }
    }
    if let Some(c) = code {
        return Some(Ok(c));
    }
    if let Some(e) = err_kind {
        let msg = match err_desc {
            Some(d) if !d.is_empty() => format!("{e}: {d}"),
            _ => e,
        };
        return Some(Err(msg));
    }
    None
}

pub async fn ms_oauth_exchange_code_for_tokens(
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<(String, String)> {
    let client = Client::new();
    let res = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&[
            ("client_id", MS_CLIENT_ID),
            ("scope", "XboxLive.signin offline_access"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await?;
    let data: serde_json::Value = res.json().await?;
    if let Some(err) = data.get("error").and_then(|e| e.as_str()) {
        let desc = data
            .get("error_description")
            .and_then(|d| d.as_str())
            .unwrap_or(err);
        return Err(Error::Custom(format!(
            "Microsoft token: {} — {}",
            err, desc
        )));
    }
    let access = data["access_token"]
        .as_str()
        .ok_or_else(|| Error::Custom("Microsoft: нет access_token".into()))?;
    let refresh = data
        .get("refresh_token")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();
    Ok((access.to_string(), refresh))
}

pub async fn ms_account_from_ms_oauth_tokens(
    ms_access: &str,
    ms_refresh: &str,
) -> Result<AccountInfo> {
    let client = Client::new();
    let (uuid, username, mc_token) = exchange_ms_access_for_minecraft(&client, ms_access).await?;
    Ok(build_microsoft_account(
        uuid,
        username,
        mc_token,
        ms_refresh.to_string(),
        None,
    ))
}

/// Скин по нику: сначала Mojang, иначе профиль `skinsystem.ely.by/profile/{nick}` (игроки только на Ely).
pub async fn fetch_skin_texture_by_username_mojang_or_ely(
    username: &str,
) -> Result<Option<(String, bool)>> {
    match fetch_mojang_skin_by_username(username).await {
        Ok(Some(x)) => return Ok(Some(x)),
        Ok(None) | Err(_) => {}
    }
    fetch_ely_skinsystem_profile_skin(username).await
}

async fn fetch_ely_skinsystem_profile_skin(username: &str) -> Result<Option<(String, bool)>> {
    let name = username.trim();
    if name.is_empty() {
        return Ok(None);
    }
    let enc = urlencoding::encode(name);
    let url = format!("https://skinsystem.ely.by/profile/{enc}?unsigned=false");
    let res = http_client().get(&url).send().await?;
    if !res.status().is_success() || res.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }
    let txt = res.text().await?.trim().to_string();
    if txt.is_empty() {
        return Ok(None);
    }
    let j: serde_json::Value = match serde_json::from_str(&txt) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    Ok(skin_url_from_profile_json(&j))
}

/// Скачать PNG по URL текстуры скина (редиректы, лимит размера).
pub async fn download_skin_png_from_url(url: &str) -> Result<Vec<u8>> {
    let url = url.trim();
    if url.is_empty() {
        return Err(Error::Custom("Пустой URL скина".into()));
    }
    let res = http_client()
        .get(url)
        .send()
        .await
        .map_err(|e| Error::Custom(format!("Скачивание скина: {e}")))?;
    if !res.status().is_success() {
        return Err(Error::Custom(format!(
            "Скачивание скина: HTTP {}",
            res.status()
        )));
    }
    let bytes = res
        .bytes()
        .await
        .map_err(|e| Error::Custom(format!("Скачивание скина: {e}")))?;
    if bytes.len() > 1024 * 1024 {
        return Err(Error::Custom("Текстура скина больше 1 МБ".into()));
    }
    if bytes.len() < 64 {
        return Err(Error::Custom("Слишком маленький файл скина".into()));
    }
    if !bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Err(Error::Custom("По URL пришло не PNG".into()));
    }
    Ok(bytes.to_vec())
}

/// Скин **лицензии Mojang** по игровому нику (UUID через api.mojang.com → session).
/// Для Ely.by с тем же ником, что и у лицензии, совпадает с «официальной» текстурой.
pub async fn fetch_mojang_skin_by_username(username: &str) -> Result<Option<(String, bool)>> {
    let name = username.trim();
    if name.is_empty() {
        return Ok(None);
    }
    let enc = urlencoding::encode(name);
    let client = Client::new();
    let res = client
        .get(format!(
            "https://api.mojang.com/users/profiles/minecraft/{enc}"
        ))
        .send()
        .await?;
    if !res.status().is_success() || res.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }
    let txt = res.text().await?.trim().to_string();
    if txt.is_empty() {
        return Ok(None);
    }
    let j: serde_json::Value = match serde_json::from_str(&txt) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let id = match j.get("id").and_then(|x| x.as_str()) {
        Some(s) if s.chars().filter(|c| *c != '-').count() >= 32 => s,
        _ => return Ok(None),
    };
    fetch_session_skin_url(id, false).await
}

/// Сообщение, если кто-то вызывает загрузку скина Ely по игровому токену (публичного API нет).
pub const ELY_SKIN_CHANGE_WEB_ONLY: &str = "Ely.by не предоставляет публичный API смены скина по accessToken из лаунчера (маршрут HMCL на authserver не реализован; Chrly /api/skins — отдельный JWT). Смена скина: https://account.ely.by/profile";

pub const ELY_SKIN_RESET_WEB_ONLY: &str =
    "Сброс скина Ely.by только в личном кабинете: https://account.ely.by/profile";

/// Ely.by отдаёт `http://ely.by/storage/...`; в webview CSP разрешен только `https:` для картинок — грузим по HTTPS.
fn normalize_skin_texture_url_for_webview(url: String) -> String {
    const HTTP_ELY: &str = "http://ely.by";
    if url.starts_with(HTTP_ELY) {
        return format!("https://ely.by{}", &url[HTTP_ELY.len()..]);
    }
    const HTTP_SKIN: &str = "http://skinsystem.ely.by";
    if url.starts_with(HTTP_SKIN) {
        return format!("https://skinsystem.ely.by{}", &url[HTTP_SKIN.len()..]);
    }
    url
}

fn skin_url_from_profile_json(profile: &serde_json::Value) -> Option<(String, bool)> {
    let props = profile.get("properties")?.as_array()?;
    for p in props {
        if p.get("name").and_then(|x| x.as_str()) != Some("textures") {
            continue;
        }
        let b64 = p.get("value").and_then(|x| x.as_str())?;
        let decoded = STANDARD.decode(b64).ok()?;
        let tex: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
        let skin = tex.get("textures")?.get("SKIN")?;
        let url = normalize_skin_texture_url_for_webview(
            skin.get("url").and_then(|x| x.as_str())?.to_string(),
        );
        let slim = skin
            .get("metadata")
            .and_then(|m| m.get("model"))
            .and_then(|x| x.as_str())
            == Some("slim");
        return Some((url, slim));
    }
    None
}

/// Текстура скина с session-сервера (Mojang или Ely.by).
pub async fn fetch_session_skin_url(uuid: &str, ely: bool) -> Result<Option<(String, bool)>> {
    let u: String = uuid.chars().filter(|c| *c != '-').collect();
    if u.len() < 32 {
        return Ok(None);
    }
    let url = if ely {
        format!("https://authserver.ely.by/session/profile/{u}?unsigned=false")
    } else {
        format!("https://sessionserver.mojang.com/session/minecraft/profile/{u}?unsigned=false")
    };
    let client = Client::new();
    let res = client
        .get(&url)
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    let status = res.status();
    if !status.is_success() {
        return Ok(None);
    }
    if status == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }
    // Ely/Mojang иногда отвечают 200 с пустым телом или HTML — `res.json()` даёт decode error.
    let txt = res.text().await?.trim().to_string();
    if txt.is_empty() {
        return Ok(None);
    }
    let j: serde_json::Value = match serde_json::from_str(&txt) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    Ok(skin_url_from_profile_json(&j))
}

/// Загрузка PNG на официальный профиль Minecraft (Microsoft-аккаунт).
pub async fn upload_skin_minecraft_services(
    access_token: &str,
    png_bytes: &[u8],
    slim: bool,
) -> Result<()> {
    if png_bytes.len() > 1024 * 1024 {
        return Err(Error::Custom("Файл скина больше 1 МБ".into()));
    }
    if png_bytes.len() < 64 {
        return Err(Error::Custom("Слишком маленький файл скина".into()));
    }
    let client = Client::new();
    let variant = if slim { "slim" } else { "classic" };
    let part = reqwest::multipart::Part::bytes(png_bytes.to_vec())
        .file_name("skin.png")
        .mime_str("image/png")
        .map_err(|e| Error::Custom(e.to_string()))?;
    let form = reqwest::multipart::Form::new()
        .text("variant", variant.to_string())
        .part("file", part);
    let res = client
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .await?;
    if res.status().is_success() || res.status().as_u16() == 204 {
        return Ok(());
    }
    let txt = res.text().await.unwrap_or_default();
    let summary = serde_json::from_str::<serde_json::Value>(&txt)
        .map(|v| mojang_services_error_summary(&v))
        .unwrap_or_else(|_| txt);
    Err(Error::Custom(format!("Mojang API: {}", summary)))
}
