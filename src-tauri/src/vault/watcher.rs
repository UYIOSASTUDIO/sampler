use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use std::path::Path;
use std::time::Duration;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};
use crate::vault::scanner;

pub fn start_background_watcher(watch_path: String, pool: SqlitePool, app: AppHandle) {
    tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut debouncer = new_debouncer(Duration::from_secs(2), move |res: DebounceEventResult| {
            if let Ok(events) = res {
                for event in events {
                    let _ = tx.send(event.path);
                }
            }
        }).expect("Failed to initialize enterprise OS watcher");

        debouncer.watcher()
            .watch(Path::new(&watch_path), RecursiveMode::Recursive)
            .expect("Failed to hook into OS directory events");

        let _keep_alive = debouncer;

        tracing::info!("Kernel-Hook active: Watching directory {}", watch_path);

        while let Some(path) = rx.recv().await {
            if path.exists() {
                if path.is_dir() {
                    tracing::info!("Ghost-Scan: New Directory detected {:?}", path);
                    // ENTERPRISE FIX: app.clone() wird nun an den Scanner übergeben,
                    // damit auch Hintergrund-Scans den UI-Ladebalken aktivieren.
                    let _ = scanner::scan_directory(
                        path.to_string_lossy().to_string(),
                        pool.clone(),
                        app.clone()
                    ).await;

                    let _ = app.emit("library-updated", ());
                } else if path.is_file() && scanner::is_supported_audio_file(&path) {
                    tracing::info!("Ghost-Scan: Added {:?}", path.file_name().unwrap_or_default());
                    scanner::process_single_file(path, pool.clone(), app.clone()).await;
                    let _ = app.emit("library-updated", ());
                }
            } else {
                tracing::info!("Ghost-Scan: Deleted {:?}", path);
                let path_str = path.to_string_lossy().to_string();

                let like_path = format!("{}%", path_str);
                let _ = sqlx::query("DELETE FROM samples WHERE original_path = ? OR original_path LIKE ?")
                    .bind(&path_str)
                    .bind(&like_path)
                    .execute(&pool)
                    .await;

                let _ = app.emit("library-updated", ());
            }
        }
    });
}