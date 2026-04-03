use tauri::{App, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn setup_logging(app: &App) -> WorkerGuard {
    // Ermittelt den standardisierten Log-Ordner des Betriebssystems (macOS: ~/Library/Logs/...)
    let log_dir = app
        .path()
        .app_log_dir()
        .expect("Failed to determine log directory");
    std::fs::create_dir_all(&log_dir).expect("Failed to create log directory");

    // Täglich rotierende Log-Dateien (verhindert gigantische Dateien)
    let file_appender = tracing_appender::rolling::daily(&log_dir, "sample-vault.log");

    // Enterprise Pattern: Non-Blocking Writer, blockiert niemals den UI-Thread
    let (non_blocking_appender, guard) = tracing_appender::non_blocking(file_appender);

    // Layer 1: Das Datei-Log (Ohne ANSI-Farbcodes, aber mit Thread-IDs für präzises Debugging)
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking_appender)
        .with_ansi(false)
        .with_thread_ids(true)
        .with_target(true);

    // Layer 2: Das Terminal-Log (Für uns Entwickler, solange die App im Dev-Modus läuft)
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_target(false);

    // Filter: Blockiert den Datenbank-Spam von sqlx, lässt aber unsere eigenen Logs durch
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .init();

    tracing::info!("Enterprise Logger initialized successfully.");
    tracing::info!("Log directory location: {:?}", log_dir);

    // WICHTIG: Der Guard MUSS zurückgegeben und am Leben gehalten werden,
    // sonst stirbt der asynchrone Writer sofort und es wird nichts auf die Festplatte geschrieben.
    guard
}
