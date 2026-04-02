#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod db;
mod vault;

use app::state::AppState;
use app::commands;
use std::fs;
use tauri::{Manager, PhysicalSize};
use rodio::OutputStream;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicUsize;

fn main() {
    // 1. Initialisiere die native Audio-Engine des Betriebssystems
    let (_stream, stream_handle) = OutputStream::try_default().expect("Failed to initialize audio stream");

    // 2. Enterprise-Hack: Den Stream im RAM "leaken", damit er während der gesamten App-Laufzeit offen bleibt
    std::mem::forget(_stream);

    // 3. Den Status für unsere Commands vorbereiten
    let audio_state = app::commands::AudioState {
        stream_handle,
        current_sink: Mutex::new(None),
        playback_id: Arc::new(AtomicUsize::new(0)),
    };

    tauri::Builder::default()
        .manage(audio_state)
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_drag::init())
        .setup(|app_handle| {
            // --- NEU: Dynamische Berechnung der minimalen Fenstergröße ---
            if let Some(window) = app_handle.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let screen_size = monitor.size();

                    // Berechnung: 2/3 der Bildschirmbreite und 1/2 der Bildschirmhöhe
                    let min_width = (screen_size.width as f64 * (2.0 / 3.0)).round() as u32;
                    let min_height = (screen_size.height as f64 * 0.5).round() as u32;

                    // Setzen der berechneten Werte als ununterschreitbares Minimum
                    let _ = window.set_min_size(Some(PhysicalSize::new(min_width, min_height)));
                }
            }
            // -------------------------------------------------------------

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
            commands::reveal_in_finder,
            commands::toggle_sample_like,
            commands::get_collections,
            commands::create_collection,
            commands::add_to_collection,
            commands::bulk_toggle_like,
            commands::update_sample_metadata,
            commands::get_user_tags,
            commands::create_user_tag,
            commands::delete_user_tag,
            commands::get_all_available_tags,
            commands::get_waveform,
            commands::play_audio,
            commands::stop_audio,
            commands::set_audio_volume
        ])
        .run(tauri::generate_context!())
        .expect("Critical: Error while running tauri application");
}