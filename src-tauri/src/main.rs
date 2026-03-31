#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod db;
mod vault;

use app::state::AppState;
use app::commands;
use std::fs;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init()) // Aktiviert den direkten File-Stream
        .setup(|app_handle| {
            // 1. Enterprise Logger initialisieren und im Speicher verankern
            let log_guard = app::logger::setup_logging(app_handle);
            app_handle.manage(log_guard);

            // 2. App Data Directory für die Datenbank auflösen
            let app_dir = app_handle
                .path()
                .app_data_dir()
                .expect("Critical: Failed to resolve app data directory");

            if !app_dir.exists() {
                fs::create_dir_all(&app_dir)
                    .expect("Critical: Failed to create app data directory");
            }

            let db_path = app_dir.join("samplevault.db");

            // 3. Datenbank asynchron initialisieren und State registrieren
            tauri::async_runtime::block_on(async move {
                match db::init_db(&db_path).await {
                    Ok(pool) => {
                        app_handle.manage(AppState::new(pool));
                        tracing::info!("Database initialized successfully at: {:?}", db_path);
                    }
                    Err(e) => {
                        tracing::error!("Critical: Database initialization failed: {}", e);
                        panic!("Critical: Database initialization failed: {}", e);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::scan_library,
            commands::rescan_all_folders,
            commands::get_samples,
            commands::clear_database,
            commands::cleanup_database,
            commands::remove_folder,
            commands::get_connected_folders,
            commands::reveal_in_finder
        ])
        .run(tauri::generate_context!())
        .expect("Critical: Error while running tauri application");
}