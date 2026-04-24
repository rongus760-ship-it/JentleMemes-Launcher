//! Скачивание authlib-injector (Ely.by, альтернативная аутентификация в клиенте).
use crate::error::{Error, Result};
use std::path::Path;

const AUTHLIB_INJECTOR_URL: &str =
    "https://github.com/yushijinhun/authlib-injector/releases/download/v1.2.7/authlib-injector-1.2.7.jar";

pub async fn ensure_authlib_injector_jar(data_dir: &Path) -> Result<std::path::PathBuf> {
    let dir = data_dir.join("bin");
    std::fs::create_dir_all(&dir).map_err(|e| Error::Custom(e.to_string()))?;
    let dest = dir.join("authlib-injector.jar");
    if dest.is_file() {
        if let Ok(m) = dest.metadata() {
            if m.len() > 10_000 {
                return Ok(dest);
            }
        }
    }
    let res = crate::core::api::http_client()
        .get(AUTHLIB_INJECTOR_URL)
        .send()
        .await
        .map_err(|e| Error::Custom(format!("authlib-injector: {e}")))?;
    if !res.status().is_success() {
        return Err(Error::Custom(format!(
            "authlib-injector: HTTP {}",
            res.status()
        )));
    }
    let bytes = res
        .bytes()
        .await
        .map_err(|e| Error::Custom(format!("authlib-injector: {e}")))?;
    std::fs::write(&dest, &bytes).map_err(|e| Error::Custom(e.to_string()))?;
    Ok(dest)
}
