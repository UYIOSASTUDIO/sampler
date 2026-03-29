use crate::app::state::AppState;
use crate::vault::scanner;
use serde::Serialize;
use sqlx::FromRow;
use tauri::State;

// DTO für den Datentransfer zum Frontend
#[derive(Debug, Serialize, FromRow)]
pub struct SampleRecord {
    pub id: String,
    pub filename: String,
    pub original_path: String,
    pub duration_ms: i64,
    pub bpm: Option<f64>,
    pub key_signature: Option<String>,
    pub instrument_type: Option<String>,
}

#[tauri::command]
pub async fn scan_library(
    path: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let pool = state.db.clone();
    scanner::scan_directory(path, pool).await
}

// Neuer Command zum Abrufen der Library
#[tauri::command]
pub async fn get_samples(
    filter_type: Option<String>,
    state: State<'_, AppState>
) -> Result<Vec<SampleRecord>, String> {
    let pool = &state.db;

    let samples = if let Some(t) = filter_type {
        // Wenn ein Filter gesetzt ist, nutze die WHERE-Klausel
        sqlx::query_as::<_, SampleRecord>(
            "SELECT id, filename, original_path, duration_ms, bpm, key_signature, instrument_type
             FROM samples
             WHERE instrument_type = ?
             ORDER BY imported_at DESC"
        )
            .bind(t)
            .fetch_all(pool)
            .await
    } else {
        // Wenn kein Filter gesetzt ist (Option::None), lade alle
        sqlx::query_as::<_, SampleRecord>(
            "SELECT id, filename, original_path, duration_ms, bpm, key_signature, instrument_type
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

    // Löscht alle Einträge aus der samples Tabelle
    sqlx::query("DELETE FROM samples")
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn read_audio_file(path: String) -> Result<Vec<u8>, String> {
    // Liest die Datei asynchron in einen Byte-Vektor
    tokio::fs::read(&path).await.map_err(|e| e.to_string())
}