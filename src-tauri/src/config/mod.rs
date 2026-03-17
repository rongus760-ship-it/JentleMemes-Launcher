use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use crate::error::Result;

// ---------- Основные структуры ----------
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountInfo {
    pub id: String,
    pub username: String,
    pub uuid: String,
    pub token: String,
    pub acc_type: String,
    #[serde(default)]
    pub active_skin_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkinPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skin_type: String,
    #[serde(default)]
    pub skin_data: String,
    #[serde(default)]
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Profiles {
    pub accounts: Vec<AccountInfo>,
    pub active_account_id: String,
    #[serde(default)]
    pub skin_presets: Vec<SkinPreset>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecentServer {
    pub ip: String,
    pub name: String,
    pub last_played: u64,
    #[serde(default)]
    pub playtime_hours: f64,
    #[serde(default)]
    pub last_instance_id: Option<String>,
    #[serde(default)]
    pub last_instance_name: Option<String>,
}

/// Один последний мир (глобально для лаунчера)
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LastWorldEntry {
    pub instance_id: String,
    pub instance_name: String,
    pub world_name: String,
    pub last_played: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LauncherSettings {
    pub ram_mb: u32,
    pub jvm_args: String,
    pub wrapper: String,
    pub close_on_launch: bool,
    pub custom_java_path: String,
    #[serde(default)]
    pub recent_servers: Vec<RecentServer>,
    #[serde(default)]
    pub last_world: Option<LastWorldEntry>,
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub background: String,
    #[serde(default = "default_true")]
    pub download_dependencies: bool,
    #[serde(default)]
    pub hybrid_provider_enabled: bool,
    #[serde(default)]
    pub mod_provider: String,
    /// CurseForge API key (from https://console.curseforge.com). If empty, CurseForge/Гибрид возвращают только Modrinth.
    #[serde(default)]
    pub curseforge_api_key: String,
}

fn default_true() -> bool {
    true
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            ram_mb: 4096,
            jvm_args: "-XX:+UseG1GC -XX:+UnlockExperimentalVMOptions".into(),
            wrapper: "".into(),
            close_on_launch: true,
            custom_java_path: "".into(),
            recent_servers: vec![],
            last_world: None,
            theme: "jentle-dark".into(),
            background: "".into(),
            download_dependencies: true,
            hybrid_provider_enabled: false,
            mod_provider: "modrinth".into(),
            curseforge_api_key: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstanceSettings {
    pub override_global: bool,
    pub ram_mb: u32,
    pub jvm_args: String,
    #[serde(default)]
    pub use_discrete_gpu: bool,
}

impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            override_global: false,
            ram_mb: 4096,
            jvm_args: "".into(),
            use_discrete_gpu: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstanceConfig {
    pub id: String,
    pub name: String,
    pub game_version: String,
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub settings: Option<InstanceSettings>,
    #[serde(default)]
    pub playtime: u64,
}

/// Метаданные источника сборки для проверки обновлений
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PackSource {
    Modrinth {
        project_id: String,
        version_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        version_name: Option<String>,
        #[serde(default)]
        installed_files: Vec<InstalledFile>,
    },
    Custom {
        pack_url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pack_id: Option<String>,
        installed_mrpack_sha1: String,
        #[serde(default)]
        installed_files: Vec<InstalledFile>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstalledFile {
    pub path: String,
    pub sha1: String,
}

// Структура для ответа от Microsoft OAuth (добавлена один раз)
#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub interval: u64,
}

// ---------- Работа с файлами ----------
pub fn get_data_dir() -> PathBuf {
    let mut p = dirs::home_dir().unwrap();
    p.push(".jentlememes_data");
    p
}

pub fn load_settings() -> Result<LauncherSettings> {
    let path = get_data_dir().join("settings.json");
    if path.exists() {
        let content = fs::read_to_string(path)?;
        if let Ok(s) = serde_json::from_str(&content) {
            return Ok(s);
        }
    }
    Ok(LauncherSettings::default())
}

pub fn save_settings(settings: &LauncherSettings) -> Result<()> {
    let path = get_data_dir().join("settings.json");
    fs::create_dir_all(get_data_dir())?;
    fs::write(path, serde_json::to_string_pretty(settings)?)?;
    Ok(())
}

pub fn load_profiles() -> Result<Profiles> {
    let path = get_data_dir().join("profiles.json");
    if path.exists() {
        let content = fs::read_to_string(path)?;
        if let Ok(p) = serde_json::from_str(&content) {
            return Ok(p);
        }
    }
    Ok(Profiles::default())
}

pub fn save_profiles(profiles: &Profiles) -> Result<()> {
    let path = get_data_dir().join("profiles.json");
    fs::create_dir_all(get_data_dir())?;
    fs::write(path, serde_json::to_string_pretty(profiles)?)?;
    Ok(())
}

/// Загружает pack_source.json из директории инстанса
pub fn load_pack_source(inst_dir: &std::path::Path) -> Result<Option<PackSource>> {
    let path = inst_dir.join("pack_source.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let source = serde_json::from_str(&content)?;
    Ok(Some(source))
}

/// Сохраняет pack_source.json в директорию инстанса
pub fn save_pack_source(inst_dir: &std::path::Path, source: &PackSource) -> Result<()> {
    let path = inst_dir.join("pack_source.json");
    fs::write(path, serde_json::to_string_pretty(source)?)?;
    Ok(())
}
