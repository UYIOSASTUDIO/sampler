use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::path::Path;

// Wir akzeptieren wieder das Path-Objekt, damit main.rs nicht weint
pub async fn init_db(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    // Wandelt den Pfad sicher in einen gültigen SQLite Connection-String um ("sqlite:///...")
    let db_url = format!("sqlite://{}", db_path.to_string_lossy());

    // 1. Hardcore Performance-Tuning der SQLite Connection
    let connection_options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true)
        // WAL-Mode: Erlaubt gleichzeitiges Lesen (UI) und Schreiben (Scanner)
        .journal_mode(SqliteJournalMode::Wal)
        // NORMAL statt FULL: Schreibt nicht sofort auf die SSD, sondern bündelt im RAM
        .synchronous(SqliteSynchronous::Normal)
        // Erhöht den RAM-Cache massiv (Standard ist winzig)
        .pragma("cache_size", "-20000")
        // Nutzt Memory-Mapped I/O für rasend schnelles Lesen von der SSD
        .pragma("mmap_size", "30000000000")
        // Temporäre Tabellen werden im RAM statt auf der SSD verarbeitet
        .pragma("temp_store", "MEMORY");

    // 2. Connection Pool konfigurieren
    let pool = SqlitePoolOptions::new()
        // Da wir WAL nutzen, können wir mehrere Verbindungen gleichzeitig offenhalten
        .max_connections(5)
        .connect_with(connection_options)
        .await?;

    // Tabellen erstellen (falls sie nicht existieren)
    let schema = include_str!("schema.sql");
    sqlx::query(schema).execute(&pool).await?;

    Ok(pool)
}