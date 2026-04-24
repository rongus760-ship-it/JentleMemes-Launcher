use crate::error::Result;
use tauri::AppHandle;

pub struct JavaResult {
    pub java_path: String,
    pub java_major: u32,
}

pub async fn resolve(
    app: &AppHandle,
    required_major: u32,
    custom_java: &str,
    is_legacy_forge: bool,
) -> Result<JavaResult> {
    let mut major = required_major;
    if is_legacy_forge && major > 8 {
        major = 8;
    }

    let java_path = if !custom_java.is_empty() {
        custom_java.to_string()
    } else {
        crate::core::java::ensure_java(app, major).await?
    };

    let detected = crate::core::java::detect_java_major(&java_path).await;
    let actual_major = detected.unwrap_or(major);

    let final_path = if is_legacy_forge && actual_major > 8 {
        crate::core::java::ensure_java(app, 8).await?
    } else {
        java_path
    };

    let final_major = if is_legacy_forge && actual_major > 8 {
        8
    } else {
        actual_major
    };

    Ok(JavaResult {
        java_path: final_path,
        java_major: final_major,
    })
}
