use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize)]
pub struct InstallProgress {
    pub percent: f64,
    pub status: String,
}

#[derive(Clone, Serialize)]
pub struct DiskInfo {
    pub available_gb: f64,
    pub required_gb: f64,
    pub enough: bool,
}

pub fn default_install_path() -> PathBuf {
    if cfg!(windows) {
        let base = std::env::var("LOCALAPPDATA")
            .or_else(|_| std::env::var("APPDATA"))
            .unwrap_or_else(|_| "C:\\Program Files".to_string());
        PathBuf::from(base).join("JentleMemes Launcher")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".local/share/jentlememes-launcher")
    }
}

pub fn check_disk_space(install_path: &Path, payload_path: &Path) -> DiskInfo {
    let required_bytes = get_dir_size(payload_path).unwrap_or(100 * 1024 * 1024); // fallback 100MB
    let required_gb = required_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    let available_bytes = get_available_space(install_path);
    let available_gb = available_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    DiskInfo {
        available_gb,
        required_gb,
        enough: available_bytes > required_bytes,
    }
}

fn get_dir_size(path: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    if path.is_file() {
        return Ok(fs::metadata(path)?.len());
    }
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                total += get_dir_size(&entry.path())?;
            } else {
                total += meta.len();
            }
        }
    }
    Ok(total)
}

#[cfg(windows)]
fn get_available_space(path: &Path) -> u64 {
    use std::os::windows::ffi::OsStrExt;
    let root = path
        .ancestors()
        .last()
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let drive = if root.len() >= 2 { &root[..3] } else { "C:\\" };
    let wide: Vec<u16> = std::ffi::OsStr::new(drive)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free_bytes: u64 = 0;
    unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_bytes as *mut u64,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
    }
    free_bytes
}

#[cfg(not(windows))]
fn get_available_space(_path: &Path) -> u64 {
    10 * 1024 * 1024 * 1024 // mock 10 GB for dev on Linux
}

pub fn install_files(
    payload_dir: &Path,
    install_path: &Path,
    progress_cb: impl Fn(InstallProgress),
) -> Result<(), String> {
    progress_cb(InstallProgress {
        percent: 0.0,
        status: "Подготовка...".to_string(),
    });

    fs::create_dir_all(install_path).map_err(|e| format!("Не удалось создать папку: {}", e))?;

    let files = collect_files(payload_dir).map_err(|e| format!("Ошибка чтения: {}", e))?;
    let total = files.len();

    for (i, src_path) in files.iter().enumerate() {
        let rel = src_path
            .strip_prefix(payload_dir)
            .map_err(|e| e.to_string())?;
        let dest = install_path.join(rel);

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir: {}", e))?;
        }

        fs::copy(src_path, &dest)
            .map_err(|e| format!("Копирование {}: {}", rel.display(), e))?;

        let pct = ((i + 1) as f64 / total as f64) * 80.0;
        progress_cb(InstallProgress {
            percent: pct,
            status: format!("Копирование: {}", rel.display()),
        });
    }

    Ok(())
}

fn collect_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                result.extend(collect_files(&path)?);
            } else {
                result.push(path);
            }
        }
    }
    Ok(result)
}

pub fn extract_payload_zip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("Не удалось открыть архив: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Невалидный архив: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Ошибка архива: {}", e))?;
        let out_path = dest.join(file.mangled_name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&out_path).ok();
        } else {
            if let Some(p) = out_path.parent() {
                fs::create_dir_all(p).ok();
            }
            let mut out = fs::File::create(&out_path)
                .map_err(|e| format!("Создание {}: {}", out_path.display(), e))?;
            std::io::copy(&mut file, &mut out)
                .map_err(|e| format!("Запись {}: {}", out_path.display(), e))?;
        }
    }

    Ok(())
}

