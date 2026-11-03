ALTER TABLE semantic_provider_configs
    ADD COLUMN request_timeout_ms INTEGER NOT NULL DEFAULT 90000;
ALTER TABLE semantic_provider_configs
    ADD COLUMN max_concurrency INTEGER NOT NULL DEFAULT 2;
ALTER TABLE semantic_provider_configs
    ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 2;

ALTER TABLE ask_grapes_provider_configs
    ADD COLUMN request_timeout_ms INTEGER NOT NULL DEFAULT 90000;
ALTER TABLE ask_grapes_provider_configs
    ADD COLUMN max_concurrency INTEGER NOT NULL DEFAULT 2;
ALTER TABLE ask_grapes_provider_configs
    ADD COLUMN max_retries INTEGER NOT NULL DEFAULT 2;
