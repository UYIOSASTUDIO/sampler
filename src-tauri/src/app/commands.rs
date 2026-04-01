use crate::app::state::AppState;
use crate::vault::scanner;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite};
use tauri::State;

use rodio::{Sink, buffer::SamplesBuffer};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use hound::{WavReader, SampleFormat};
use ssstretch::Stretch;

#[derive(Debug, Serialize, FromRow)]
pub struct SampleRecord {
    pub id: String,
    pub filename: String,
    pub original_path: String,
    pub duration_ms: i64,
    pub bpm: Option<f64>,
    pub key_signature: Option<String>,
    pub instrument_type: Option<String>,
    pub tags: String,
    pub waveform_data: Option<Vec<u8>>,
    pub is_liked: bool, // NEU
}

// ==========================================
// FILTER STRUCTS
// ==========================================
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BpmFilter {
    pub is_range: bool,
    pub exact: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterPayload {
    pub instruments: Vec<String>,
    pub genres: Vec<String>,
    pub keys: Vec<String>,
    pub formats: Vec<String>,
    pub bpm: BpmFilter,
    pub tag_match_mode: String,
    pub only_liked: bool,
    pub collection_id: Option<i64>, // NEU
}
#[derive(Debug, Serialize, FromRow)]
pub struct TagResponse {
    pub category: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse {
    pub samples: Vec<SampleRecord>,
    pub total_count: i64,
    pub available_tags: Vec<TagResponse>,
}

// 1. SCAN: Speichert den Pfad und scannt
#[tauri::command]
pub async fn scan_library(
    path: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<usize, String> {
    let pool = state.db.clone();

    let _ = sqlx::query("INSERT OR IGNORE INTO connected_folders (path) VALUES (?)")
        .bind(&path)
        .execute(&pool)
        .await;

    scanner::scan_directory(path, pool, app_handle).await
}

// 2. SYNC: Scannt alle gespeicherten Ordner neu ab
#[tauri::command]
pub async fn rescan_all_folders(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<usize, String> {
    let pool = state.db.clone();
    let mut total_added = 0;

    let folders: Vec<(String,)> = sqlx::query_as("SELECT path FROM connected_folders")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    for (path,) in folders {
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

    let folders: Vec<(String,)> = sqlx::query_as("SELECT path FROM connected_folders ORDER BY added_at DESC")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(folders.into_iter().map(|(path,)| path).collect())
}

// ==========================================
// DYNAMIC QUERY BUILDER HELPER
// ==========================================
fn build_where_clause<'args>(
    builder: &mut QueryBuilder<'args, Sqlite>,
    filters: &'args FilterPayload,
    fts_match: &'args Option<String>,
) {
    #[allow(unused_assignments)]
    let mut has_where = false;

    macro_rules! push_and {
        () => {
            if has_where { builder.push(" AND "); }
            else { builder.push(" WHERE "); has_where = true; }
        }
    }

    // NEU: Favoriten-Filter zwingend anwenden, wenn aktiv
    if filters.only_liked {
        push_and!();
        builder.push("s.is_liked = 1");
    }

    if let Some(ref s) = fts_match {
        push_and!();
        builder.push("fts.samples_fts MATCH ");
        builder.push_bind(s);
    }

    let all_tags: Vec<&String> = filters.instruments.iter()
        .chain(filters.genres.iter())
        .chain(filters.formats.iter())
        .collect();

    if !all_tags.is_empty() {
        if filters.tag_match_mode == "OR" {
            push_and!();
            builder.push("EXISTS (SELECT 1 FROM json_each(s.tags) WHERE json_extract(value, '$.value') IN (");
            let mut sep = builder.separated(", ");
            // DER FIX: &t entpackt die Referenz, wodurch sie wieder lang genug lebt!
            for &t in &all_tags { sep.push_bind(t); }
            builder.push("))");
        } else {
            for &t in &all_tags {
                push_and!();
                builder.push("EXISTS (SELECT 1 FROM json_each(s.tags) WHERE json_extract(value, '$.value') = ");
                builder.push_bind(t);
                builder.push(")");
            }
        }
    }

    // NEU: Intelligente Key-Logik (Erlaubt Suche nach "min" oder "maj" als Wildcard)
    if !filters.keys.is_empty() {
        push_and!();
        builder.push("(");
        let mut first = true;
        for v in &filters.keys {
            if !first { builder.push(" OR "); }
            first = false;

            if v == "min" || v == "maj" {
                // Wildcard-Suche: Findet "C min", "F# min", etc.
                builder.push("s.key_signature LIKE ");
                builder.push_bind(format!("%{}", v));
            } else {
                // Exakte Suche: Findet exakt "C min" oder nur "C"
                builder.push("s.key_signature = ");
                builder.push_bind(v);
            }
        }
        builder.push(")");
    }

    if filters.bpm.is_range {
        if let Some(min) = filters.bpm.min { push_and!(); builder.push("s.bpm >= "); builder.push_bind(min); }
        if let Some(max) = filters.bpm.max { push_and!(); builder.push("s.bpm <= "); builder.push_bind(max); }
    } else if let Some(exact) = filters.bpm.exact {
        push_and!(); builder.push("s.bpm = "); builder.push_bind(exact);
    }
}

// 3. READ: Lädt die Samples und berechnet Facetten
#[tauri::command]
pub async fn get_samples(
    search_query: Option<String>,
    page: u32,
    page_size: u32,
    filters: FilterPayload,
    sort_field: String, // NEU: name, type, pack oder random
    sort_order: String, // NEU: asc oder desc
    state: State<'_, AppState>
) -> Result<PaginatedResponse, String> {
    let pool = &state.db;

    let limit = page_size as i64;
    let offset = ((page.max(1) - 1) * page_size) as i64;

    // Wir bereiten die Suche so vor, dass sie nur Dateinamen und Tags scannt
    let fts_match = search_query
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            let sanitized = s.replace("\"", "");
            format!("filename:\"{sanitized}\"* OR tags:\"{sanitized}\"*")
        });

    // 1. TOTAL COUNT
    let mut count_query = QueryBuilder::new("SELECT COUNT(*) FROM samples s ");
    if fts_match.is_some() { count_query.push("INNER JOIN samples_fts fts ON s.id = fts.id "); }

    // DER FIX: Der Collection-Join MUSS auch hier beim Zählen rein!
    if let Some(c_id) = filters.collection_id {
        count_query.push("INNER JOIN collection_samples cs ON s.id = cs.sample_id AND cs.collection_id = ");
        count_query.push_bind(c_id);
        count_query.push(" ");
    }

    build_where_clause(&mut count_query, &filters, &fts_match);
    let total_count: i64 = count_query.build_query_scalar().fetch_one(pool).await.unwrap_or(0);

    // 2. DYNAMISCHE SAMPLES
    let mut samples_query = QueryBuilder::new("SELECT s.id, s.filename, s.original_path, s.duration_ms, s.bpm, s.key_signature, s.instrument_type, s.tags, s.waveform_data, s.is_liked FROM samples s ");

    if fts_match.is_some() { samples_query.push("INNER JOIN samples_fts fts ON s.id = fts.id "); }

    // NEU: Wenn eine Collection gefiltert wird, joine die relationale Tabelle
    if let Some(c_id) = filters.collection_id {
        samples_query.push("INNER JOIN collection_samples cs ON s.id = cs.sample_id AND cs.collection_id = ");
        samples_query.push_bind(c_id);
        samples_query.push(" ");
    }

    build_where_clause(&mut samples_query, &filters, &fts_match);

    // NEU: Dynamische ORDER BY Klausel
    match sort_field.as_str() {
        "name" => { samples_query.push(" ORDER BY s.filename "); }
        "type" => { samples_query.push(" ORDER BY s.instrument_type "); }
        "pack" => { samples_query.push(" ORDER BY s.original_path "); }
        "random" => { samples_query.push(" ORDER BY RANDOM() "); }
        _ => { samples_query.push(" ORDER BY s.filename "); }
    }

    // Random hat kein ASC/DESC, alles andere schon
    if sort_field != "random" {
        if sort_order == "desc" {
            samples_query.push(" DESC LIMIT ");
        } else {
            samples_query.push(" ASC LIMIT ");
        }
    } else {
        samples_query.push(" LIMIT ");
    }

    samples_query.push_bind(limit);
    samples_query.push(" OFFSET ");
    samples_query.push_bind(offset);

    let samples = samples_query.build_query_as::<SampleRecord>().fetch_all(pool).await.map_err(|e| e.to_string())?;

    // 3. DYNAMISCHE FACETTEN-SUCHE
    let mut tags_query = QueryBuilder::new("SELECT DISTINCT json_extract(value, '$.category') as category, json_extract(value, '$.value') as value FROM samples s ");
    if fts_match.is_some() { tags_query.push("INNER JOIN samples_fts fts ON s.id = fts.id "); }
    tags_query.push(", json_each(s.tags) ");
    build_where_clause(&mut tags_query, &filters, &fts_match);

    let mut tags_vec = tags_query.build_query_as::<TagResponse>().fetch_all(pool).await.unwrap_or_default();

    tags_vec.sort_by(|a, b| {
        let prio = |cat: &str, val: &str| -> i32 {
            if cat == val { return 1; }
            match cat {
                "Drums" | "Percussion" | "Bass" | "Synth" | "Keys" | "Guitar" | "Strings" | "Vocals" | "Brass and Woodwinds" | "FX" => 2,
                "Format" => 3, "Genre" => 4, "Character" => 5, _ => 6,
            }
        };
        prio(&a.category, &a.value).cmp(&prio(&b.category, &b.value))
            .then_with(|| a.category.cmp(&b.category))
            .then_with(|| a.value.cmp(&b.value))
    });

    Ok(PaginatedResponse { samples, total_count, available_tags: tags_vec })
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

    sqlx::query("DELETE FROM connected_folders WHERE path = ?")
        .bind(&path)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

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

    let folders: Vec<(String,)> = sqlx::query_as("SELECT path FROM connected_folders")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;

    let mut online_folders = Vec::new();
    for (folder,) in folders {
        if std::path::Path::new(&folder).exists() {
            online_folders.push(folder);
        }
    }

    if online_folders.is_empty() {
        return Ok(0);
    }

    let records: Vec<(String, String)> = sqlx::query_as("SELECT id, original_path FROM samples")
        .fetch_all(pool).await.map_err(|e| e.to_string())?;

    for (id, path) in records {
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

// 9. TOGGLE LIKE: Ändert den Favoriten-Status in der Datenbank
#[tauri::command]
pub async fn toggle_sample_like(id: String, is_liked: bool, state: State<'_, AppState>) -> Result<(), String> {
    let pool = &state.db;

    sqlx::query("UPDATE samples SET is_liked = ? WHERE id = ?")
        .bind(is_liked)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Debug, Serialize, sqlx::FromRow)] // FIX: sqlx::FromRow hinzugefügt!
#[serde(rename_all = "camelCase")]
pub struct CollectionRecord {
    pub id: i64,
    pub name: String,
}

#[tauri::command]
pub async fn get_collections(state: State<'_, AppState>) -> Result<Vec<CollectionRecord>, String> {
    // FIX: query_as::<_, CollectionRecord> (ohne Ausrufezeichen) überspringt den Compile-Time Check!
    sqlx::query_as::<_, CollectionRecord>("SELECT id, name FROM collections ORDER BY name ASC")
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_collection(name: String, state: State<'_, AppState>) -> Result<i64, String> {
    let result = sqlx::query("INSERT INTO collections (name) VALUES (?)")
        .bind(name).execute(&state.db).await.map_err(|e| e.to_string())?;
    Ok(result.last_insert_rowid())
}

#[tauri::command]
pub async fn add_to_collection(collection_id: i64, sample_ids: Vec<String>, state: State<'_, AppState>) -> Result<(), String> {
    let mut builder = QueryBuilder::new("INSERT OR IGNORE INTO collection_samples (collection_id, sample_id) ");
    builder.push_values(sample_ids, |mut b, id| {
        b.push_bind(collection_id).push_bind(id);
    });
    builder.build().execute(&state.db).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn bulk_toggle_like(sample_ids: Vec<String>, is_liked: bool, state: State<'_, AppState>) -> Result<(), String> {
    let mut builder = QueryBuilder::new("UPDATE samples SET is_liked = ");
    builder.push_bind(is_liked);
    builder.push(" WHERE id IN (");
    let mut sep = builder.separated(", ");
    for id in sample_ids { sep.push_bind(id); }
    builder.push(")");
    builder.build().execute(&state.db).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadataPayload {
    pub id: String,
    pub filename: String,
    pub bpm: Option<f64>,
    pub key_signature: Option<String>,
    pub tags: String,
}

#[tauri::command]
pub async fn update_sample_metadata(payload: UpdateMetadataPayload, state: State<'_, AppState>) -> Result<(), String> {
    sqlx::query("UPDATE samples SET filename = ?, bpm = ?, key_signature = ?, tags = ? WHERE id = ?")
        .bind(payload.filename)
        .bind(payload.bpm)
        .bind(payload.key_signature)
        .bind(payload.tags)
        .bind(payload.id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Debug, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TagRecord {
    pub category: String,
    pub value: String,
}

#[tauri::command]
pub async fn get_user_tags(state: State<'_, AppState>) -> Result<Vec<TagRecord>, String> {
    sqlx::query_as::<_, TagRecord>("SELECT category, value FROM user_tags ORDER BY value ASC")
        .fetch_all(&state.db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_user_tag(category: String, value: String, state: State<'_, AppState>) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO user_tags (category, value) VALUES (?, ?)")
        .bind(category).bind(value).execute(&state.db).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_user_tag(value: String, state: State<'_, AppState>) -> Result<(), String> {
    // 1. Den Tag global aus der user_tags Tabelle entfernen
    sqlx::query("DELETE FROM user_tags WHERE value = ?")
        .bind(&value).execute(&state.db).await.map_err(|e| e.to_string())?;

    // Wir brauchen ein temporäres Struct, um die betroffenen Samples zu laden
    #[derive(sqlx::FromRow)]
    struct SampleTagRow {
        id: String,
        tags: String,
    }

    // 2. Suche alle Samples, die diesen Tag aktuell verwenden
    let pattern = format!("%\"value\":\"{}\"%", value);
    let samples = sqlx::query_as::<_, SampleTagRow>("SELECT id, tags FROM samples WHERE tags LIKE ?")
        .bind(pattern)
        .fetch_all(&state.db).await.map_err(|e| e.to_string())?;

    // 3. Cascade Delete: Den Tag aus der JSON jedes Samples entfernen
    for sample in samples {
        if let Ok(mut parsed_tags) = serde_json::from_str::<Vec<serde_json::Value>>(&sample.tags) {

            // Filtere das Array: Behalte alles, ABER NICHT den gelöschten Tag
            parsed_tags.retain(|t| {
                t.get("value").and_then(|v| v.as_str()) != Some(&value)
            });

            // Speichere die gesäuberte JSON zurück in die Datenbank
            if let Ok(new_tags_string) = serde_json::to_string(&parsed_tags) {
                sqlx::query("UPDATE samples SET tags = ? WHERE id = ?")
                    .bind(new_tags_string)
                    .bind(sample.id)
                    .execute(&state.db).await.ok(); // .ok() ignoriert einzelne Fehler, damit der Loop weiterläuft
            }
        }
    }

    Ok(())
}

use crate::vault::taxonomy::TaxonomyEngine;

#[tauri::command]
pub async fn get_all_available_tags(state: State<'_, AppState>) -> Result<Vec<TagRecord>, String> {
    let mut all_tags = Vec::new();

    // 1. Hole die festen System-Tags aus der Taxonomy Engine
    let taxonomy = TaxonomyEngine::new();
    for rule in taxonomy.rules {
        // Vermeide Duplikate im UI (z.B. weil "hat" und "hihat" beide zum Tag "Hi-Hat" führen)
        if !all_tags.iter().any(|t: &TagRecord| t.value == rule.value && t.category == rule.category) {
            all_tags.push(TagRecord {
                category: rule.category.to_string(),
                value: rule.value.to_string(),
            });
        }
    }

    // 2. Hole die flexiblen User-Tags aus der Datenbank
    let user_tags = sqlx::query_as::<_, TagRecord>("SELECT category, value FROM user_tags ORDER BY value ASC")
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Füge beide Listen zusammen
    all_tags.extend(user_tags);

    Ok(all_tags)
}

// FIX: current_sink nutzt jetzt Arc<Sink>!
pub struct AudioState {
    pub stream_handle: rodio::OutputStreamHandle,
    pub current_sink: Mutex<Option<Arc<Sink>>>,
    pub playback_id: Arc<AtomicUsize>,
}

#[tauri::command]
pub fn stop_audio(state: State<'_, AudioState>) {
    let mut current_sink = state.current_sink.lock().unwrap();
    if let Some(sink) = current_sink.take() {
        sink.stop();
    }
    state.playback_id.fetch_add(1, Ordering::SeqCst);
}

#[tauri::command]
pub fn set_audio_volume(volume: f32, state: State<'_, AudioState>) {
    if let Some(sink) = state.current_sink.lock().unwrap().as_ref() {
        sink.set_volume(volume);
    }
}

#[tauri::command]
pub fn play_audio(
    file_path: String,
    semitones: f32,
    volume: f32,
    state: State<'_, AudioState>,
) -> Result<(), String> {
    let new_id = state.playback_id.fetch_add(1, Ordering::SeqCst) + 1;

    // FIX: Wir wrappen den Sink in einen Arc, um ihn sicher zu teilen
    let sink = Arc::new(Sink::try_new(&state.stream_handle).map_err(|e| e.to_string())?);
    sink.set_volume(volume);

    {
        let mut current_sink = state.current_sink.lock().unwrap();
        if let Some(old_sink) = current_sink.take() {
            old_sink.stop();
        }
        *current_sink = Some(Arc::clone(&sink)); // Wir klonen nur die Referenz!
    }

    let id_clone = state.playback_id.clone();

    // Der Worker-Thread erbt die original-Referenz des Sinks
    std::thread::spawn(move || {
        let mut reader = match WavReader::open(&file_path) {
            Ok(r) => r,
            Err(_) => return,
        };

        let spec = reader.spec();
        let channels = spec.channels as usize;
        let sample_rate = spec.sample_rate;

        let raw_samples: Vec<f32> = match spec.sample_format {
            SampleFormat::Int => {
                let max_val = 1_f32 / (1_i64 << (spec.bits_per_sample - 1)) as f32;
                if spec.bits_per_sample <= 16 {
                    reader.samples::<i16>().map(|s| s.unwrap_or(0) as f32 * max_val).collect()
                } else {
                    reader.samples::<i32>().map(|s| s.unwrap_or(0) as f32 * max_val).collect()
                }
            }
            SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        };

        let total_frames = raw_samples.len() / channels;
        let mut input_channels = vec![vec![0.0; total_frames]; channels];
        for (i, &sample) in raw_samples.iter().enumerate() {
            input_channels[i % channels][i / channels] = sample;
        }

        let mut stretch = Stretch::new();
        stretch.preset_default(channels as i32, sample_rate as f32);
        stretch.set_transpose_semitones(semitones, None);

        let chunk_size = 4096;
        let mut frames_processed = 0;

        loop {
            if id_clone.load(Ordering::SeqCst) != new_id { break; }
            if sink.len() > 2 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }

            let frames_left = total_frames - frames_processed;
            if frames_left == 0 { break; }
            let current_chunk = frames_left.min(chunk_size);

            let mut in_chunk = vec![vec![0.0f32; current_chunk]; channels];
            for ch in 0..channels {
                in_chunk[ch].copy_from_slice(&input_channels[ch][frames_processed..frames_processed + current_chunk]);
            }

            let mut out_chunk = vec![vec![0.0f32; current_chunk]; channels];
            stretch.process_vec(&in_chunk, current_chunk as i32, &mut out_chunk, current_chunk as i32);

            let mut out_interleaved = Vec::with_capacity(current_chunk * channels);
            for frame in 0..current_chunk {
                for ch in 0..channels {
                    out_interleaved.push(out_chunk[ch][frame]);
                }
            }

            let buffer = SamplesBuffer::new(channels as u16, sample_rate, out_interleaved);
            sink.append(buffer);
            frames_processed += current_chunk;
        }
    });

    Ok(())
}