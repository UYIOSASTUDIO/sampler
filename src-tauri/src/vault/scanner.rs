use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};
use std::fs::File;
use std::io::Read;
use uuid::Uuid;
use futures::stream::{self, StreamExt};
use walkdir::WalkDir;
use crate::audio::{analyzer, classify, waveform};

const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "aiff", "flac", "ogg", "m4a"];

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

// Transfer-Objekt für den Thread-Pool
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
    instrument_type: Option<String>,
    waveform_data: Vec<u8>, // GEÄNDERT: Vec<u8> statt String
}

fn analyze_file_cpu_heavy(path: &Path) -> Result<CpuAnalysisResult, String> {
    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let original_path = path.to_string_lossy().to_string();
    let extension = path.extension().unwrap_or_default().to_string_lossy().to_string().to_lowercase();

    let mut file = File::open(path).map_err(|e| format!("Failed to open: {}", e))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    let mut file_size = 0;

    while let Ok(count) = file.read(&mut buffer) {
        if count == 0 { break; }
        hasher.update(&buffer[..count]);
        file_size += count as i64;
    }
    let file_hash = format!("{:x}", hasher.finalize());

    let instrument_type = classify::classify_by_filename(&filename);

    let waveform_data = waveform::extract_waveform(path, 40).unwrap_or_else(|_| {
        vec![10; 40]
    });

    let (duration_ms, sample_rate, channels, bit_depth) = match analyzer::extract_metadata(path) {
        Ok(meta) => (meta.duration_ms, meta.sample_rate as i64, meta.channels as i64, meta.bit_depth as i64),
        Err(_) => (0, 44100, 2, 16)
    };

    Ok(CpuAnalysisResult {
        original_path, filename, extension, file_hash, file_size,
        duration_ms, sample_rate, channels, bit_depth, instrument_type,
        waveform_data // GEÄNDERT: Feldname anpassen
    })
}

pub async fn scan_directory(path: String, pool: SqlitePool) -> Result<usize, String> {
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
    }).await.map_err(|e| format!("Directory read error: {}", e))?;

    if files.is_empty() { return Ok(0); }

    let concurrency_limit = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) * 2;
    let mut processed_count = 0;

    // Stream der die CPU-Berechnungen durchführt
    let stream = stream::iter(files).map(|file_path| {
        tokio::task::spawn_blocking(move || analyze_file_cpu_heavy(&file_path))
    }).buffer_unordered(concurrency_limit);

    // Chunks: Wir sammeln 500 analysierte Dateien im RAM
    let mut chunk_stream = stream.chunks(500);

    // Batch-Insert Phase
    while let Some(chunk) = chunk_stream.next().await {
        // Starte eine einzige, massive Transaktion für alle 500 Dateien
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

        for task_res in chunk {
            if let Ok(Ok(analysis)) = task_res {
                let id = Uuid::new_v4().to_string();
                let insert_result = sqlx::query(
                    r#"
                    INSERT OR IGNORE INTO samples (
                        id, file_hash, original_path, filename, extension, file_size,
                        duration_ms, sample_rate, channels, bit_depth, instrument_type, waveform_data
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                    .bind(&analysis.instrument_type)
                    .bind(&analysis.waveform_data) // GEÄNDERT
                    .execute(&mut *tx)
                    .await;

                if let Ok(res) = insert_result {
                    if res.rows_affected() > 0 {
                        processed_count += 1;
                    }
                }
            }
        }
        // Schreibe die Transaktion mit nur einem SSD-Zugriff endgültig in die Datenbank
        tx.commit().await.map_err(|e| e.to_string())?;
    }

    Ok(processed_count)
}

// Wird vom unsichtbaren Background-Watcher aufgerufen
pub async fn process_single_file(path: PathBuf, pool: SqlitePool) {
    let p_clone = path.clone();

    // CPU-Analyse im Hintergrund-Thread (Hashen, DSP, etc.)
    let analysis_res = tokio::task::spawn_blocking(move || analyze_file_cpu_heavy(&p_clone)).await;

    if let Ok(Ok(analysis)) = analysis_res {
        let id = Uuid::new_v4().to_string();

        // Lautloser Insert in die Datenbank (FTS5 Trigger laufen automatisch mit!)
        let _ = sqlx::query(
            r#"
            INSERT OR IGNORE INTO samples (
                id, file_hash, original_path, filename, extension, file_size,
                duration_ms, sample_rate, channels, bit_depth, instrument_type, waveform_data
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            .bind(&analysis.instrument_type)
            .bind(&analysis.waveform_data)
            .execute(&pool)
            .await;
    }
}