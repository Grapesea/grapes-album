CREATE TABLE IF NOT EXISTS dimensions (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    kind            TEXT NOT NULL,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    system_defined  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS dimension_values (
    id              TEXT PRIMARY KEY,
    dimension_id    TEXT NOT NULL REFERENCES dimensions(id) ON DELETE CASCADE,
    parent_id       TEXT REFERENCES dimension_values(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    description     TEXT,
    color           TEXT,
    sort_order      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_dimension_values_dimension
    ON dimension_values(dimension_id, sort_order);

CREATE TABLE IF NOT EXISTS classifications (
    asset_id            TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    dimension_value_id  TEXT NOT NULL REFERENCES dimension_values(id) ON DELETE CASCADE,
    source              TEXT NOT NULL,
    confidence          REAL,
    status              TEXT NOT NULL DEFAULT 'accepted',
    created_at          TEXT NOT NULL,
    PRIMARY KEY(asset_id, dimension_value_id)
);

CREATE INDEX IF NOT EXISTS idx_classifications_value
    ON classifications(dimension_value_id, status);

INSERT OR IGNORE INTO dimensions(id, name, kind, sort_order, system_defined) VALUES
    ('dimension:time', 'Time', 'time', 10, 1),
    ('dimension:weather', 'Weather', 'weather', 20, 1),
    ('dimension:location', 'Location', 'location', 30, 1),
    ('dimension:type', 'Type', 'type', 40, 1),
    ('dimension:tone', 'Tone', 'tone', 50, 1),
    ('dimension:custom', 'Custom', 'custom', 60, 1);

INSERT OR IGNORE INTO dimension_values(id, dimension_id, name, color, sort_order) VALUES
    ('dimension:time:morning', 'dimension:time', 'Morning', '#e6b96e', 10),
    ('dimension:time:day', 'dimension:time', 'Day', '#d6d6dc', 20),
    ('dimension:time:golden-hour', 'dimension:time', 'Golden Hour', '#e8a65b', 30),
    ('dimension:time:blue-hour', 'dimension:time', 'Blue Hour', '#7891d6', 40),
    ('dimension:time:night', 'dimension:time', 'Night', '#6e6f98', 50),
    ('dimension:weather:sunny', 'dimension:weather', 'Sunny', '#d7c777', 10),
    ('dimension:weather:cloudy', 'dimension:weather', 'Cloudy', '#a8acb5', 20),
    ('dimension:weather:overcast', 'dimension:weather', 'Overcast', '#868a92', 30),
    ('dimension:weather:rain', 'dimension:weather', 'Rain', '#719bc3', 40),
    ('dimension:weather:snow', 'dimension:weather', 'Snow', '#bdd5df', 50),
    ('dimension:weather:fog', 'dimension:weather', 'Fog', '#9fa7aa', 60),
    ('dimension:type:landscape', 'dimension:type', 'Landscape', '#7d9a76', 10),
    ('dimension:type:portrait', 'dimension:type', 'Portrait', '#b78389', 20),
    ('dimension:type:documentary', 'dimension:type', 'Documentary', '#91867b', 30),
    ('dimension:type:street', 'dimension:type', 'Street', '#7e8999', 40),
    ('dimension:type:architecture', 'dimension:type', 'Architecture', '#8b82a2', 50),
    ('dimension:type:still-life', 'dimension:type', 'Still Life', '#a08d76', 60),
    ('dimension:type:wildlife', 'dimension:type', 'Wildlife', '#6f9777', 70),
    ('dimension:tone:warm', 'dimension:tone', 'Warm', '#d79465', 10),
    ('dimension:tone:cool', 'dimension:tone', 'Cool', '#7997ca', 20),
    ('dimension:tone:neutral', 'dimension:tone', 'Neutral', '#a5a5ab', 30),
    ('dimension:tone:high-saturation', 'dimension:tone', 'High Saturation', '#be7593', 40),
    ('dimension:tone:low-saturation', 'dimension:tone', 'Low Saturation', '#8d8d93', 50),
    ('dimension:tone:cinematic', 'dimension:tone', 'Cinematic', '#756b8e', 60);
