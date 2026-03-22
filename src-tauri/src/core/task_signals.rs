//! Счётчик «эпохи» для отмены фоновых задач (мета): любой видимый download_progress увеличивает эпоху.

use std::sync::atomic::{AtomicU64, Ordering};

static FOREGROUND_EPOCH: AtomicU64 = AtomicU64::new(0);

/// Вызывать перед emit каждого **не** silent `download_progress`.
#[inline]
pub fn bump_foreground_epoch() {
    FOREGROUND_EPOCH.fetch_add(1, Ordering::SeqCst);
}

#[inline]
pub fn current_foreground_epoch() -> u64 {
    FOREGROUND_EPOCH.load(Ordering::SeqCst)
}

/// Фоновая мета стартовала при `start_epoch`; если эпоха изменилась — прервать.
#[inline]
pub fn background_meta_cancelled(start_epoch: u64) -> bool {
    current_foreground_epoch() != start_epoch
}
