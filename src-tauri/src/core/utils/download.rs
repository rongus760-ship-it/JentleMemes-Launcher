use crate::core::api::http_client;
use crate::error::{Error, Result};
use bytes::Bytes;
use sha1::{Digest, Sha1};
use std::time::Duration;

pub async fn download_file(url: &str, expected_sha1: Option<&str>) -> Result<Bytes> {
    const MAX_RETRIES: u32 = 3;
    for attempt in 1..=MAX_RETRIES {
        let req = http_client()
            .get(url)
            .timeout(Duration::from_secs(600))
            .send()
            .await;

        let resp = match req {
            Ok(r) => {
                // Если файла физически нет, не мучаем сервер и сразу выходим
                if r.status() == reqwest::StatusCode::NOT_FOUND
                    || r.status() == reqwest::StatusCode::FORBIDDEN
                {
                    return Err(Error::Custom("File not found".into()));
                }
                r.error_for_status()
            }
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
                    })
                    .await?;

                    if actual != sha1 {
                        if attempt < MAX_RETRIES {
                            continue;
                        }
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

pub async fn download_with_progress(
    url: &str,
    dest: &std::path::Path,
    app: &tauri::AppHandle,
    task_name: &str,
) -> Result<()> {
    download_with_progress_sha1(url, dest, app, task_name, None).await
}

pub async fn download_with_progress_sha1(
    url: &str,
    dest: &std::path::Path,
    app: &tauri::AppHandle,
    task_name: &str,
    expected_sha1: Option<&str>,
) -> Result<()> {
    use crate::core::progress_emit::emit_download_progress;
    use crate::core::types::DownloadProgress;
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    emit_download_progress(
        app,
        DownloadProgress {
            task_name: task_name.to_string(),
            downloaded: 0,
            total: 100,
            instance_id: None,
            ..Default::default()
        },
    );

    let response = http_client()
        .get(url)
        .timeout(std::time::Duration::from_secs(600))
        .send()
        .await?;
    let total_bytes = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();

    if let Some(p) = dest.parent() {
        let _ = tokio::fs::create_dir_all(p).await;
    }

    let tmp_name = format!(
        "{}.tmp_{}",
        dest.file_name().unwrap_or_default().to_string_lossy(),
        &uuid::Uuid::new_v4().to_string().replace('-', "")[..8]
    );
    let tmp_path = dest.with_file_name(&tmp_name);

    let mut out_file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| Error::Custom(format!("Не удалось создать файл: {e}")))?;

    let mut downloaded_bytes: u64 = 0;
    let total_mb = ((total_bytes.max(1)) / 1024 / 1024) as usize;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            Error::Custom(format!("Ошибка скачивания: {e}"))
        })?;
        out_file
            .write_all(&chunk)
            .await
            .map_err(|e| {
                let _ = std::fs::remove_file(&tmp_path);
                Error::Custom(format!("Ошибка записи: {e}"))
            })?;
        downloaded_bytes += chunk.len() as u64;
        let dl_mb = (downloaded_bytes / 1024 / 1024) as usize;
        if dl_mb % 5 == 0 || downloaded_bytes >= total_bytes.max(1) {
            emit_download_progress(
                app,
                DownloadProgress {
                    task_name: task_name.to_string(),
                    downloaded: dl_mb,
                    total: total_mb.max(1),
                    instance_id: None,
                    ..Default::default()
                },
            );
        }
    }
    drop(out_file);

    if let Some(sha1) = expected_sha1 {
        let tp = tmp_path.clone();
        let expected = sha1.to_string();
        let ok = tokio::task::spawn_blocking(move || {
            crate::core::fluxcore::storage::verify_sha1(&tp, &expected)
        })
        .await??;
        if !ok {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(Error::Custom(format!(
                "SHA-1 mismatch after download: {url}"
            )));
        }
    }

    tokio::fs::rename(&tmp_path, dest).await.map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        Error::Custom(format!("Atomic rename failed: {e}"))
    })?;
    Ok(())
}
