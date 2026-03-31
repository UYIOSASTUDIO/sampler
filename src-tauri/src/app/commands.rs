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
    pub waveform_data: Option<Vec<u8>>,
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse {
    pub samples: Vec<SampleRecord>,
    pub total_count: i64,
}

// 1. SCAN: Speichert den Pfad und scannt
#[tauri::command]
pub async fn scan_library(
    path: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle, // NEU: AppHandle injiziert
) -> Result<usize, String> {
    let pool = state.db.clone();

    let _ = sqlx::query("INSERT OR IGNORE INTO connected_folders (path) VALUES (?)")
        .bind(&path)
        .execute(&pool)
        .await;

    // AppHandle an den Scanner weitergeben
    scanner::scan_directory(path, pool, app_handle).await
}

// 2. SYNC: Scannt alle gespeicherten Ordner neu ab
#[tauri::command]
pub async fn rescan_all_folders(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle, // NEU: AppHandle injiziert
) -> Result<usize, String> {
    let pool = state.db.clone();
    let mut total_added = 0;

    let folders: Vec<(String,)> = sqlx::query_as("SELECT path FROM connected_folders")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    for (path,) in folders {
        // app_handle.clone(), damit wir ihn für jeden Ordner neu übergeben können
        if let Ok(count) = scanner::scan_directory(path, pool.clone(), app_handle.clone()).await {
            total_added += count;
        }
    }

    Ok(total_added)
}

// 7. GET FOLDERS: Liefert alle aktuell verbundenen Hauptordner für das Settings-UI
#[tauri::command]
pub async fn get_connected_folders(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let pool = &state.db;

    // Wir holen nur die Pfade, sortiert nach dem Hinzufüge-Datum
    let folders: Vec<(String,)> = sqlx::query_as("SELECT path FROM connected_folders ORDER BY added_at DESC")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Wandle das SQL-Tuple in ein sauberes String-Array für Svelte um
    Ok(folders.into_iter().map(|(path,)| path).collect())
}

// 3. READ: Lädt die Samples für das UI (inkl. Full-Text Search)
#[tauri::command]
pub async fn get_samples(
    filter_type: Option<String>,
    search_query: Option<String>,
    page: u32,
    page_size: u32,
    state: State<'_, AppState>
) -> Result<PaginatedResponse, String> {
    let pool = &state.db;

    let limit = page_size as i64;
    let offset = ((page.max(1) - 1) * page_size) as i64;

    let fts_match = search_query
        .filter(|s| !s.trim().is_empty())
        .map(|s| format!("\"{}\"*", s.replace("\"", "")));

    let (samples, total_count) = match (filter_type, fts_match) {
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

// 4. CLEAR: Löscht die gesamte Library
#[tauri::command]
pub async fn clear_database(state: State<'_, AppState>) -> Result<(), String> {
    let pool = &state.db;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM samples").execute(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM samples_fts").execute(&mut *tx).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM connected_folders").execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(())
}

// 5. UN-LINK: Entfernt einen gezielten Ordner und all seine Samples
#[tauri::command]
pub async fn remove_folder(path: String, state: State<'_, AppState>) -> Result<usize, String> {
    let pool = &state.db;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 1. Ordner aus der History entfernen
    sqlx::query("DELETE FROM connected_folders WHERE path = ?")
        .bind(&path)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Alle Samples löschen, deren Pfad mit diesem Ordnerpfad beginnt
    let like_path = format!("{}%", path);
    let result = sqlx::query("DELETE FROM samples WHERE original_path LIKE ?")
        .bind(&like_path)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(result.rows_affected() as usize)
}

// 6. GARBAGE COLLECTION: Gesichert gegen externe Festplatten-Disconnects
#[tauri::command]
pub async fn cleanup_database(state: State<'_, AppState>) -> Result<usize, String> {
    let pool = &state.db;
    let mut removed = 0;

    // Phase 1: Welche Hauptordner sind aktuell physisch erreichbar (online)?
    let folders: Vec<(String,)> = sqlx::query_as("SELECT path FROM connected_folders")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;

    let mut online_folders = Vec::new();
    for (folder,) in folders {
        if std::path::Path::new(&folder).exists() {
            online_folders.push(folder);
        }
    }

    // Wenn gar kein Ordner erreichbar ist, brechen wir direkt sicher ab
    if online_folders.is_empty() {
        return Ok(0);
    }

    // Phase 2: Prüfen der Einzeldateien
    let records: Vec<(String, String)> = sqlx::query_as("SELECT id, original_path FROM samples")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;

    for (id, path) in records {
        // Wir löschen nur Dateileichen, wenn ihr übergeordneter Hauptordner gerade ONLINE ist
        let is_parent_online = online_folders.iter().any(|f| path.starts_with(f));

        if is_parent_online {
            if !std::path::Path::new(&path).exists() {
                let _ = sqlx::query("DELETE FROM samples WHERE id = ?").bind(&id).execute(pool).await;
                removed += 1;
            }
        }
    }

    Ok(removed)
}

// 8. REVEAL IN FINDER: Öffnet den Ordner und markiert das Sample
#[tauri::command]
pub fn reveal_in_finder(path: String) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(&path).spawn();

    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(format!("/select,\"{}\"", path)).spawn();
}