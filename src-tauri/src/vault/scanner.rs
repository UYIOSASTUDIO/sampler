use futures::stream::{self, StreamExt};
use lofty::{read_from_path, TaggedFileExt};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::audio::{analyzer, metadata_parser, waveform};
use crate::vault::taxonomy;

const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "aiff", "flac", "ogg", "m4a"];

#[derive(Clone, Serialize)]
pub struct ScanProgressPayload {
    pub total: usize,
    pub current: usize,
    pub current_file: String,
}

pub fn is_supported_audio_file(path: &Path) -> bool {
    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
        if file_name.starts_with('.') || file_name.starts_with("._") {
            return false;
        }
    }
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        return SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str());
    }
    false
}

struct CpuAnalysisResult {
    original_path: String,
    filename: String,
    extension: String,
    file_hash: String,
    file_size: i64,
    duration_ms: i64,
    sample_rate: i64,
    channels: i64,
    bit_depth: i64,
    bpm: Option<f64>,
    key_signature: Option<String>,
    instrument_type: Option<String>,
    waveform_data: Vec<u8>,
    tags_json: String,
    cover_path: Option<String>, // NEU: Pfad zum deduplizierten Cover
}

fn fast_fingerprint(path: &Path) -> Result<(String, i64), String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("metadata error: {}", e))?;
    let size = meta.len() as i64;

    let mut file = std::fs::File::open(path).map_err(|e| format!("open error: {}", e))?;
    let mut buffer = [0u8; 8192]; // 8 KB Puffer
    let bytes_read = file.read(&mut buffer).unwrap_or(0);

    let mut hasher = Sha256::new();
    hasher.update(&buffer[..bytes_read]);
    hasher.update(size.to_be_bytes()); // Sichert ab, falls zwei Dateien denselben Header haben

    let fingerprint = format!("{:x}", hasher.finalize());
    Ok((fingerprint, size))
}

/// Extrahiert das Cover-Bild, erstellt einen SHA256-Hash zur Deduplizierung und speichert es auf der Disk.
fn extract_and_save_cover(file_path: &Path, app_data_dir: &Path) -> Option<String> {
    let tagged_file = read_from_path(file_path).ok()?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;

    let picture = tag.pictures().first()?;
    let pic_data = picture.data();

    if pic_data.is_empty() {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(pic_data);
    let hash = format!("{:x}", hasher.finalize());

    let covers_dir = app_data_dir.join("covers");
    if !covers_dir.exists() {
        let _ = fs::create_dir_all(&covers_dir);
    }

    // ENTERPRISE FIX: Sicheres Formatieren des MimeType-Enums in einen String
    let ext = match picture.mime_type() {
        Some(mime) if format!("{:?}", mime).to_lowercase().contains("png") => "png",
        _ => "jpg",
    };

    let cover_file_path = covers_dir.join(format!("{}.{}", hash, ext));

    // Deduplizierung: Nur auf die Festplatte schreiben, wenn der Hash noch nicht existiert
    if !cover_file_path.exists() {
        let _ = fs::write(&cover_file_path, pic_data);
    }

    Some(cover_file_path.to_string_lossy().to_string())
}

fn analyze_file_cpu_heavy(path: &Path, app_data_dir: &Path) -> Result<CpuAnalysisResult, String> {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let original_path = path.to_string_lossy().to_string();
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
        .to_lowercase();

    let (file_hash, file_size) = fast_fingerprint(path)?;

    // ENTERPRISE FIX: Höhere Auflösung für detaillierte Editor-Darstellung
    let waveform_data = waveform::extract_waveform(path, 300).unwrap_or_else(|_| vec![10; 150]);

    let (duration_ms, sample_rate, channels, bit_depth) = match analyzer::extract_metadata(path) {
        Ok(meta) => (
            meta.duration_ms,
            meta.sample_rate as i64,
            meta.channels as i64,
            meta.bit_depth as i64,
        ),
        Err(_) => (0, 44100, 2, 16),
    };

    let engine = taxonomy::TaxonomyEngine::global();
    let tags_array = engine.analyze(path, duration_ms);
    let tags_json = serde_json::to_string(&tags_array).unwrap_or_else(|_| "[]".to_string());

    let is_loop = tags_array
        .iter()
        .any(|t| t["category"] == "Format" && t["value"] == "Loop");
    let parsed_meta = metadata_parser::parse_filename(&filename, is_loop);

    let instrument_type = tags_array
        .iter()
        .find(|t| t["category"] == "Drums" || t["category"] == "Synth" || t["category"] == "Bass")
        .map(|t| t["value"].as_str().unwrap_or("").to_string());

    // Cover extrahieren und auf Disk sichern
    let cover_path = extract_and_save_cover(path, app_data_dir);

    Ok(CpuAnalysisResult {
        original_path,
        filename,
        extension,
        file_hash,
        file_size,
        duration_ms,
        sample_rate,
        channels,
        bit_depth,
        bpm: parsed_meta.bpm,
        key_signature: parsed_meta.key,
        instrument_type,
        waveform_data,
        tags_json,
        cover_path,
    })
}

pub async fn scan_directory(
    path: String,
    pool: SqlitePool,
    app: AppHandle,
) -> Result<usize, String> {
    tracing::info!("Starting directory scan for: {}", path);

    let app_data_dir = app
        .path()
        .app_data_dir()
        .expect("Critical: Failed to resolve app data directory");

    let files = tokio::task::spawn_blocking(move || {
        let mut valid_files = Vec::new();
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let p = entry.path();
                if is_supported_audio_file(p) {
                    valid_files.push(p.to_path_buf());
                }
            }
        }
        valid_files
    })
    .await
    .map_err(|e| format!("Directory read error: {}", e))?;

    let discovered = files.len();
    if discovered == 0 {
        tracing::info!("No valid audio files found in directory.");
        return Ok(0);
    }

    let known_paths: HashSet<String> =
        sqlx::query_scalar::<_, String>("SELECT original_path FROM samples")
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    let files: Vec<PathBuf> = files
        .into_iter()
        .filter(|p| !known_paths.contains(p.to_string_lossy().as_ref()))
        .collect();

    let total_files = files.len();
    tracing::info!(
        "Found {} files ({} already indexed, {} new). Beginning CPU analysis...",
        discovered,
        discovered - total_files,
        total_files
    );

    if total_files == 0 {
        tracing::info!("Library is up to date — nothing to scan.");
        return Ok(0);
    }

    let concurrency_limit = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        * 2;
    let mut processed_count = 0;
    let mut scanned_so_far = 0;

    let _ = app.emit(
        "scan-progress",
        ScanProgressPayload {
            total: total_files,
            current: 0,
            current_file: String::from("Warming up threads..."),
        },
    );

    let stream = stream::iter(files)
        .map(|file_path| {
            let app_data_dir_clone = app_data_dir.clone();
            tokio::task::spawn_blocking(move || {
                analyze_file_cpu_heavy(&file_path, &app_data_dir_clone)
            })
        })
        .buffer_unordered(concurrency_limit);

    let mut chunk_stream = stream.chunks(500);

    while let Some(chunk) = chunk_stream.next().await {
        let chunk_len = chunk.len();
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        let mut last_filename = String::new();

        for task_res in chunk {
            if let Ok(Ok(analysis)) = task_res {
                let id = Uuid::new_v4().to_string();
                last_filename = analysis.filename.clone();

                let insert_result = sqlx::query(
                    r#"
                    INSERT OR IGNORE INTO samples (
                        id, file_hash, original_path, filename, extension, file_size,
                        duration_ms, sample_rate, channels, bit_depth, bpm, key_signature, instrument_type, tags, waveform_data, cover_path
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#
                )
                    .bind(id)
                    .bind(&analysis.file_hash)
                    .bind(&analysis.original_path)
                    .bind(&analysis.filename)
                    .bind(&analysis.extension)
                    .bind(analysis.file_size)
                    .bind(analysis.duration_ms)
                    .bind(analysis.sample_rate)
                    .bind(analysis.channels)
                    .bind(analysis.bit_depth)
                    .bind(analysis.bpm)
                    .bind(&analysis.key_signature)
                    .bind(&analysis.instrument_type)
                    .bind(&analysis.tags_json)
                    .bind(&analysis.waveform_data)
                    .bind(&analysis.cover_path) // NEU
                    .execute(&mut *tx)
                    .await;

                match insert_result {
                    Ok(res) => {
                        if res.rows_affected() > 0 {
                            processed_count += 1;
                        }
                    }
                    Err(e) => {
                        tracing::error!("CRITICAL DB ERROR for file {}: {}", last_filename, e);
                    }
                }
            }
        }

        tx.commit().await.map_err(|e| e.to_string())?;

        scanned_so_far += chunk_len;
        let _ = app.emit(
            "scan-progress",
            ScanProgressPayload {
                total: total_files,
                current: scanned_so_far,
                current_file: last_filename,
            },
        );
    }

    tracing::info!(
        "Directory scan complete. Added {} new files to the database.",
        processed_count
    );
    Ok(processed_count)
}

