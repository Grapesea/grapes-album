CREATE TABLE IF NOT EXISTS workspaces (
    id              TEXT PRIMARY KEY,
    root_path       TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    last_opened_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS folders (
    id              TEXT PRIMARY KEY,
    parent_id       TEXT REFERENCES folders(id),
    name            TEXT NOT NULL,
    fs_file_id      TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_id);
CREATE INDEX IF NOT EXISTS idx_folders_fs_id ON folders(fs_file_id);

CREATE TABLE IF NOT EXISTS assets (
    id              TEXT PRIMARY KEY,
    folder_id       TEXT NOT NULL REFERENCES folders(id),
    filename        TEXT NOT NULL,
    content_hash    BLOB,
    fs_file_id      TEXT,
    file_size       INTEGER,
    mtime_ns        INTEGER,
    width           INTEGER,
    height          INTEGER,
    captured_at     TEXT,
    camera_make     TEXT,
    camera_model    TEXT,
    lens_model      TEXT,
    gps_lat         REAL,
    gps_lon         REAL,
    metadata_json   TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE(folder_id, filename)
);

CREATE INDEX IF NOT EXISTS idx_assets_folder ON assets(folder_id);
CREATE INDEX IF NOT EXISTS idx_assets_fs_id ON assets(fs_file_id);
CREATE INDEX IF NOT EXISTS idx_assets_hash ON assets(content_hash);
