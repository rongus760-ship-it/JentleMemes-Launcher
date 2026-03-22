#![windows_subsystem = "windows"]

mod commands;
mod config;
mod core;
mod custom_packs;
mod custom_packs_url;
mod error;

use commands::*;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|_| {
            tauri::async_runtime::spawn(async {
                let _ = crate::custom_packs::fetch_and_cache_packs().await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // settings
            load_settings,
            save_settings,
            get_backgrounds,
            // instances
            get_instances,
            create_instance,
            delete_instance,
            update_instance_core,
            repair_core,
            open_folder,
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
            // modrinth
            search_modrinth,
            get_modrinth_project,
            get_modrinth_versions,
            search_curseforge,
            get_curseforge_project,
            get_curseforge_versions,
            search_hybrid,
            get_hybrid_versions,
            install_modrinth_file,
            // mrpack
            install_mrpack_from_url,
            install_mrpack,
            list_instance_folders,
            export_instance,
            export_mrpack,
            import_instance,
            check_modpack_update,
            update_modpack,
            // game
            fetch_vanilla_versions,
            install_version,
            install_fabric,
            install_quilt,
            install_forge,
            download_game_files,
            extract_natives,
            launch_game,
            stop_instance,
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
            get_system_ram,
            ping_server,
            get_loader_versions,
            get_specific_loader_versions,
            get_data_dir,
            load_custom_packs_config,
            fetch_custom_packs,
            get_custom_packs,
            refresh_custom_packs,
            pick_image_file,
            copy_background,
            delete_background,
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