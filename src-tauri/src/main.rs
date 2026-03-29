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
        .plugin(tauri_plugin_fs::init()) // NEU: Aktiviert den direkten File-Stream
        .setup(|app_handle| {
            let app_dir = app_handle
                .path()
                .app_data_dir()
                .expect("Critical: Failed to resolve app data directory");

            if !app_dir.exists() {
                fs::create_dir_all(&app_dir)
                    .expect("Critical: Failed to create app data directory");
            }

            let db_path = app_dir.join("samplevault.db");

            tauri::async_runtime::block_on(async move {
                match db::init_db(&db_path).await {
                    Ok(pool) => {
                        app_handle.manage(AppState::new(pool));
                        println!("Database initialized successfully at: {:?}", db_path);
                    }
                    Err(e) => {
                        panic!("Critical: Database initialization failed: {}", e);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app::commands::scan_library,
            app::commands::get_samples,
            app::commands::clear_database
        ])
        .run(tauri::generate_context!())
        .expect("Critical: Error while running tauri application");
}