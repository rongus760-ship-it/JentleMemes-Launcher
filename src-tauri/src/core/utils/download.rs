use bytes::Bytes;
use sha1::{Sha1, Digest};
use crate::error::{Result, Error};
use crate::core::api::HTTP_CLIENT;
use std::time::Duration;

pub async fn download_file(url: &str, expected_sha1: Option<&str>) -> Result<Bytes> {
    const MAX_RETRIES: u32 = 3;
    for attempt in 1..=MAX_RETRIES {
        let req = HTTP_CLIENT.get(url).send().await;
        
        let resp = match req {
            Ok(r) => {
                // Если файла физически нет, не мучаем сервер и сразу выходим
                if r.status() == reqwest::StatusCode::NOT_FOUND || r.status() == reqwest::StatusCode::FORBIDDEN {
                    return Err(Error::Custom("File not found".into()));
                }
                r.error_for_status()
            },
            Err(e) => Err(e),
        };

        match resp {
            Ok(r) => {
                let bytes = r.bytes().await?;
                if let Some(sha1) = expected_sha1 {
                    let actual = tokio::task::spawn_blocking({
                        let b = bytes.clone();
                        move || {
                            let mut hasher = Sha1::new();
                            hasher.update(&b);
                            format!("{:x}", hasher.finalize())
                        }
                    }).await?;

                    if actual != sha1 {
                        if attempt < MAX_RETRIES { continue; }
                        return Err(Error::Custom(format!("Неверный хэш для {}", url)));
                    }
                }
                return Ok(bytes);
            }
            Err(_) if attempt < MAX_RETRIES => {
                tokio::time::sleep(Duration::from_millis(300)).await;
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!()
}