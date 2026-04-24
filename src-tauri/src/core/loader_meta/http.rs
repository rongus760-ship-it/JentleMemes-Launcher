use crate::core::api::http_client;
use crate::error::{Error, Result};

use super::cache::{get_cached, put_cached};
use super::concurrency::fetch_semaphore;

pub async fn fetch_text_cached(cache_key: &str, url: &str) -> Result<String> {
    if let Some(c) = get_cached(cache_key) {
        return Ok(c);
    }
    let sem = fetch_semaphore();
    let _permit = sem
        .acquire()
        .await
        .map_err(|e| Error::Custom(format!("meta fetch semaphore: {e}")))?;
    let res = http_client().get(url).send().await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        let preview: String = text.chars().take(200).collect();
        return Err(Error::Custom(format!("HTTP {} — {}", status, preview)));
    }
    put_cached(cache_key, text.clone());
    Ok(text)
}
