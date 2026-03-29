use crate::app::state::AppState;
use crate::vault::scanner;
use serde::Serialize;
use sqlx::FromRow;
use tauri::State;

#[derive(Debug, Serialize, FromRow)]
pub struct SampleRecord {
    pub id: String,
    pub filename: String,
    pub original_path: String,
    pub duration_ms: i64,
    pub bpm: Option<f64>,
    pub key_signature: Option<String>,
    pub instrument_type: Option<String>,
    pub waveform_data: Option<Vec<u8>>, // GEÄNDERT: Von Option<String> zu Option<Vec<u8>>
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse {
    pub samples: Vec<SampleRecord>,
    pub total_count: i64,
}

#[tauri::command]
pub async fn scan_library(
    path: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let pool = state.db.clone();

    // 1. Der normale High-Performance Batch-Scan
    let result = scanner::scan_directory(path.clone(), pool.clone()).await;

    // 2. Den "Ghost"-Watcher für die Zukunft starten
    if result.is_ok() {
        crate::vault::watcher::start_background_watcher(path, pool);
    }

    result
}

#[tauri::command]
pub async fn get_samples(
    filter_type: Option<String>,
    search_query: Option<String>, // NEU: Der Full-Text Search Parameter
    page: u32,
    page_size: u32,
    state: State<'_, AppState>
) -> Result<PaginatedResponse, String> {
    let pool = &state.db;

    let limit = page_size as i64;
    let offset = ((page.max(1) - 1) * page_size) as i64;

    // Such-String formatieren: Entfernt störende Quotes und hängt einen Wildcard (*) an.
    // Aus "808 Kick" wird "\"808 Kick\"*" -> FTS5 Prefix Search.
    let fts_match = search_query
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("\"{}\"*", s.replace("\"", "")));

    // Wir nutzen explizite Branches für absolute Typsicherheit in SQLx
    let (samples, total_count) = match (filter_type, fts_match) {

        // Fall 1: Suchen UND Filtern
        (Some(f), Some(s)) => {
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM samples s
                 INNER JOIN samples_fts fts ON s.id = fts.id
                 WHERE fts.samples_fts MATCH ? AND s.instrument_type = ?"
            )
                .bind(&s).bind(&f).fetch_one(pool).await.map_err(|e| e.to_string())?;

            let items = sqlx::query_as::<_, SampleRecord>(
                "SELECT s.id, s.filename, s.original_path, s.duration_ms, s.bpm, s.key_signature, s.instrument_type, s.waveform_data
                 FROM samples s
                 INNER JOIN samples_fts fts ON s.id = fts.id
                 WHERE fts.samples_fts MATCH ? AND s.instrument_type = ?
                 ORDER BY s.imported_at DESC LIMIT ? OFFSET ?"
            )
                .bind(&s).bind(&f).bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| e.to_string())?;

            (items, count.0)
        },

        // Fall 2: NUR Suchen
        (None, Some(s)) => {
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM samples s
                 INNER JOIN samples_fts fts ON s.id = fts.id
                 WHERE fts.samples_fts MATCH ?"
            )
                .bind(&s).fetch_one(pool).await.map_err(|e| e.to_string())?;

            let items = sqlx::query_as::<_, SampleRecord>(
                "SELECT s.id, s.filename, s.original_path, s.duration_ms, s.bpm, s.key_signature, s.instrument_type, s.waveform_data
                 FROM samples s
                 INNER JOIN samples_fts fts ON s.id = fts.id
                 WHERE fts.samples_fts MATCH ?
                 ORDER BY s.imported_at DESC LIMIT ? OFFSET ?"
            )
                .bind(&s).bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| e.to_string())?;

            (items, count.0)
        },

        // Fall 3: NUR Filtern
        (Some(f), None) => {
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM samples WHERE instrument_type = ?")
                .bind(&f).fetch_one(pool).await.map_err(|e| e.to_string())?;

            let items = sqlx::query_as::<_, SampleRecord>(
                "SELECT id, filename, original_path, duration_ms, bpm, key_signature, instrument_type, waveform_data
                 FROM samples WHERE instrument_type = ?
                 ORDER BY imported_at DESC LIMIT ? OFFSET ?"
            )
                .bind(&f).bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| e.to_string())?;

            (items, count.0)
        },

        // Fall 4: Weder Suche noch Filter (Standardansicht)
        (None, None) => {
            let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM samples")
                .fetch_one(pool).await.map_err(|e| e.to_string())?;

            let items = sqlx::query_as::<_, SampleRecord>(
                "SELECT id, filename, original_path, duration_ms, bpm, key_signature, instrument_type, waveform_data
                 FROM samples ORDER BY imported_at DESC LIMIT ? OFFSET ?"
            )
                .bind(limit).bind(offset).fetch_all(pool).await.map_err(|e| e.to_string())?;

            (items, count.0)
        }
    };

    Ok(PaginatedResponse { samples, total_count })
}

#[tauri::command]
pub async fn clear_database(state: State<'_, AppState>) -> Result<(), String> {
    let pool = &state.db;

    // Enterprise Clear: Wir verpacken das Löschen in eine Transaktion,
    // damit SQLite nicht bei jeder gelöschten Zeile auf die SSD schreibt.
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM samples").execute(&mut *tx).await.map_err(|e| e.to_string())?;
    // Der Trigger löscht fts_samples automatisch mit, aber wir sichern das hier explizit ab:
    sqlx::query("DELETE FROM samples_fts").execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(())
}