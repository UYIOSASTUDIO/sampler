use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool};
use std::path::Path;
use std::str::FromStr;

pub async fn init_db(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let database_url = format!("sqlite:{}", db_path.display());

    let connection_options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .pragma("journal_mode", "WAL")
        .pragma("synchronous", "NORMAL")
        .pragma("foreign_keys", "ON")
        .pragma("temp_store", "MEMORY");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connection_options)
        .await?;

    let schema = include_str!("schema.sql");

    // Execute multiple statements from the schema file
    for statement in schema.split(';') {
        let trimmed = statement.trim();
        if !trimmed.is_empty() {
            sqlx::query(trimmed).execute(&pool).await?;
        }
    }

    Ok(pool)
}