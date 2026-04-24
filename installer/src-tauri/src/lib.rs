mod installer;

use installer::{InstallProgress};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

struct AppState {
    payload_path: Mutex<Option<PathBuf>>,
    is_uninstall: Mutex<bool>,
}

#[tauri::command]
fn get_mode(state: State<AppState>) -> String {
    if *state.is_uninstall.lock().unwrap() {
        "uninstall".to_string()
    } else {
        "install".to_string()
    }
}

#[tauri::command]
fn get_default_path() -> String {
    installer::default_install_path()
        .to_string_lossy()
        .to_string()
}

#[tauri::command]
fn check_disk_space(install_path: String, state: State<AppState>) -> installer::DiskInfo {
    let payload = state.payload_path.lock().unwrap();
    let payload_path = payload
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    installer::check_disk_space(&PathBuf::from(&install_path), payload_path)
}

#[tauri::command]
fn validate_path(path: String) -> Result<bool, String> {
    let p = PathBuf::from(&path);
    if let Some(parent) = p.parent() {
        if !parent.exists() {
            return Ok(false);
        }
    }
    Ok(true)
}

#[tauri::command]
fn run_install(
    install_path: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) {
    let install_dir = PathBuf::from(&install_path);
    let payload = state.payload_path.lock().unwrap().clone();

    std::thread::spawn(move || {
        let emit_progress = |percent: f64, status: &str| {
            let _ = app.emit("install-progress", InstallProgress {
                percent,
                status: status.to_string(),
            });
        };

        emit_progress(1.0, "Подготовка...");

        let payload_dir = match payload {
            Some(p) => {
                if p.extension().map(|e| e == "zip").unwrap_or(false) {
                    let extract_dir = p.parent().unwrap().join("_payload_extracted");
                    if let Err(e) = installer::extract_payload_zip(&p, &extract_dir) {
                        let _ = app.emit("install-error", e);
                        return;
                    }
                    extract_dir
                } else {
                    p
                }
            }
            None => {
                let _ = app.emit("install-error", "Путь к payload не задан. Установщик запущен без аргумента --payload.".to_string());
                return;
            }
        };

        emit_progress(5.0, "Копирование файлов...");

        if let Err(e) = installer::install_files(&payload_dir, &install_dir, |progress| {
            let _ = app.emit("install-progress", progress);
        }) {
            let _ = app.emit("install-error", e);
            return;
        }

        emit_progress(85.0, "Копирование установщика...");

        let current_exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                let _ = app.emit("install-error", format!("Не удалось определить путь к exe: {}", e));
                return;
            }
        };
        let dest_installer = install_dir.join("jentlememes-installer.exe");
        if current_exe != dest_installer {
            std::fs::copy(&current_exe, &dest_installer).ok();
        }

        emit_progress(90.0, "Создание ярлыков...");
        if let Err(e) = installer::create_shortcuts(&install_dir) {
            let _ = app.emit("install-error", e);
            return;
        }

        emit_progress(95.0, "Регистрация в системе...");
        if let Err(e) = installer::create_registry_entries(&install_dir) {
            let _ = app.emit("install-error", e);
            return;
        }
        if let Err(e) = installer::save_install_path(&install_dir) {
            let _ = app.emit("install-error", e);
            return;
        }

        emit_progress(100.0, "Установка завершена!");
    });
}

#[tauri::command]
fn run_uninstall(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let install_path = get_install_path_from_registry().unwrap_or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("C:\\"))
        });

        if let Err(e) = installer::uninstall(&install_path, |progress| {
            let _ = app.emit("install-progress", progress);
        }) {
            let _ = app.emit("install-error", e);
        }
    });
}

#[cfg(windows)]
fn get_install_path_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\JentlememesLauncher")
        .ok()?;
    let val: String = key.get_value("InstallLocation").ok()?;
    Some(PathBuf::from(val))
}

#[cfg(not(windows))]
fn get_install_path_from_registry() -> Option<PathBuf> {
    None
}

#[tauri::command]
fn close_window(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        w.close().ok();
    } else {
        app.exit(0);
    }
}

#[tauri::command]
fn minimize_window(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        w.minimize().ok();
    }
}

#[tauri::command]
fn drag_window(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        w.start_dragging().ok();
    }
}

#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn launch_app(install_path: String, args: Option<Vec<String>>) {
    let _ = &args;
    let exe = PathBuf::from(&install_path).join("jentlememes-launcher.exe");
    if exe.exists() {
        #[cfg(windows)]
        {
            use std::process::Command;
            let mut cmd = Command::new(&exe);
            if let Some(extra) = args {
                cmd.args(extra);
            }
            cmd.spawn().ok();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().collect();

    let is_uninstall = args.iter().any(|a| a == "--uninstall");
    let payload_path = args
        .iter()
        .position(|a| a == "--payload")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            payload_path: Mutex::new(payload_path),
            is_uninstall: Mutex::new(is_uninstall),
        })
        .invoke_handler(tauri::generate_handler![
            get_mode,
            get_default_path,
            check_disk_space,
            validate_path,
            run_install,
            run_uninstall,
            launch_app,
            close_window,
            minimize_window,
            drag_window,
            exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running installer");
}
