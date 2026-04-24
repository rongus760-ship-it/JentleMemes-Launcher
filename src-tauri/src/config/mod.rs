use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// ---------- Основные структуры ----------
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccountInfo {
    pub id: String,
    pub username: String,
    pub uuid: String,
    pub token: String,
    /// Пустое / отсутствующее в старом JSON — восстанавливается по префиксу `id` при refresh.
    #[serde(default)]
    pub acc_type: String,
    #[serde(default)]
    pub active_skin_id: String,
    /// Refresh token Microsoft (offline_access); нужен для обновления Minecraft-сессии при каждом запуске.
    #[serde(default)]
    pub ms_refresh_token: String,
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
    /// "slim" | "default" — для офлайн-скина через authlib-injector.
    #[serde(default)]
    pub model: String,
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
    /// Затемнение цветом темы поверх фоновой картинки, 12..=98 — чем меньше, тем ярче виден фон.
    #[serde(default = "default_background_dim_percent")]
    pub background_dim_percent: u8,
    /// Непрозрачность панелей при фоновой картинке (40..=100). Ниже — «стекло», фон виден сильнее.
    #[serde(default = "default_ui_panel_opacity_percent")]
    pub ui_panel_opacity_percent: u8,
    #[serde(default = "default_true")]
    pub download_dependencies: bool,
    #[serde(default)]
    pub hybrid_provider_enabled: bool,
    /// Внутренний просмотр папок данных (экспериментально).
    #[serde(default)]
    pub internal_file_browser: bool,
    #[serde(default = "default_true")]
    pub show_news: bool,
    #[serde(default)]
    pub mod_provider: String,
    /// CurseForge API key (from https://console.curseforge.com). If empty, CurseForge/Гибрид возвращают только Modrinth.
    #[serde(default)]
    pub curseforge_api_key: String,
    #[serde(default)]
    pub custom_themes: Vec<serde_json::Value>,
    /// Показывать в Discord статус «играет в Minecraft» на время сессии (нужен Application ID в `discord_presence.rs`).
    #[serde(default = "default_true")]
    pub discord_rich_presence: bool,
    /// Показать отдельную вкладку «Расширенные настройки» в шапке (по умолчанию скрыта).
    #[serde(default)]
    pub show_advanced_tab: bool,
    /// Через сколько **активных** часов в лаунчере обновлять Microsoft-сессии по refresh_token.
    #[serde(default = "default_token_refresh_hours")]
    pub token_refresh_active_hours: f64,
    /// Перед запуском любой сборки обновлять все Microsoft-аккаунты.
    #[serde(default)]
    pub token_refresh_on_instance_launch: bool,
    /// Накопленное время активности (мс) для фонового refresh.
    #[serde(default)]
    pub launcher_active_ms_accumulated: u64,
    /// Поставщик JRE при скачивании: adoptium | zulu | microsoft (microsoft → Adoptium Microsoft build где доступно).
    #[serde(default = "default_java_provider")]
    pub java_download_provider: String,
    /// Один `clientToken` на установку лаунчера для Yggdrasil (Ely.by).
    #[serde(default)]
    pub yggdrasil_client_token: String,
    /// Дефолтная JRE для major: подкаталог внутри `data_dir/java/` (например `runtimes/eclipse_jdk-17.0.18_8`).
    #[serde(default)]
    pub java_major_default_subdir: HashMap<String, String>,
    /// Отключить анимации и тяжёлые визуальные эффекты.
    #[serde(default)]
    pub reduce_motion: bool,
    /// Linux: в начало JVM добавить `-XX:+UnlockExperimentalVMOptions` (частые краши OpenAL / LWJGL).
    #[serde(default)]
    pub linux_jvm_unlock_experimental: bool,
    /// Linux: в конец JVM добавить `-Dorg.lwjgl.openal.libname=…` (путь к системному OpenAL).
    #[serde(default)]
    pub linux_lwjgl_openal_libname: bool,
    /// Путь к `libopenal.so` (зависит от дистрибутива).
    #[serde(default = "default_linux_openal_lib_path")]
    pub linux_openal_lib_path: String,
    /// HTTP(S) прокси для загрузок лаунчера (моды, мета, Java, файлы игры). Пример: `http://127.0.0.1:7890`
    #[serde(default)]
    pub download_proxy_url: String,
    /// Показать вкладку «Чат» (соц. сеть / друзья, API сайта).
    #[serde(default)]
    pub show_friends_chat_tab: bool,
    /// Базовый URL API сайта (без завершающего `/`). Чат и вход в лаунчере.
    #[serde(default = "default_jentlememes_api_base")]
    pub jentlememes_api_base_url: String,
    /// В профиле пользователя в чате показывать блок Minecraft-сервера (MOTD, онлайн).
    #[serde(default)]
    pub chat_profile_mc_server: bool,
    /// LiteLoader и Risugami ModLoader в списке загрузчиков (метаданные Prism / vanilla-only профиль).
    #[serde(default)]
    pub enable_alpha_loaders: bool,
    /// В списках версий Minecraft и в каталоге модов: снапшоты / предрелизы (Modrinth: snapshot; Mojang: snapshot).
    #[serde(default)]
    pub show_mc_snapshot_versions: bool,
    /// В списках версий и в каталоге модов: альфа/бета (Modrinth: alpha, beta; Mojang: old_alpha, old_beta).
    #[serde(default)]
    pub show_mc_alpha_beta_versions: bool,
    /// Оверлей поверх игры (второе окно), переключается глобальной горячей клавишей.
    #[serde(default)]
    pub ingame_overlay_enabled: bool,
    /// Сочетание для оверлея (формат Tauri, напр. Alt+Backquote для Alt+`).
    #[serde(default = "default_ingame_overlay_hotkey")]
    pub ingame_overlay_hotkey: String,
    /// Глобальный масштаб интерфейса (0.85..=1.60). По умолчанию 1.05.
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    /// Стиль бокового меню: "expanded" (полный, по умолчанию) или "compact" (только иконки).
    #[serde(default = "default_sidebar_style")]
    pub sidebar_style: String,
    /// Расположение и тип панели навигации (см. фронтенд `ChromeLayout`).
    #[serde(default = "default_chrome_layout")]
    pub chrome_layout: String,
    /// Внешний вид модальных окон: minimal | glass | dense | sheet.
    #[serde(default = "default_modal_preset")]
    pub modal_preset: String,
    /// Визуальный пресет UI (2.0): blend | modrinth | discord | legacy | glass.
    /// Задаёт радиусы, плотность, тени, интенсивность декоративных эффектов.
    /// Оси темизации — см. src/lib/themeApply.ts::applyVisualPreset.
    #[serde(default = "default_visual_preset")]
    pub visual_preset: String,
    /// Shell layout (2.0): classic | dock-bottom | split-rail | command-only | holo-arc.
    /// Перетасовывает расположение и оформление дока/навигации (ось «полная смена UI»).
    #[serde(default = "default_shell_layout")]
    pub shell_layout: String,
    /// Overlay layout (2.0): panel | hud | radial | ticker | neon-grid.
    /// Радикальный пересмотр разметки in-game оверлея.
    #[serde(default = "default_overlay_layout")]
    pub overlay_layout: String,
    /// Угол экрана для виджета загрузки / фонового прогресса: bl | br | tl | tr | hidden.
    #[serde(default = "default_download_corner")]
    pub download_corner: String,
    /// Пройден ли начальный визард настройки (appearance onboarding).
    /// Для принудительного показа визарда: запуск с флагом `--onboarding`.
    #[serde(default)]
    pub onboarding_completed: bool,
}

