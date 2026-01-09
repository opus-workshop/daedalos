-- Undo Timeline Database Schema
-- Version 1.0

CREATE TABLE IF NOT EXISTS entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp REAL NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('edit', 'create', 'delete', 'rename', 'checkpoint', 'restore')),
    file_path TEXT,
    description TEXT,
    before_hash TEXT,
    after_hash TEXT,
    backup_ref TEXT,
    project_path TEXT NOT NULL,
    metadata TEXT
);

CREATE TABLE IF NOT EXISTS checkpoints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    timestamp REAL NOT NULL,
    entry_id INTEGER REFERENCES entries(id),
    type TEXT CHECK(type IN ('manual', 'session_start', 'auto')),
    description TEXT
);

CREATE TABLE IF NOT EXISTS file_backups (
    hash TEXT PRIMARY KEY,
    content BLOB,
    compressed INTEGER DEFAULT 1,
    storage_type TEXT CHECK(storage_type IN ('inline', 'git', 'btrfs', 'file')),
    storage_ref TEXT,
    size INTEGER,
    created REAL
);

CREATE TABLE IF NOT EXISTS projects (
    path TEXT PRIMARY KEY,
    storage_mode TEXT DEFAULT 'git',
    last_checkpoint REAL,
    total_size INTEGER DEFAULT 0
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_entries_timestamp ON entries(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_entries_file ON entries(file_path);
CREATE INDEX IF NOT EXISTS idx_entries_project ON entries(project_path);
CREATE INDEX IF NOT EXISTS idx_entries_type ON entries(type);
CREATE INDEX IF NOT EXISTS idx_checkpoints_name ON checkpoints(name);
CREATE INDEX IF NOT EXISTS idx_backups_created ON file_backups(created);
