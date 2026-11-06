-- v11: classification provenance and per-dimension semantics.
--
-- `method` / `model_id` / `prompt_version` / `evidence` make every
-- classification reproducible: deterministic rows carry the algorithm that
-- produced them, model rows carry the model identity, and manual rows are
-- marked as such. `cardinality` and `origin` let Grapes distinguish dimensions
-- that are derived from metadata or pixels from dimensions a user asserts.

ALTER TABLE classifications ADD COLUMN method TEXT;
ALTER TABLE classifications ADD COLUMN model_id TEXT;
ALTER TABLE classifications ADD COLUMN prompt_version TEXT;
ALTER TABLE classifications ADD COLUMN evidence TEXT;

ALTER TABLE dimensions ADD COLUMN cardinality TEXT NOT NULL DEFAULT 'multi'
    CHECK (cardinality IN ('single', 'multi'));
ALTER TABLE dimensions ADD COLUMN origin TEXT NOT NULL DEFAULT 'manual'
    CHECK (origin IN ('derived', 'manual', 'hybrid'));

UPDATE classifications SET method = 'manual' WHERE source = 'manual';

UPDATE dimensions SET cardinality = 'single', origin = 'derived' WHERE id = 'dimension:time';
UPDATE dimensions SET cardinality = 'single', origin = 'manual' WHERE id = 'dimension:weather';
UPDATE dimensions SET cardinality = 'single', origin = 'hybrid' WHERE id = 'dimension:location';
UPDATE dimensions SET cardinality = 'single', origin = 'manual' WHERE id = 'dimension:type';
UPDATE dimensions SET cardinality = 'multi', origin = 'derived' WHERE id = 'dimension:tone';
UPDATE dimensions SET cardinality = 'multi', origin = 'manual' WHERE id = 'dimension:custom';