#[cfg(windows)]
pub fn create_registry_entries(install_path: &Path) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let uninstall_key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\JentlememesLauncher";

    let (key, _) = hkcu
        .create_subkey(uninstall_key)
        .map_err(|e| format!("Реестр: {}", e))?;

    let exe_path = install_path.join("jentlememes-launcher.exe");
    let uninstall_exe = install_path.join("jentlememes-installer.exe");

    key.set_value("DisplayName", &"JentleMemes Launcher")
        .map_err(|e| format!("Реестр DisplayName: {}", e))?;
    key.set_value("UninstallString", &format!("\"{}\" --uninstall", uninstall_exe.display()))
        .map_err(|e| format!("Реестр UninstallString: {}", e))?;
    key.set_value("InstallLocation", &install_path.to_string_lossy().to_string())
        .map_err(|e| format!("Реестр InstallLocation: {}", e))?;
    key.set_value("DisplayIcon", &exe_path.to_string_lossy().to_string())
        .map_err(|e| format!("Реестр DisplayIcon: {}", e))?;
    key.set_value("Publisher", &"JentleProject")
        .map_err(|e| format!("Реестр Publisher: {}", e))?;
    key.set_value("DisplayVersion", &"0.1.0")
        .map_err(|e| format!("Реестр DisplayVersion: {}", e))?;
    key.set_value("NoModify", &1u32)
        .map_err(|e| format!("Реестр NoModify: {}", e))?;
    key.set_value("NoRepair", &1u32)
        .map_err(|e| format!("Реестр NoRepair: {}", e))?;

    Ok(())
}

#[cfg(not(windows))]
pub fn create_registry_entries(_install_path: &Path) -> Result<(), String> {
    Ok(()) // no-op on Linux (dev mode)
}

#[cfg(windows)]
pub fn create_shortcuts(install_path: &Path) -> Result<(), String> {
    use mslnk::ShellLink;

    let exe_path = install_path.join("jentlememes-launcher.exe");
    let exe_str = exe_path.to_string_lossy().to_string();

    // Desktop shortcut
    if let Some(desktop) = dirs::desktop_dir() {
        let lnk_path = desktop.join("JentleMemes Launcher.lnk");
        let sl = ShellLink::new(&exe_str).map_err(|e| format!("Ярлык: {}", e))?;
        sl.create_lnk(&lnk_path)
            .map_err(|e| format!("Ярлык на рабочий стол: {}", e))?;
    }

    // Start menu shortcut
    if let Some(start_menu) = dirs::data_dir() {
        let sm_dir = start_menu
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("JentleMemes Launcher");
        fs::create_dir_all(&sm_dir).ok();
        let lnk_path = sm_dir.join("JentleMemes Launcher.lnk");
        let sl = ShellLink::new(&exe_str).map_err(|e| format!("Ярлык: {}", e))?;
        sl.create_lnk(&lnk_path)
            .map_err(|e| format!("Ярлык в меню Пуск: {}", e))?;
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn create_shortcuts(_install_path: &Path) -> Result<(), String> {
    Ok(())
}

pub fn save_install_path(install_path: &Path) -> Result<(), String> {
    let meta_dir = install_path.join(".installer_meta");
    fs::create_dir_all(&meta_dir).map_err(|e| e.to_string())?;
    let meta_path = meta_dir.join("install_info.json");
    let info = serde_json::json!({
        "install_path": install_path.to_string_lossy(),
        "version": "1.0.5-beta.1",
    });
    fs::write(&meta_path, serde_json::to_string_pretty(&info).unwrap())
        .map_err(|e| e.to_string())?;
    Ok(())
}

// --- Uninstall ---

pub fn uninstall(install_path: &Path, progress_cb: impl Fn(InstallProgress)) -> Result<(), String> {
    progress_cb(InstallProgress {
        percent: 0.0,
        status: "Удаление файлов...".to_string(),
    });

    remove_shortcuts();
    progress_cb(InstallProgress {
        percent: 30.0,
        status: "Удаление ярлыков...".to_string(),
    });

    remove_registry_entries();
    progress_cb(InstallProgress {
        percent: 60.0,
        status: "Очистка реестра...".to_string(),
    });

    if install_path.exists() {
        fs::remove_dir_all(install_path).ok();
    }

    progress_cb(InstallProgress {
        percent: 100.0,
        status: "Удаление завершено".to_string(),
    });

    Ok(())
}

#[cfg(windows)]
fn remove_registry_entries() {
    use winreg::enums::*;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(
        r"Software\Microsoft\Windows\CurrentVersion\Uninstall\JentlememesLauncher",
    );
}

#[cfg(not(windows))]
fn remove_registry_entries() {}

#[cfg(windows)]
fn remove_shortcuts() {
    if let Some(desktop) = dirs::desktop_dir() {
        let lnk = desktop.join("JentleMemes Launcher.lnk");
        fs::remove_file(lnk).ok();
    }
    if let Some(start_menu) = dirs::data_dir() {
        let sm_dir = start_menu
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("JentleMemes Launcher");
        fs::remove_dir_all(sm_dir).ok();
    }
}

#[cfg(not(windows))]
fn remove_shortcuts() {}
