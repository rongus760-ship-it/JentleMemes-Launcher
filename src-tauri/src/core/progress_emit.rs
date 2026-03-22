use tauri::{AppHandle, Emitter};

use crate::core::task_signals;
use crate::core::types::DownloadProgress;

/// Единая точка emit: несilent прогресс увеличивает эпоху (отмена фоновой меты).
pub fn emit_download_progress(app: &AppHandle, progress: DownloadProgress) {
    if !progress.silent {
        task_signals::bump_foreground_epoch();
    }
    let _ = app.emit("download_progress", progress);
}