fn default_jentlememes_api_base() -> String {
    "https://jentlememes.ru".into()
}

fn default_ingame_overlay_hotkey() -> String {
    "Alt+Backquote".into()
}

fn default_ui_scale() -> f32 {
    1.05
}

fn default_sidebar_style() -> String {
    "expanded".into()
}

fn default_chrome_layout() -> String {
    "sidebar_left_expanded".into()
}

fn default_modal_preset() -> String {
    "minimal".into()
}

fn default_visual_preset() -> String {
    "blend".into()
}

fn default_shell_layout() -> String {
    "classic".into()
}

fn default_overlay_layout() -> String {
    "panel".into()
}

fn default_download_corner() -> String {
    "bl".into()
}

fn default_background_dim_percent() -> u8 {
    78
}

/// Дефолт 96 (эволюция: 82 → 92 → 96) и минимум 78 в UI. При <78 % на ярких
/// обоях плитки становились невидимыми и UI «пустел». Эти пороги отлажены под
/// тему `jentle-dark` (--card = #141a14) на пользовательских обоях.
fn default_ui_panel_opacity_percent() -> u8 {
    96
}

fn default_linux_openal_lib_path() -> String {
    "/usr/lib/libopenal.so".into()
}

fn default_true() -> bool {
    true
}

