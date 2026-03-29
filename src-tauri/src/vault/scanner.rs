use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};
use tokio::fs::{self, File};
use tokio::io::AsyncReadExt;
use uuid::Uuid;
use crate::audio::analyzer;
use crate::audio::classify;

const SUPPORTED_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "aiff", "ogg"];

pub async fn scan_directory(path: String, pool: SqlitePool) -> Result<usize, String> {
    let start_path = PathBuf::from(path);
    if !start_path.exists() || !start_path.is_dir() {
        return Err("Path is not a valid directory".to_string());
    }

    let mut dirs_to_visit = vec![start_path];
    let mut files_processed = 0;

    // Asynchronous directory traversal
    while let Some(current_dir) = dirs_to_visit.pop() {
        let mut entries = match fs::read_dir(&current_dir).await {
            Ok(e) => e,
            Err(_) => continue, // Skip unreadable directories (permissions)
        };

        let mut tasks = vec![];

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();

            if path.is_dir() {
                dirs_to_visit.push(path);
            } else if is_supported_audio_file(&path) {
                let pool_clone = pool.clone();

                // Spawn a concurrent task for each file to maximize I/O throughput
                let task = tokio::spawn(async move {
                    process_audio_file(path, pool_clone).await
                });
                tasks.push(task);
            }
        }

        // Wait for all files in the current directory to be processed
        for task in tasks {
            if let Ok(Ok(true)) = task.await {
                files_processed += 1;
            }
        }
    }

    Ok(files_processed)
}

fn is_supported_audio_file(path: &Path) -> bool {
    // macOS AppleDouble und versteckte Dateien herausfiltern
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

async fn process_audio_file(path: PathBuf, pool: SqlitePool) -> Result<bool, String> {
    // 1. Calculate Hash via streaming to minimize RAM usage
    let file_hash = calculate_file_hash(&path).await
        .map_err(|e| format!("Failed to hash {:?}: {}", path, e))?;

    // 2. Check if already exists in DB to prevent duplicates
    let exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM samples WHERE file_hash = ?")
        .bind(&file_hash)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if exists.0 > 0 {
        return Ok(false);
    }

    // 3. Extract basic metadata
    let metadata = fs::metadata(&path).await.map_err(|e| e.to_string())?;
    let file_size = metadata.len() as i64;
    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let extension = path.extension().unwrap_or_default().to_string_lossy().to_string().to_lowercase();
    let original_path_str = path.to_string_lossy().to_string();
    let id = Uuid::new_v4().to_string();

    // Heuristische Klassifikation über den Dateinamen
    let instrument_type = classify::classify_by_filename(&filename);

    let (duration_ms, sample_rate, channels, bit_depth) = match analyzer::extract_metadata(&path) {
        Ok(meta) => (
            meta.duration_ms,
            meta.sample_rate as i64,
            meta.channels as i64,
            meta.bit_depth as i64
        ),
        Err(e) => {
            println!("Warning: Could not extract metadata for {:?}: {}", path, e);
            (0, 44100, 2, 16)
        }
    };

    // 4. Insert into Database (aktualisiert mit instrument_type)
    sqlx::query(
        r#"
        INSERT INTO samples (
            id, file_hash, original_path, filename, extension, file_size,
            duration_ms, sample_rate, channels, bit_depth, instrument_type
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#
    )
        .bind(id)
        .bind(file_hash)
        .bind(original_path_str)
        .bind(filename)
        .bind(extension)
        .bind(file_size)
        .bind(duration_ms)
        .bind(sample_rate)
        .bind(channels)
        .bind(bit_depth)
        .bind(instrument_type)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(true)
}

async fn calculate_file_hash(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192]; // 8KB Chunk size

    loop {
        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}