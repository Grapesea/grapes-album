CREATE TABLE IF NOT EXISTS fs_operations (
    id              TEXT PRIMARY KEY,
    kind            TEXT NOT NULL,
    entity_id       TEXT,
    source_path     TEXT NOT NULL,
    target_path     TEXT NOT NULL,
    status          TEXT NOT NULL,
    error           TEXT,
    created_at      TEXT NOT NULL,
    completed_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_fs_operations_status
    ON fs_operations(status, created_at);

CREATE TABLE IF NOT EXISTS reconciliation_runs (
    id              TEXT PRIMARY KEY,
    report_json     TEXT NOT NULL,
    created_at      TEXT NOT NULL
);
