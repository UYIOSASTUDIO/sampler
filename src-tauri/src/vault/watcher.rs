use notify_debouncer_mini::{new_debouncer, notify::RecursiveMode, DebounceEventResult};
use std::path::Path;
use std::time::Duration;
use sqlx::SqlitePool;
use crate::vault::scanner;

pub fn start_background_watcher(watch_path: String, pool: SqlitePool) {
    tokio::spawn(async move {
        // Kanal für die Kommunikation zwischen dem synchronen OS-Watcher und unserem asynchronen Tokio-Thread
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        // Der Debouncer wartet 2 Sekunden nach der letzten Dateiänderung, um Dateikorruption
        // beim Kopieren großer Dateien zu verhindern.
        let mut debouncer = new_debouncer(Duration::from_secs(2), move |res: DebounceEventResult| {
            if let Ok(events) = res {
                for event in events {
                    let _ = tx.blocking_send(event.path);
                }
            }
        }).expect("Failed to initialize enterprise OS watcher");

        debouncer.watcher()
            .watch(Path::new(&watch_path), RecursiveMode::Recursive)
            .expect("Failed to hook into OS directory events");

        // Wir MÜSSEN das Debouncer-Objekt im Speicher behalten, sonst stirbt der Watcher sofort.
        let _keep_alive = debouncer;

        println!("Kernel-Hook active: Watching directory {}", watch_path);

        // Die Endlos-Schleife, die auf Kernel-Events wartet
        while let Some(path) = rx.recv().await {
            // Wenn eine neue Datei reinkommt, prüfen wir, ob sie existiert und ein Audio-File ist
            if path.is_file() && scanner::is_supported_audio_file(&path) {
                println!("Ghost-Scan triggered for: {:?}", path.file_name().unwrap_or_default());

                // Wir schieben die Datei an unseren Hochleistungs-Scanner
                scanner::process_single_file(path, pool.clone()).await;
            }
        }
    });
}