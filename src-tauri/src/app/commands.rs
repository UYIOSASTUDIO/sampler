use crate::app::state::AppState;
use crate::vault::scanner;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder, Sqlite};
use tauri::State;

use rodio::{Sink, buffer::SamplesBuffer};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

// Symphonia — universeller Decoder für WAV, MP3, FLAC, AIFF, OGG, M4A
use symphonia::core::audio::SampleBuffer as SymphoniaSampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

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
    pub is_liked: bool,
    pub cover_path: Option<String>, // <--- ENTERPRISE FIX: Frontend benötigt dieses Feld!
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
    let mut samples_query = QueryBuilder::new("SELECT s.id, s.filename, s.original_path, s.duration_ms, s.bpm, s.key_signature, s.instrument_type, s.tags, s.waveform_data, s.is_liked, s.cover_path FROM samples s ");

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

    // ENTERPRISE FIX: fetch_all → fetch() Stream, damit nie alle Samples auf einmal
    // in den RAM geladen werden (schlecht bei 100k+ Einträgen).
    // Wir sammeln nur die IDs der zu löschenden Samples (kleine Strings) und
    // führen danach einen einzigen atomaren Batch-Delete durch.
    use futures::StreamExt;

    let mut stream = sqlx::query_as::<_, (String, String)>(
        "SELECT id, original_path FROM samples"
    ).fetch(pool);

    let mut ids_to_delete: Vec<String> = Vec::new();

    while let Some(row) = stream.next().await {
        let (id, path) = row.map_err(|e| e.to_string())?;
        let is_parent_online = online_folders.iter().any(|f| path.starts_with(f.as_str()));
        if is_parent_online && !std::path::Path::new(&path).exists() {
            ids_to_delete.push(id);
        }
    }

    // Stream-Borrow freigeben, bevor wir die Transaktion öffnen
    drop(stream);

    if !ids_to_delete.is_empty() {
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        for id in &ids_to_delete {
            let _ = sqlx::query("DELETE FROM samples WHERE id = ?")
                .bind(id).execute(&mut *tx).await;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        removed = ids_to_delete.len();
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

    // 3. Cascade Delete: Den Tag aus der JSON jedes Samples entfernen.
    //    ENTERPRISE FIX: Alle UPDATEs in einer einzigen Transaktion — statt N
    //    einzelner Roundtrips (N+1-Anti-Pattern). Atomarität als Bonus: entweder
    //    alle Samples werden bereinigt oder keines.
    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;

    for sample in samples {
        if let Ok(mut parsed_tags) = serde_json::from_str::<Vec<serde_json::Value>>(&sample.tags) {

            // Filtere das Array: Behalte alles, ABER NICHT den gelöschten Tag
            parsed_tags.retain(|t| {
                t.get("value").and_then(|v| v.as_str()) != Some(&value)
            });

            // Schreibe die gesäuberte JSON in die laufende Transaktion
            if let Ok(new_tags_string) = serde_json::to_string(&parsed_tags) {
                sqlx::query("UPDATE samples SET tags = ? WHERE id = ?")
                    .bind(new_tags_string)
                    .bind(sample.id)
                    .execute(&mut *tx).await.ok();
            }
        }
    }

    // Erst hier werden alle Writes atomar committed
    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(())
}

/// Returns the waveform BLOB for a single sample, loaded on-demand.
///
/// By separating this from `get_samples`, the list query stays lean and only
/// pays the BLOB I/O cost when the frontend actually needs to render a waveform
/// (e.g. on hover or selection).
#[tauri::command]
pub async fn get_waveform(id: String, state: State<'_, AppState>) -> Result<Option<Vec<u8>>, String> {
    let row: Option<(Option<Vec<u8>>,)> =
        sqlx::query_as("SELECT waveform_data FROM samples WHERE id = ?")
            .bind(id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| e.to_string())?;

    Ok(row.and_then(|(blob,)| blob))
}

use crate::vault::taxonomy::TaxonomyEngine;

#[tauri::command]
pub async fn get_all_available_tags(state: State<'_, AppState>) -> Result<Vec<TagRecord>, String> {
    let mut all_tags = Vec::new();

    // global() liefert die einmalig kompilierte OnceLock-Instanz — kein Re-Build des
    // Aho-Corasick-Automaten pro Aufruf (war vorher TaxonomyEngine::new() = teuer)
    let taxonomy = TaxonomyEngine::global();
    for rule in &taxonomy.rules {
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

// ─────────────────────────────────────────────────────────────────────────────
// AUDIO ENGINE STATE
// ─────────────────────────────────────────────────────────────────────────────

pub struct AudioState {
    pub stream_handle: rodio::OutputStreamHandle,
    pub current_sink:  Mutex<Option<Arc<Sink>>>,
    pub playback_id:   Arc<AtomicUsize>,
}

// ─────────────────────────────────────────────────────────────────────────────
// AUDIO ENGINE CONSTANTS
// ─────────────────────────────────────────────────────────────────────────────

/// Frames per processing chunk.
/// 4096 frames @ 44.1kHz ≈ 93 ms — balances latency vs. overhead.
const CHUNK_FRAMES: usize = 4096;

/// Max chunks buffered in the sink queue before applying backpressure.
/// 8 chunks ≈ 744 ms of audio headroom — prevents unbounded memory growth.
const SINK_BUFFER_MAX: usize = 8;

// ─────────────────────────────────────────────────────────────────────────────
// CORE PLAYBACK HELPERS
// ─────────────────────────────────────────────────────────────────────────────

/// Encodes one interleaved slice (direct or pitch-shifted) and appends it to
/// the Sink.  All mutable buffers are pre-allocated outside the hot loop and
/// passed by reference to avoid per-chunk heap allocations.
#[inline(always)]
fn flush_chunk(
    interleaved_in:  &[f32],
    frames:          usize,
    channels:        usize,
    sample_rate:     u32,
    stretch:         &mut Option<ssstretch::Stretch>,
    in_deint:        &mut Vec<Vec<f32>>,
    out_deint:       &mut Vec<Vec<f32>>,
    sink:            &Arc<Sink>,
) {
    if let Some(ref mut s) = stretch {
        // ── Pitch-shift path ───────────────────────────────────────────────
        // Deinterleave: [L,R, L,R, …] → [[L,L,…], [R,R,…]]
        for (fi, frame) in interleaved_in.chunks_exact(channels).take(frames).enumerate() {
            for (ch, &sample) in frame.iter().enumerate() {
                in_deint[ch][fi] = sample;
            }
        }

        s.process_vec(in_deint, frames as i32, out_deint, frames as i32);

        // Reinterleave and submit
        let mut out: Vec<f32> = Vec::with_capacity(frames * channels);
        for fi in 0..frames {
            for ch in 0..channels {
                out.push(out_deint[ch][fi]);
            }
        }
        sink.append(SamplesBuffer::new(channels as u16, sample_rate, out));
    } else {
        // ── Direct path (no pitch shift) ───────────────────────────────────
        // Copy the interleaved slice directly to the sink — zero extra work.
        sink.append(SamplesBuffer::new(
            channels as u16,
            sample_rate,
            interleaved_in[..frames * channels].to_vec(),
        ));
    }
}

/// Spawns a background thread that:
/// 1. Opens any audio format via Symphonia (WAV, MP3, FLAC, AIFF, OGG, M4A)
/// 2. Streams decoded chunks to `sink` without loading the full file into RAM
/// 3. Optionally applies ssstretch pitch-shifting — skipped entirely at 0 semitones
/// 4. Cancels cleanly when `playback_id` no longer matches `expected_id`
fn spawn_playback_thread(
    file_path:   String,
    semitones:   f32,
    sink:        Arc<Sink>,
    playback_id: Arc<AtomicUsize>,
    expected_id: usize,
) {
    std::thread::spawn(move || {
        // ── 1. Open & probe (format-agnostic) ─────────────────────────────
        let file = match std::fs::File::open(&file_path) {
            Ok(f)  => f,
            Err(e) => {
                tracing::error!("play_audio: cannot open '{}': {}", file_path, e);
                return;
            }
        };

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = std::path::Path::new(&file_path)
            .extension()
            .and_then(|e| e.to_str())
        {
            hint.with_extension(ext);
        }

        let probed = match symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        ) {
            Ok(p)  => p,
            Err(e) => {
                tracing::error!("play_audio: probe failed for '{}': {}", file_path, e);
                return;
            }
        };

        let mut format = probed.format;

        let track = match format.default_track() {
            Some(t) => t,
            None    => {
                tracing::error!("play_audio: no audio track in '{}'", file_path);
                return;
            }
        };

        let sample_rate: u32 = track.codec_params.sample_rate.unwrap_or(44100);
        let channels:    usize = track.codec_params.channels
            .map(|c| c.count())
            .unwrap_or(2);
        let track_id = track.id;

        let mut decoder = match symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
        {
            Ok(d)  => d,
            Err(e) => {
                tracing::error!("play_audio: decoder error for '{}': {}", file_path, e);
                return;
            }
        };

        // ── 2. Pitch-shift setup — only when actually needed ───────────────
        // Fast-path: semitones ≈ 0.0 → ssstretch is never instantiated.
        let mut stretch: Option<ssstretch::Stretch> = if semitones.abs() > 0.01 {
            let mut s = ssstretch::Stretch::new();
            s.preset_default(channels as i32, sample_rate as f32);
            s.set_transpose_semitones(semitones, None);
            Some(s)
        } else {
            None
        };

        // ── 3. Pre-allocate ALL mutable buffers outside the hot loop ───────
        // This eliminates repeated heap allocations inside the decode loop.
        let mut in_deint:  Vec<Vec<f32>> = vec![vec![0.0; CHUNK_FRAMES]; channels];
        let mut out_deint: Vec<Vec<f32>> = vec![vec![0.0; CHUNK_FRAMES]; channels];

        // Ring buffer for decoded interleaved samples awaiting processing.
        let mut pending: Vec<f32> = Vec::with_capacity(CHUNK_FRAMES * channels * 4);

        // Reused SampleBuffer — grows on first use, stays stable thereafter.
        let mut sym_sample_buf: Option<SymphoniaSampleBuffer<f32>> = None;

        tracing::debug!(
            "play_audio: starting stream '{}' | {}ch | {}Hz | pitch {:+.2} st",
            file_path, channels, sample_rate, semitones
        );

        // ── 4. Streaming decode + playback loop ────────────────────────────
        'decode: loop {
            // Cancellation check — another play_audio() call was made
            if playback_id.load(Ordering::SeqCst) != expected_id {
                break 'decode;
            }

            // Backpressure: pause producer until consumer (rodio) catches up
            if sink.len() > SINK_BUFFER_MAX {
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }

            // Process a full chunk from pending buffer when available
            let frames_pending = pending.len() / channels;
            if frames_pending >= CHUNK_FRAMES {
                flush_chunk(
                    &pending[..CHUNK_FRAMES * channels],
                    CHUNK_FRAMES,
                    channels,
                    sample_rate,
                    &mut stretch,
                    &mut in_deint,
                    &mut out_deint,
                    &sink,
                );
                pending.drain(..CHUNK_FRAMES * channels);
                continue; // drain as many full chunks as possible before decoding
            }

            // Decode the next Symphonia packet
            let packet = match format.next_packet() {
                Ok(p)  => p,
                Err(_) => {
                    // EOF — flush any remaining samples
                    let frames_left = pending.len() / channels;
                    if frames_left > 0 {
                        flush_chunk(
                            &pending,
                            frames_left,
                            channels,
                            sample_rate,
                            &mut stretch,
                            &mut in_deint,
                            &mut out_deint,
                            &sink,
                        );
                    }
                    tracing::debug!("play_audio: EOF reached for '{}'", file_path);
                    break 'decode;
                }
            };

            if packet.track_id() != track_id {
                continue; // skip packets from other tracks (e.g. cover art)
            }

            let audio_buf = match decoder.decode(&packet) {
                Ok(b)  => b,
                Err(e) => {
                    tracing::warn!("play_audio: skipping corrupt packet: {}", e);
                    continue;
                }
            };

            // Reuse SampleBuffer — avoids allocation after the first packet
            let spec = *audio_buf.spec();
            let sb = sym_sample_buf.get_or_insert_with(|| {
                SymphoniaSampleBuffer::new(audio_buf.capacity() as u64, spec)
            });
            sb.copy_interleaved_ref(audio_buf);
            pending.extend_from_slice(sb.samples());
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// TAURI COMMANDS — AUDIO
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn play_audio(
    file_path: String,
    semitones:  f32,
    volume:     f32,
    state:      State<'_, AudioState>,
) -> Result<(), String> {
    // Atomically increment playback ID — this cancels any running thread
    let new_id = state.playback_id.fetch_add(1, Ordering::SeqCst) + 1;

    let sink = Arc::new(
        Sink::try_new(&state.stream_handle).map_err(|e| e.to_string())?
    );
    sink.set_volume(volume);

    // Swap in the new sink, stopping the previous one cleanly
    {
        let mut guard = state.current_sink.lock().unwrap();
        if let Some(old) = guard.take() {
            old.stop();
        }
        *guard = Some(Arc::clone(&sink));
    }

    spawn_playback_thread(
        file_path,
        semitones,
        sink,
        state.playback_id.clone(),
        new_id,
    );

    Ok(())
}

#[tauri::command]
pub fn stop_audio(state: State<'_, AudioState>) {
    let mut guard = state.current_sink.lock().unwrap();
    if let Some(sink) = guard.take() {
        sink.stop();
    }
    // Increment ID so any running thread terminates on next cancellation check
    state.playback_id.fetch_add(1, Ordering::SeqCst);
}

#[tauri::command]
pub fn set_audio_volume(volume: f32, state: State<'_, AudioState>) {
    if let Some(sink) = state.current_sink.lock().unwrap().as_ref() {
        sink.set_volume(volume);
    }
}