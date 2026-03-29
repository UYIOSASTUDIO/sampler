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
    pub waveform_data: Option<String>,
}

// NEU: Ein Wrapper-Struct für die paginierte Antwort
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
    scanner::scan_directory(path, pool).await
}

#[tauri::command]
pub async fn get_samples(
    filter_type: Option<String>,
    page: u32,       // NEU: Aktuelle Seite (1-basiert)
    page_size: u32,  // NEU: Anzahl der Elemente pro Seite
    state: State<'_, AppState>
) -> Result<PaginatedResponse, String> {
    let pool = &state.db;

    // SQLite Offset und Limit berechnen
    let limit = page_size as i64;
    let offset = ((page.max(1) - 1) * page_size) as i64;

    let (samples, total_count) = if let Some(t) = filter_type {
        // Zuerst die Gesamtanzahl für diesen Filter abfragen
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM samples WHERE instrument_type = ?")
            .bind(&t)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

        // Dann nur die spezifische Seite laden
        let items = sqlx::query_as::<_, SampleRecord>(
            "SELECT id, filename, original_path, duration_ms, bpm, key_signature, instrument_type, waveform_data
             FROM samples
             WHERE instrument_type = ?
             ORDER BY imported_at DESC
             LIMIT ? OFFSET ?"
        )
            .bind(&t)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

        (items, count.0)
    } else {
        // Gesamtanzahl ohne Filter
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM samples")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

        // Spezifische Seite ohne Filter laden
        let items = sqlx::query_as::<_, SampleRecord>(
            "SELECT id, filename, original_path, duration_ms, bpm, key_signature, instrument_type, waveform_data
             FROM samples
             ORDER BY imported_at DESC
             LIMIT ? OFFSET ?"
        )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

        (items, count.0)
    };

    Ok(PaginatedResponse { samples, total_count })
}

#[tauri::command]
pub async fn clear_database(state: State<'_, AppState>) -> Result<(), String> {
    let pool = &state.db;

    sqlx::query("DELETE FROM samples")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn read_audio_file(path: String) -> Result<Vec<u8>, String> {
    tokio::fs::read(&path).await.map_err(|e| e.to_string())
}