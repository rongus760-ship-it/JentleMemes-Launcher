use crate::error::{Error, Result};
use tauri::AppHandle;

pub struct AuthResult {
    pub username: String,
    pub uuid: String,
    pub token: String,
    pub acc_type: String,
}

pub async fn resolve(
    _app: &AppHandle,
    username: &str,
    uuid: &str,
    token: &str,
    acc_type: &str,
) -> Result<AuthResult> {
    if acc_type == "microsoft" || acc_type.starts_with("ms-") {
        if crate::core::auth::ms_token_expired(token) {
            return Err(Error::Custom(
                "Токен Microsoft истёк. Подключитесь к сети и перезайдите в аккаунт, \
                 либо выберите оффлайн-аккаунт для игры без сети."
                    .into(),
            ));
        }
    }

    Ok(AuthResult {
        username: username.to_string(),
        uuid: uuid.to_string(),
        token: token.to_string(),
        acc_type: acc_type.to_string(),
    })
}
