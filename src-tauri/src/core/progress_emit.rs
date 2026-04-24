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

/// Сбрасывает виджет загрузки после установки .mrpack (`silent` — без bump эпохи).
/// Фронт обрабатывает это в `download_progress` и обнуляет `progress` / `busyInstanceId`.
pub fn emit_install_progress_cleared(app: &AppHandle) {
    emit_download_progress(
        app,
        DownloadProgress {
            silent: true,
            ..Default::default()
        },
    );
}
