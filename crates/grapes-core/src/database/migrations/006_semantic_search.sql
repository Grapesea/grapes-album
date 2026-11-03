CREATE TABLE IF NOT EXISTS semantic_provider_configs (
    kind            TEXT PRIMARY KEY
                    CHECK(kind IN ('embedding', 'rerank')),
    provider_type   TEXT NOT NULL,
    base_url        TEXT NOT NULL,
    model           TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    updated_at      TEXT NOT NULL
);

INSERT OR IGNORE INTO semantic_provider_configs(
    kind, provider_type, base_url, model, enabled, updated_at
) VALUES
    (
        'embedding',
        'siliconflow',
        'https://api.siliconflow.cn/v1',
        'Qwen/Qwen3-VL-Embedding-8B',
        0,
        datetime('now')
    ),
    (
        'rerank',
        'siliconflow',
        'https://api.siliconflow.cn/v1',
        'Qwen/Qwen3-VL-Reranker-8B',
        0,
        datetime('now')
    );

CREATE TABLE IF NOT EXISTS embedding_models (
    id              TEXT PRIMARY KEY,
    provider_kind   TEXT NOT NULL,
    provider_id     TEXT NOT NULL,
    model           TEXT NOT NULL,
    dimension       INTEGER NOT NULL,
    config_hash     TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_embedding_models_config
    ON embedding_models(provider_id, config_hash, dimension);

CREATE TABLE IF NOT EXISTS asset_embeddings (
    asset_id        TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    model_id        TEXT NOT NULL REFERENCES embedding_models(id) ON DELETE CASCADE,
    vector          BLOB NOT NULL,
    created_at      TEXT NOT NULL,
    PRIMARY KEY(asset_id, model_id)
);
