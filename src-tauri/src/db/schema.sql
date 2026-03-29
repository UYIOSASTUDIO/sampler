-- sample-vault/src-tauri/src/db/schema.sql

-- Enable Foreign Key Support in SQLite
PRAGMA foreign_keys = ON;

-- 1. Samples Table (Core Entity)
CREATE TABLE IF NOT EXISTS samples (
                                       id TEXT PRIMARY KEY NOT NULL,
                                       file_hash TEXT UNIQUE NOT NULL,
                                       original_path TEXT NOT NULL,
                                       vault_path TEXT,
                                       filename TEXT NOT NULL,
                                       extension TEXT NOT NULL,
                                       file_size INTEGER NOT NULL,
                                       duration_ms INTEGER NOT NULL,
                                       sample_rate INTEGER NOT NULL,
                                       channels INTEGER NOT NULL,
                                       bit_depth INTEGER NOT NULL,
                                       waveform_data TEXT, -- NEU: Speichert das [10, 40, 100, 20...] Array
                                       peak_db REAL,
                                       rms_db REAL,
                                       lufs_db REAL,
                                       bpm REAL,
                                       bpm_confidence REAL,
                                       key_signature TEXT,
                                       key_confidence REAL,
                                       instrument_type TEXT,
                                       type_confidence REAL,
                                       is_favorite BOOLEAN NOT NULL DEFAULT 0,
                                       play_count INTEGER NOT NULL DEFAULT 0,
                                       imported_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                       last_played_at DATETIME
);

-- Index for fast lookup by hash or path
CREATE INDEX IF NOT EXISTS idx_samples_hash ON samples(file_hash);
CREATE INDEX IF NOT EXISTS idx_samples_path ON samples(original_path);
CREATE INDEX IF NOT EXISTS idx_samples_type ON samples(instrument_type);

-- 2. Tags Table
CREATE TABLE IF NOT EXISTS tags (
                                    id TEXT PRIMARY KEY NOT NULL, -- UUID
                                    name TEXT UNIQUE NOT NULL
);

-- 3. Sample-Tags Relations (Many-to-Many)
CREATE TABLE IF NOT EXISTS sample_tags (
                                           sample_id TEXT NOT NULL,
                                           tag_id TEXT NOT NULL,
                                           PRIMARY KEY (sample_id, tag_id),
    FOREIGN KEY (sample_id) REFERENCES samples(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
    );

-- 4. Sources Table (Watch Folders)
CREATE TABLE IF NOT EXISTS sources (
                                       id TEXT PRIMARY KEY NOT NULL, -- UUID
                                       path TEXT UNIQUE NOT NULL,
                                       mode TEXT NOT NULL CHECK(mode IN ('reference', 'copy')),
    is_enabled BOOLEAN NOT NULL DEFAULT 1,
    last_scanned_at DATETIME
    );