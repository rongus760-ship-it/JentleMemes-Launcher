fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
        println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
    }
    tauri_build::build()
}
