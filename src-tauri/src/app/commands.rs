use crate::app::state::AppState;
use crate::vault::scanner;
use serde::Serialize;
use sqlx::FromRow;
use tauri::State;

// 1. Struct um waveform_data erweitern
#[derive(Debug, Serialize, FromRow)]
pub struct SampleRecord {
    pub id: String,
    pub filename: String,
    pub original_path: String,
    pub duration_ms: i64,
    pub bpm: Option<f64>,
    pub key_signature: Option<String>,
    pub instrument_type: Option<String>,
    pub waveform_data: Option<String>, // Neu hinzugefügt
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
    state: State<'_, AppState>
) -> Result<Vec<SampleRecord>, String> {
    let pool = &state.db;

    let samples = if let Some(t) = filter_type {
        // 2. SELECT Query MIT Filter anpassen
        sqlx::query_as::<_, SampleRecord>(
            "SELECT id, filename, original_path, duration_ms, bpm, key_signature, instrument_type, waveform_data
             FROM samples
             WHERE instrument_type = ?
             ORDER BY imported_at DESC"
        )
            .bind(t)
            .fetch_all(pool)
            .await
    } else {
        // 3. SELECT Query OHNE Filter anpassen
        sqlx::query_as::<_, SampleRecord>(
            "SELECT id, filename, original_path, duration_ms, bpm, key_signature, instrument_type, waveform_data
             FROM samples
             ORDER BY imported_at DESC"
        )
            .fetch_all(pool)
            .await
    }.map_err(|e| e.to_string())?;

    Ok(samples)
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