// HINWEIS: Signatur wurde um `app: AppHandle` erweitert, da wir den Ordner für das Cover auflösen müssen.
// Falls du diese Funktion aus `commands.rs` aufrufst, musst du dort `app: AppHandle` als Parameter hinzufügen.
pub async fn process_single_file(path: PathBuf, pool: SqlitePool, app: AppHandle) {
    let app_data_dir = app.path().app_data_dir().unwrap_or_default();
    let p_clone = path.clone();

    let analysis_res =
        tokio::task::spawn_blocking(move || analyze_file_cpu_heavy(&p_clone, &app_data_dir)).await;

    if let Ok(Ok(analysis)) = analysis_res {
        let id = Uuid::new_v4().to_string();
        let filename = analysis.filename.clone();

        let insert_result = sqlx::query(
            r#"
                    INSERT OR IGNORE INTO samples (
                        id, file_hash, original_path, filename, extension, file_size,
                        duration_ms, sample_rate, channels, bit_depth, bpm, key_signature, instrument_type, tags, waveform_data, cover_path
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#
        )
            .bind(id)
            .bind(&analysis.file_hash)
            .bind(&analysis.original_path)
            .bind(&analysis.filename)
            .bind(&analysis.extension)
            .bind(analysis.file_size)
            .bind(analysis.duration_ms)
            .bind(analysis.sample_rate)
            .bind(analysis.channels)
            .bind(analysis.bit_depth)
            .bind(analysis.bpm)
            .bind(&analysis.key_signature)
            .bind(&analysis.instrument_type)
            .bind(&analysis.tags_json)
            .bind(&analysis.waveform_data)
            .bind(&analysis.cover_path) // NEU
            .execute(&pool)
            .await;

        if let Err(e) = insert_result {
            tracing::error!("CRITICAL DB ERROR for file {}: {}", filename, e);
        } else {
            tracing::info!(
                "Successfully processed background ghost-scan for file: {}",
                filename
            );
        }
    } else {
        tracing::error!("Failed to analyze single file: {:?}", path);
    }
}