fn default_token_refresh_hours() -> f64 {
    22.0
}

fn default_java_provider() -> String {
    "adoptium".into()
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
            background_dim_percent: default_background_dim_percent(),
            ui_panel_opacity_percent: default_ui_panel_opacity_percent(),
            show_news: true,
            download_dependencies: true,
            hybrid_provider_enabled: false,
            internal_file_browser: false,
            mod_provider: "modrinth".into(),
            curseforge_api_key: String::new(),
            custom_themes: vec![],
            discord_rich_presence: true,
            show_advanced_tab: false,
            token_refresh_active_hours: 22.0,
            token_refresh_on_instance_launch: false,
            launcher_active_ms_accumulated: 0,
            java_download_provider: "adoptium".into(),
            yggdrasil_client_token: String::new(),
            java_major_default_subdir: HashMap::new(),
            reduce_motion: false,
            linux_jvm_unlock_experimental: false,
            linux_lwjgl_openal_libname: false,
            linux_openal_lib_path: default_linux_openal_lib_path(),
            download_proxy_url: String::new(),
            show_friends_chat_tab: false,
            jentlememes_api_base_url: default_jentlememes_api_base(),
            chat_profile_mc_server: false,
            enable_alpha_loaders: false,
            show_mc_snapshot_versions: false,
            show_mc_alpha_beta_versions: false,
            ingame_overlay_enabled: false,
            ingame_overlay_hotkey: default_ingame_overlay_hotkey(),
            ui_scale: default_ui_scale(),
            sidebar_style: default_sidebar_style(),
            chrome_layout: default_chrome_layout(),
            modal_preset: default_modal_preset(),
            visual_preset: default_visual_preset(),
            shell_layout: default_shell_layout(),
            overlay_layout: default_overlay_layout(),
            download_corner: default_download_corner(),
            onboarding_completed: false,
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
    /// Путь к `java` только для этой сборки (пусто — глобальный/авто).
    #[serde(default)]
    pub custom_java_path: String,
}

