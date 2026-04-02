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
                                       cover_path TEXT,
                                       key_signature TEXT,
                                       key_confidence REAL,
                                       instrument_type TEXT,
                                       type_confidence REAL,
                                       is_liked BOOLEAN NOT NULL DEFAULT 0,

    -- NEU FÜR DIE TAXONOMIE ENGINE:
                                       tags TEXT NOT NULL DEFAULT '[]',
                                       is_user_edited BOOLEAN NOT NULL DEFAULT 0,

                                       is_favorite BOOLEAN NOT NULL DEFAULT 0,
                                       play_count INTEGER NOT NULL DEFAULT 0,
                                       imported_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                                       last_played_at DATETIME
);

CREATE TABLE IF NOT EXISTS collections (
                                           id INTEGER PRIMARY KEY AUTOINCREMENT,
                                           name TEXT UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS collection_samples (
                                                  collection_id INTEGER,
                                                  sample_id TEXT,
                                                  PRIMARY KEY (collection_id, sample_id),
    FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
    FOREIGN KEY (sample_id) REFERENCES samples(id) ON DELETE CASCADE
    );

CREATE TABLE IF NOT EXISTS user_tags (
                                         id INTEGER PRIMARY KEY AUTOINCREMENT,
                                         category TEXT NOT NULL,
                                         value TEXT UNIQUE NOT NULL
);

-- 2. B-TREE INDIZES
CREATE INDEX IF NOT EXISTS idx_samples_instrument ON samples(instrument_type);
CREATE INDEX IF NOT EXISTS idx_samples_bpm ON samples(bpm);
CREATE INDEX IF NOT EXISTS idx_samples_key ON samples(key_signature);
CREATE INDEX IF NOT EXISTS idx_samples_imported ON samples(imported_at DESC);

-- 3. FTS5 VIRTUAL TABLE (Erweitert um 'tags')
CREATE VIRTUAL TABLE IF NOT EXISTS samples_fts USING fts5(
    id UNINDEXED,
    filename,
    original_path,
    instrument_type,
    tags, -- NEU: Damit die blitzschnelle Suchleiste auch Tags findet!
    tokenize = 'unicode61'
);

-- 4. FTS5 SYNCHRONISATION TRIGGERS (Erweitert um 'tags')
CREATE TRIGGER IF NOT EXISTS samples_ai AFTER INSERT ON samples BEGIN
    INSERT INTO samples_fts(id, filename, original_path, instrument_type, tags)
    VALUES (new.id, new.filename, new.original_path, new.instrument_type, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS samples_ad AFTER DELETE ON samples BEGIN
DELETE FROM samples_fts WHERE id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS samples_au AFTER UPDATE ON samples BEGIN
DELETE FROM samples_fts WHERE id = old.id;
INSERT INTO samples_fts(id, filename, original_path, instrument_type, tags)
VALUES (new.id, new.filename, new.original_path, new.instrument_type, new.tags);
END;

-- 5. CONNECTED FOLDERS
CREATE TABLE IF NOT EXISTS connected_folders (
                                                 path TEXT PRIMARY KEY NOT NULL,
                                                 added_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);