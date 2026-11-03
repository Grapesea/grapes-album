CREATE TABLE IF NOT EXISTS ask_grapes_provider_configs (
    id              INTEGER PRIMARY KEY CHECK(id = 1),
    provider_type   TEXT NOT NULL,
    base_url        TEXT NOT NULL,
    model           TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    api_key         TEXT,
    updated_at      TEXT NOT NULL
);

INSERT OR IGNORE INTO ask_grapes_provider_configs(
    id, provider_type, base_url, model, enabled, updated_at
) VALUES (
    1,
    'siliconflow',
    'https://api.siliconflow.cn/v1',
    'Qwen/Qwen3.5-4B',
    1,
    datetime('now')
);