impl Default for InstanceSettings {
    fn default() -> Self {
        Self {
            override_global: false,
            ram_mb: 4096,
            jvm_args: "".into(),
            use_discrete_gpu: false,
            custom_java_path: "".into(),
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
    /// Модпак с CurseForge: version_id в UI = file_id файла .mrpack
    Curseforge {
        project_id: String,
        file_id: String,
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

fn bootstrap_config_dir() -> PathBuf {
    dirs::config_dir()
        .or_else(dirs::home_dir)
        .unwrap()
        .join("JentleMemes")
}

fn data_root_override_file() -> PathBuf {
    bootstrap_config_dir().join("data_root.json")
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct DataRootFile {
    #[serde(default)]
    path: String,
}

/// Явный каталог данных (вне стандартного). Хранится в конфиге ОС, чтобы не зависеть от расположения самих данных.
pub fn read_data_root_override() -> Option<PathBuf> {
    let p = data_root_override_file();
    if !p.exists() {
        return None;
    }
    let s = fs::read_to_string(&p).ok()?;
    let v: DataRootFile = serde_json::from_str(&s).ok()?;
    let path = v.path.trim();
    if path.is_empty() {
        return None;
    }
    Some(PathBuf::from(path))
}

pub fn set_data_root_override(path: Option<PathBuf>) -> Result<()> {
    let dir = bootstrap_config_dir();
    fs::create_dir_all(&dir)?;
    let file = data_root_override_file();
    match path {
        Some(p) => {
            let obj = DataRootFile {
                path: p.to_string_lossy().to_string(),
            };
            fs::write(&file, serde_json::to_string_pretty(&obj)?)?;
        }
        None => {
            let _ = fs::remove_file(&file);
        }
    }
    Ok(())
}

/// Стандартный каталог данных без переопределения: `XDG_DATA_HOME/JentleMemes` и аналоги.
pub fn default_data_dir_without_override() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .unwrap()
        .join("JentleMemes")
}

pub fn get_data_dir() -> PathBuf {
    if let Some(p) = read_data_root_override() {
        return p;
    }
    default_data_dir_without_override()
}

/// Перенос `~/.jentlememes_data` → стандартный каталог, если переопределения нет и новый путь пуст.
pub fn migrate_legacy_home_data_if_needed() {
    if read_data_root_override().is_some() {
        return;
    }
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let legacy = home.join(".jentlememes_data");
    if !legacy.exists() {
        return;
    }
    let newd = default_data_dir_without_override();
    if newd.exists() {
        return;
    }
    if let Some(parent) = newd.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::rename(&legacy, &newd);
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
    let text = serde_json::to_string_pretty(settings)?;
    crate::core::utils::atomic_fs::write_atomic_string(&path, &text)
}

/// Восстанавливает `acc_type` для старых записей без поля (иначе сессия MS не обновляется).
pub fn normalize_account_types(profiles: &mut Profiles) -> bool {
    let mut changed = false;
    for acc in profiles.accounts.iter_mut() {
        if !acc.acc_type.is_empty() {
            continue;
        }
        if acc.id.starts_with("ms-") {
            acc.acc_type = "microsoft".into();
            changed = true;
        } else if acc.id.starts_with("elyby-") {
            acc.acc_type = "elyby".into();
            changed = true;
        } else if acc.id.starts_with("offline-") {
            acc.acc_type = "offline".into();
            changed = true;
        } else {
            acc.acc_type = "offline".into();
            changed = true;
        }
    }
    changed
}

pub fn load_profiles() -> Result<Profiles> {
    let path = get_data_dir().join("profiles.json");
    if path.exists() {
        let content = fs::read_to_string(path)?;
        if let Ok(p) = serde_json::from_str::<Profiles>(&content) {
            return Ok(p);
        }
    }
    Ok(Profiles::default())
}

pub fn save_profiles(profiles: &Profiles) -> Result<()> {
    let path = get_data_dir().join("profiles.json");
    fs::create_dir_all(get_data_dir())?;
    let text = serde_json::to_string_pretty(profiles)?;
    crate::core::utils::atomic_fs::write_atomic_string(&path, &text)
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
    let text = serde_json::to_string_pretty(source)?;
    crate::core::utils::atomic_fs::write_atomic_string(&path, &text)
}

/// Load servers for a specific instance
pub fn load_instance_servers(inst_dir: &std::path::Path) -> Result<Vec<RecentServer>> {
    let path = inst_dir.join("servers.json");
    if path.exists() {
        let content = fs::read_to_string(path)?;
        if let Ok(servers) = serde_json::from_str(&content) {
            return Ok(servers);
        }
    }
    Ok(vec![])
}

/// Save servers for a specific instance
pub fn save_instance_servers(inst_dir: &std::path::Path, servers: &[RecentServer]) -> Result<()> {
    let path = inst_dir.join("servers.json");
    let text = serde_json::to_string_pretty(servers)?;
    crate::core::utils::atomic_fs::write_atomic_string(&path, &text)
}

pub fn load_instance_last_world(inst_dir: &std::path::Path) -> Result<Option<LastWorldEntry>> {
    let path = inst_dir.join("last_world.json");
    if path.exists() {
        let content = fs::read_to_string(path)?;
        if let Ok(w) = serde_json::from_str(&content) {
            return Ok(Some(w));
        }
    }
    Ok(None)
}

pub fn save_instance_last_world(inst_dir: &std::path::Path, world: &LastWorldEntry) -> Result<()> {
    let path = inst_dir.join("last_world.json");
    let text = serde_json::to_string_pretty(world)?;
    crate::core::utils::atomic_fs::write_atomic_string(&path, &text)
}
