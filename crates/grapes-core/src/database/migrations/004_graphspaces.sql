CREATE TABLE IF NOT EXISTS graphs (
    id                  TEXT PRIMARY KEY,
    parent_graph_id     TEXT REFERENCES graphs(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    kind                TEXT NOT NULL,
    dimension           TEXT,
    dimension_value     TEXT,
    query_json          TEXT,
    layout_algorithm    TEXT NOT NULL DEFAULT 'forceatlas2',
    created_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_graphs_parent ON graphs(parent_graph_id);
CREATE INDEX IF NOT EXISTS idx_graphs_dimension ON graphs(dimension, dimension_value);

CREATE TABLE IF NOT EXISTS graph_manual_members (
    graph_id    TEXT NOT NULL REFERENCES graphs(id) ON DELETE CASCADE,
    asset_id    TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    PRIMARY KEY(graph_id, asset_id)
);

CREATE TABLE IF NOT EXISTS graph_positions (
    graph_id    TEXT NOT NULL REFERENCES graphs(id) ON DELETE CASCADE,
    node_key    TEXT NOT NULL,
    x           REAL NOT NULL,
    y           REAL NOT NULL,
    pinned      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(graph_id, node_key)
);
