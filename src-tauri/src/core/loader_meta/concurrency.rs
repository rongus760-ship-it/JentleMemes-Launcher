//! Ограничение параллельных исходящих запросов по числу логических ядер (без «прожарки» слабых ПК).

use std::sync::Arc;

use once_cell::sync::Lazy;
use tokio::sync::Semaphore;

/// Логические ядра ОС (минимум 1).
pub fn logical_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1)
}

/// Параллельные загрузки меты: от 2 до 8, по умолчанию от ядер.
fn fetch_permits() -> usize {
    logical_cpu_count().clamp(2, 8)
}

static FETCH_SEM: Lazy<Arc<Semaphore>> = Lazy::new(|| Arc::new(Semaphore::new(fetch_permits())));

pub fn fetch_semaphore() -> Arc<Semaphore> {
    FETCH_SEM.clone()
}
