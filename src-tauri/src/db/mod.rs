use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::path::Path;

pub async fn init_db(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());

    // 1. Hardcore Performance-Tuning der SQLite Connection
    let connection_options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .pragma("cache_size", "-20000")
        .pragma("mmap_size", "30000000000")
        .pragma("temp_store", "MEMORY");

    // 2. Connection Pool konfigurieren
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connection_options)
        .await?;

    // 3. ENTERPRISE MIGRATIONS
    // Das Makro bindet den migrations-Ordner zur Compile-Zeit in die Binary ein.
    // Beim App-Start wird geprüft, welche SQL-Dateien bereits angewendet wurden.
    // Fehlende Updates werden transaktionssicher nachgezogen.
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;

    Ok(pool)
}