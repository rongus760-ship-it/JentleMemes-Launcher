use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Default)]
pub struct DownloadProgress {
    pub task_name: String,
    pub downloaded: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    /// Не трогать UI busy/прогресс; фоновая мета прерывается при любом несilent прогрессе.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub silent: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Library {
    pub name: String,
    pub downloads: Option<Value>,
    pub url: Option<String>,
    pub rules: Option<Vec<Value>>,
    pub natives: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub id: Option<String>,              // Сделали необязательным
    pub inherits_from: Option<String>,
    #[serde(rename = "type")]            // Явно указываем имя поля "type"
    pub type_: Option<String>,           // Сделали необязательным
    pub main_class: Option<String>,
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<Value>,
    #[serde(default)]                    // Если массива нет, будет пустой
    pub libraries: Vec<Library>,
    pub asset_index: Option<Value>,
    pub assets: Option<String>,
    pub downloads: Option<Value>,
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub maven_files: Vec<Library>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub major_version: u32,
    pub component: Option<String>,
}