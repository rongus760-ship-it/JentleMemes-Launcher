#![windows_subsystem = "windows"]

mod commands;
mod config;
mod core;
mod custom_packs;
mod custom_packs_url;
mod error;

use commands::*;

#[cfg(target_os = "linux")]
fn install_linux_webview_permissions(app: &tauri::App) {
    use tauri::Manager;
    let label_list: Vec<String> = app
        .webview_windows()
        .keys()
        .cloned()
        .collect();
    for label in label_list {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.with_webview(|webview| {
                // SAFETY: inner() возвращает webkit2gtk::WebView из того же процесса UI-потока.
                use webkit2gtk::{PermissionRequestExt, WebViewExt};
                let gtk_webview = webview.inner();
                gtk_webview.connect_permission_request(|_wv, req| {
                    // Для WebRTC / getUserMedia (микрофон, камера), геолокации и уведомлений
                    // лаунчера сразу соглашаемся: пользователь и так работает в своём клиенте.
                    // Без этого на webkit2gtk getUserMedia возвращает NotAllowedError до любого prompt.
                    req.allow();
                    true
                });
                // Явно включаем media-capture и доступ к устройствам в политике безопасности WebKit.
                if let Some(settings) = gtk_webview.settings() {
                    use webkit2gtk::SettingsExt;
                    settings.set_enable_media_stream(true);
                    settings.set_enable_mediasource(true);
                    settings.set_enable_webrtc(true);
                    settings.set_enable_encrypted_media(true);
                    settings.set_media_playback_requires_user_gesture(false);
                }
            });
        }
    }
}

/// На Wayland + проприетарный NVIDIA WebKitGTK с DMA-BUF часто даёт «слайдшоу» FPS.
/// Переменная должна быть выставлена до инициализации WebKit (первый вызов GTK/WebView).
#[cfg(target_os = "linux")]
fn apply_linux_webkit_defaults() {
    let wayland = std::env::var_os("WAYLAND_DISPLAY")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if wayland && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

fn parse_cli_flags() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut force_onboarding = false;
    for a in args {
        match a.as_str() {
            "--onboarding" | "--setup" | "--setup-wizard" | "-w" => {
                force_onboarding = true;
            }
            _ => {}
        }
    }
    if force_onboarding {
        commands::set_force_onboarding(true);
    }
}

/// Инициализация структурированного логирования через `tracing` с двумя слоями:
/// stdout (цветной) и файл `<data_dir>/logs/launcher-YYYYMMDD.log` с ротацией.
/// Возвращает guard, который нужно держать до конца процесса (иначе буфер файла
/// не будет сброшен). Уровень по умолчанию — `INFO`, переопределяется `JM_LOG`.
fn init_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let data_dir = crate::config::get_data_dir();
    let log_dir = data_dir.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let file_appender = tracing_appender::rolling::daily(log_dir, "launcher.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env = EnvFilter::try_from_env("JM_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env)
        .with(fmt::layer().with_target(true))
        .with(
            fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking)
                .with_target(true),
        )
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        "FluxCore logging initialized"
    );

    guard
}

fn main() {
    #[cfg(target_os = "linux")]
    apply_linux_webkit_defaults();
    parse_cli_flags();
    let _log_guard = init_tracing();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|_app| {
            crate::config::migrate_legacy_home_data_if_needed();
            tauri::async_runtime::spawn(async {
                let _ = crate::custom_packs::fetch_and_cache_packs().await;
            });
            #[cfg(target_os = "linux")]
            install_linux_webview_permissions(_app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // settings
            load_settings,
            save_settings,
            patch_settings,
            is_onboarding_pending,
            complete_onboarding,
            is_game_session_running,
            get_running_instance_ids,
            get_overlay_target_rect,
            get_game_overlay_stats,
            get_backgrounds,
            // instances
            get_instances,
            create_instance,
            delete_instance,
            update_instance_core,
            repair_core,
            open_folder,
            open_launcher_data_folder,
            get_default_data_dir_path,
            get_data_root_override_path_json,
            pick_data_root_folder,
            apply_data_root_override,
            clear_data_root_override,
            add_launcher_active_ms,
            list_detected_java_runtimes,
            download_java_major,
            list_java_build_options,
            download_java_build,
            set_java_default_for_major,
            install_dropped_content_file,
            backup_instance_zip,
            get_instance_content_counts,
            list_data_subdir_entries,
            save_instance_settings,
            // mods
            get_installed_mods,
            get_installed_content,
            refresh_mod_metadata,
            list_worlds,
            install_datapack,
            toggle_mod,
            delete_mod,
            check_mod_updates,
            install_mod_with_dependencies,
            // auth
            load_profiles,
            save_profiles,
            login_offline,
            login_elyby,
            ms_init_device_code,
            ms_login_poll,
            ms_oauth_prepare_interactive,
            ms_oauth_try_take_account,
            refresh_microsoft_sessions_startup,
            refresh_account_for_launch,
            resolve_session_skin,
            resolve_skin_texture_by_username,
            upload_skin_mojang_for_account,
            upload_skin_from_remote_username_for_account,
            delete_skin_elyby_for_account,
            // modrinth
            search_modrinth,
            get_modrinth_project,
            get_modrinth_versions,
            search_curseforge,
            get_curseforge_project,
            get_curseforge_versions,
            get_modrinth_version_details,
            get_curseforge_file_details,
            download_mod_to_user_folder,
            search_hybrid,
            get_hybrid_versions,
            install_modrinth_file,
            install_curseforge_mod_file,
            // mrpack
            install_mrpack_from_url,
            install_mrpack,
            list_instance_folders,
            export_instance,
            export_mrpack,
            export_jentlepack,
            export_jentlepack_to_temp,
            read_data_tmp_file_base64,
            import_instance,
            import_jentlepack_from_url,
            check_modpack_update,
            update_modpack,
            // game
            fetch_vanilla_versions,
            install_version,
            install_fabric,
            install_quilt,
            install_forge,
            install_liteloader,
            install_modloader,
            prepare_launch,
            download_game_files,
            launch_game,
            fluxcore_launch,
            stop_instance,
            stop_game_from_overlay,
            tail_game_log,
            take_minecraft_screenshot,
            add_recent_server,
            load_servers,
            import_servers_from_dat,
            update_server_last_played,
            update_last_world,
            get_last_world,
            rename_instance,
            unlink_modpack,
            get_pack_source_info,
            // launcher updates & news
            fetch_launcher_news,
            check_launcher_update,
            download_and_apply_update,
            // utils
            runtime_os,
            get_system_ram,
            ping_server,
            get_loader_versions,
            get_ui_game_versions,
            get_specific_loader_versions,
            get_data_dir,
            verify_data_dir_writable,
            load_custom_packs_config,
            fetch_custom_packs,
            get_custom_packs,
            refresh_custom_packs,
            pick_image_file,
            copy_background,
            delete_background,
            read_local_image_data_url,
            // window management
            window_close,
            window_minimize,
            window_maximize,
            window_drag,
            window_is_maximized,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
