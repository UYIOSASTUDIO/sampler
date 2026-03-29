-- Enable Foreign Key Support in SQLite
PRAGMA foreign_keys = ON;

-- 1. Core Entity: Samples Table
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
                                       waveform_data BLOB,
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

-- 2. B-TREE INDIZES
CREATE INDEX IF NOT EXISTS idx_samples_instrument ON samples(instrument_type);
CREATE INDEX IF NOT EXISTS idx_samples_bpm ON samples(bpm);
CREATE INDEX IF NOT EXISTS idx_samples_key ON samples(key_signature);
CREATE INDEX IF NOT EXISTS idx_samples_imported ON samples(imported_at DESC);

-- 3. FTS5 VIRTUAL TABLE
CREATE VIRTUAL TABLE IF NOT EXISTS samples_fts USING fts5(
    id UNINDEXED,
    filename,
    original_path,
    instrument_type,
    tokenize = 'unicode61'
);

-- 4. FTS5 SYNCHRONISATION TRIGGERS (Korrigiert)
CREATE TRIGGER IF NOT EXISTS samples_ai AFTER INSERT ON samples BEGIN
    INSERT INTO samples_fts(id, filename, original_path, instrument_type)
    VALUES (new.id, new.filename, new.original_path, new.instrument_type);
END;

CREATE TRIGGER IF NOT EXISTS samples_ad AFTER DELETE ON samples BEGIN
DELETE FROM samples_fts WHERE id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS samples_au AFTER UPDATE ON samples BEGIN
DELETE FROM samples_fts WHERE id = old.id;
INSERT INTO samples_fts(id, filename, original_path, instrument_type)
VALUES (new.id, new.filename, new.original_path, new.instrument_type);
END;