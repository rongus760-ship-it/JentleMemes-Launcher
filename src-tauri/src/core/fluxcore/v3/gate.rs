use crate::error::{Error, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

static INSTANCE_SEMAPHORES: Lazy<Mutex<HashMap<String, Arc<Semaphore>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn try_acquire_exclusive_owned(
    instance_id: &str,
) -> Result<tokio::sync::OwnedSemaphorePermit> {
    let sem = {
        let mut locks = INSTANCE_SEMAPHORES.lock().await;
        locks
            .entry(instance_id.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone()
    };

    sem.try_acquire_owned()
        .map_err(|_| Error::Custom("Сборка уже запускается. Дождитесь завершения.".into()))
}
