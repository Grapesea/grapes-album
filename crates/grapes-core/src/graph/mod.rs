use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::database::{load_dimensions, now};
use crate::error::{GrapesError, Result};
use crate::semantic::cosine_similarity;
use crate::types::{
    AssetView, GraphEdge, GraphInfo, GraphNode, GraphNodeKind, GraphView, SmartGraphFilter,
    WorkspaceSnapshot,
};

pub const GRAPHSPACES_ID: &str = "graphspaces";
const DATE_ID: &str = "graph:dimension:date";
const CAMERA_ID: &str = "graph:dimension:camera";
const LENS_ID: &str = "graph:dimension:lens";
const TAGS_ID: &str = "graph:dimension:tags";

#[derive(Debug, Clone)]
struct DimensionGraph {
    key: String,
    id: String,
    label: String,
    values: Vec<ValueGroup>,
}

#[derive(Debug, Clone)]
struct SmartGraphRow {
    id: String,
    name: String,
    filters: Vec<SmartGraphFilter>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SmartGraphQuery {
    all: Vec<SmartGraphFilter>,
}

#[derive(Debug, Clone)]
struct ValueGroup {
    id: String,
    key: String,
    label: String,
    asset_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct GraphDefinition {
    id: String,
    parent_id: Option<String>,
    name: String,
    kind: &'static str,
    dimension: Option<String>,
    dimension_value: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct Position {
    x: f64,
    y: f64,
    pinned: bool,
}

struct GraphNodeSpec<'a> {
    id: &'a str,
    kind: GraphNodeKind,
    dummy: bool,
    label: &'a str,
    subtitle: Option<String>,
    target_graph_id: Option<&'a str>,
    asset_id: Option<&'a str>,
    photo_count: usize,
    available: bool,
}

pub fn graph_catalog(snapshot: &WorkspaceSnapshot, conn: &Connection) -> Result<Vec<GraphInfo>> {
    let dimensions = grouped_dimensions(snapshot, conn)?;
    let smart_graphs = load_smart_graphs(snapshot, conn)?;
    let tag_dimension = dimensions
        .iter()
        .find(|dimension| dimension.key == "tags")
        .expect("tag dimension is always generated");
    let root_dimension_count = dimensions
        .iter()
        .filter(|dimension| is_root_dimension(dimension))
        .count();
    let mut catalog = Vec::new();
    catalog.push(GraphInfo {
        id: GRAPHSPACES_ID.into(),
        parent_id: None,
        name: "GraphSpaces".into(),
        kind: "root".into(),
        dimension: None,
        dimension_value: None,
        photo_count: snapshot.assets.len(),
        child_count: root_dimension_count + tag_dimension.values.len() + smart_graphs.len(),
    });
    ensure_graph(
        conn,
        &GraphDefinition {
            id: GRAPHSPACES_ID.into(),
            parent_id: None,
            name: "GraphSpaces".into(),
            kind: "root",
            dimension: None,
            dimension_value: None,
        },
    )?;

    for dimension in dimensions
        .iter()
        .filter(|dimension| dimension.key != "tags")
    {
        let photo_ids = dimension
            .values
            .iter()
            .flat_map(|value| value.asset_ids.iter())
            .collect::<HashSet<_>>();
        catalog.push(GraphInfo {
            id: dimension.id.clone(),
            parent_id: Some(GRAPHSPACES_ID.into()),
            name: dimension.label.clone(),
            kind: "dimension".into(),
            dimension: Some(dimension.key.clone()),
            dimension_value: None,
            photo_count: photo_ids.len(),
            child_count: dimension.values.len(),
        });
        ensure_graph(
            conn,
            &GraphDefinition {
                id: dimension.id.clone(),
                parent_id: Some(GRAPHSPACES_ID.into()),
                name: dimension.label.clone(),
                kind: "dimension",
                dimension: Some(dimension.key.clone()),
                dimension_value: None,
            },
        )?;

        for value in &dimension.values {
            catalog.push(GraphInfo {
                id: value.id.clone(),
                parent_id: Some(dimension.id.clone()),
                name: value.label.clone(),
                kind: "value".into(),
                dimension: Some(dimension.key.clone()),
                dimension_value: Some(value.key.clone()),
                photo_count: value.asset_ids.len(),
                child_count: 0,
            });
            ensure_graph(
                conn,
                &GraphDefinition {
                    id: value.id.clone(),
                    parent_id: Some(dimension.id.clone()),
                    name: value.label.clone(),
                    kind: "value",
                    dimension: Some(dimension.key.clone()),
                    dimension_value: Some(value.key.clone()),
                },
            )?;
        }
    }

    for value in &tag_dimension.values {
        catalog.push(GraphInfo {
            id: value.id.clone(),
            parent_id: Some(GRAPHSPACES_ID.into()),
            name: value.label.clone(),
            kind: "value".into(),
            dimension: Some("tags".into()),
            dimension_value: Some(value.key.clone()),
            photo_count: value.asset_ids.len(),
            child_count: 0,
        });
        ensure_graph(
            conn,
            &GraphDefinition {
                id: value.id.clone(),
                parent_id: Some(GRAPHSPACES_ID.into()),
                name: value.label.clone(),
                kind: "value",
                dimension: Some("tags".into()),
                dimension_value: Some(value.key.clone()),
            },
        )?;
    }

    for smart in &smart_graphs {
        catalog.push(GraphInfo {
            id: smart.id.clone(),
            parent_id: Some(GRAPHSPACES_ID.into()),
            name: smart.name.clone(),
            kind: "smart".into(),
            dimension: None,
            dimension_value: None,
            photo_count: matching_smart_assets(snapshot, &smart.filters).len(),
            child_count: 0,
        });
        ensure_graph(
            conn,
            &GraphDefinition {
                id: smart.id.clone(),
                parent_id: Some(GRAPHSPACES_ID.into()),
                name: smart.name.clone(),
                kind: "smart",
                dimension: None,
                dimension_value: None,
            },
        )?;
    }

    Ok(catalog)
}

pub fn build_graph_view(
    snapshot: &WorkspaceSnapshot,
    conn: &Connection,
    requested_graph_id: Option<&str>,
) -> Result<GraphView> {
    let catalog = graph_catalog(snapshot, conn)?;
    let graph_id = requested_graph_id.unwrap_or(GRAPHSPACES_ID);
    let graph = catalog
        .iter()
        .find(|graph| graph.id == graph_id)
        .cloned()
        .ok_or_else(|| GrapesError::NotFound(format!("图谱 {graph_id}")))?;
    let positions = load_positions(conn, graph_id)?;
    let dimensions = grouped_dimensions(snapshot, conn)?;
    let smart_graphs = load_smart_graphs(snapshot, conn)?;
    let assets_by_id = snapshot
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect::<HashMap<_, _>>();

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    match graph.kind.as_str() {
        "root" => {
            let center_id = "portal:graphspaces".to_owned();
            nodes.push(graph_node(
                GraphNodeSpec {
                    id: &center_id,
                    kind: GraphNodeKind::Portal,
                    dummy: true,
                    label: "GraphSpaces",
                    subtitle: Some(format!("{} photos", snapshot.assets.len())),
                    target_graph_id: None,
                    asset_id: None,
                    photo_count: snapshot.assets.len(),
                    available: true,
                },
                positions.get(&center_id),
            ));
            for dimension in dimensions
                .iter()
                .filter(|dimension| is_root_dimension(dimension))
            {
                let target = Some(dimension.id.as_str());
                let id = format!("portal:{}", dimension.id);
                nodes.push(graph_node(
                    GraphNodeSpec {
                        id: &id,
                        kind: GraphNodeKind::Portal,
                        dummy: false,
                        label: &dimension.label,
                        subtitle: target
                            .map(|_| format!("{} photos", graph_count(&catalog, target))),
                        target_graph_id: target,
                        asset_id: None,
                        photo_count: graph_count(&catalog, target),
                        available: target.is_some(),
                    },
                    positions.get(&id),
                ));
                edges.push(edge(&center_id, &id));
            }
            let tags = dimensions
                .iter()
                .find(|dimension| dimension.key == "tags")
                .ok_or_else(|| GrapesError::State("标签维度缺失".into()))?;
            for value in &tags.values {
                let id = format!("portal:{}", value.id);
                nodes.push(graph_node(
                    GraphNodeSpec {
                        id: &id,
                        kind: GraphNodeKind::Portal,
                        dummy: false,
                        label: &value.label,
                        subtitle: Some(format!("{} photos", value.asset_ids.len())),
                        target_graph_id: Some(&value.id),
                        asset_id: None,
                        photo_count: value.asset_ids.len(),
                        available: true,
                    },
                    positions.get(&id),
                ));
                edges.push(edge(&center_id, &id));
            }
            for smart in &smart_graphs {
                let id = format!("portal:{}", smart.id);
                let count = graph_count(&catalog, Some(&smart.id));
                nodes.push(graph_node(
                    GraphNodeSpec {
                        id: &id,
                        kind: GraphNodeKind::Portal,
                        dummy: false,
                        label: &smart.name,
                        subtitle: Some(format!("{count} photos")),
                        target_graph_id: Some(&smart.id),
                        asset_id: None,
                        photo_count: count,
                        available: true,
                    },
                    positions.get(&id),
                ));
                edges.push(edge(&center_id, &id));
            }
        }
        "dimension" => {
            let dimension_key = graph.dimension.as_deref().unwrap_or_default();
            let dimension = dimensions
                .iter()
                .find(|dimension| dimension.key == dimension_key)
                .ok_or_else(|| GrapesError::State("未知的图谱维度".into()))?;
            let center_id = format!("portal:{}", graph.id);
            nodes.push(graph_node(
                GraphNodeSpec {
                    id: &center_id,
                    kind: GraphNodeKind::Portal,
                    dummy: true,
                    label: &dimension.label,
                    subtitle: Some(format!("{} photos", graph.photo_count)),
                    target_graph_id: None,
                    asset_id: None,
                    photo_count: graph.photo_count,
                    available: true,
                },
                positions.get(&center_id),
            ));
            for value in &dimension.values {
                let value_graph = catalog
                    .iter()
                    .find(|candidate| {
                        candidate.parent_id.as_deref() == Some(dimension.id.as_str())
                            && candidate.dimension_value.as_deref() == Some(&value.key)
                    })
                    .ok_or_else(|| GrapesError::State("维度值图谱缺失".into()))?;
                let id = format!("portal:{}", value_graph.id);
                nodes.push(graph_node(
                    GraphNodeSpec {
                        id: &id,
                        kind: GraphNodeKind::Portal,
                        dummy: false,
                        label: &value.label,
                        subtitle: Some(format!("{} photos", value.asset_ids.len())),
                        target_graph_id: Some(&value_graph.id),
                        asset_id: None,
                        photo_count: value.asset_ids.len(),
                        available: true,
                    },
                    positions.get(&id),
                ));
                edges.push(edge(&center_id, &id));
            }
        }
        "value" => {
            let dimension = dimensions
                .iter()
                .find(|dimension| Some(dimension.key.as_str()) == graph.dimension.as_deref())
                .ok_or_else(|| GrapesError::NotFound("分类维度不存在".into()))?;
            let value = find_value(
                &dimension.values,
                graph.dimension_value.as_deref().unwrap_or_default(),
            )
            .ok_or_else(|| GrapesError::NotFound("维度值不存在".into()))?;
            let center_id = format!("portal:{}", graph.id);
            nodes.push(graph_node(
                GraphNodeSpec {
                    id: &center_id,
                    kind: GraphNodeKind::Portal,
                    dummy: true,
                    label: &graph.name,
                    subtitle: Some(format!("{} photos", value.asset_ids.len())),
                    target_graph_id: None,
                    asset_id: None,
                    photo_count: value.asset_ids.len(),
                    available: true,
                },
                positions.get(&center_id),
            ));
            for asset_id in &value.asset_ids {
                let Some(asset) = assets_by_id.get(asset_id.as_str()) else {
                    continue;
                };
                let id = format!("asset:{}", asset.id);
                nodes.push(graph_node(
                    GraphNodeSpec {
                        id: &id,
                        kind: GraphNodeKind::Asset,
                        dummy: false,
                        label: &asset.filename,
                        subtitle: asset_subtitle(asset),
                        target_graph_id: None,
                        asset_id: Some(&asset.id),
                        photo_count: 1,
                        available: true,
                    },
                    positions.get(&id),
                ));
                edges.push(edge(&center_id, &id));
            }
        }
        "smart" => {
            let smart = smart_graphs
                .iter()
                .find(|smart| smart.id == graph.id)
                .ok_or_else(|| GrapesError::NotFound("组合图谱不存在".into()))?;
            let matching = matching_smart_assets(snapshot, &smart.filters);
            let center_id = format!("portal:{}", graph.id);
            nodes.push(graph_node(
                GraphNodeSpec {
                    id: &center_id,
                    kind: GraphNodeKind::Portal,
                    dummy: true,
                    label: &graph.name,
                    subtitle: Some(format!("{} photos", matching.len())),
                    target_graph_id: None,
                    asset_id: None,
                    photo_count: matching.len(),
                    available: true,
                },
                positions.get(&center_id),
            ));
            for asset in matching {
                let id = format!("asset:{}", asset.id);
                nodes.push(graph_node(
                    GraphNodeSpec {
                        id: &id,
                        kind: GraphNodeKind::Asset,
                        dummy: false,
                        label: &asset.filename,
                        subtitle: asset_subtitle(asset),
                        target_graph_id: None,
                        asset_id: Some(&asset.id),
                        photo_count: 1,
                        available: true,
                    },
                    positions.get(&id),
                ));
                edges.push(edge(&center_id, &id));
            }
        }
        _ => return Err(GrapesError::State("未知的图谱类型".into())),
    }

    append_semantic_neighbor_edges(conn, &nodes, &mut edges)?;

    let mut breadcrumbs = Vec::new();
    let mut current = Some(graph.id.clone());
    while let Some(id) = current {
        let Some(entry) = catalog.iter().find(|entry| entry.id == id) else {
            break;
        };
        breadcrumbs.push(entry.clone());
        current = entry.parent_id.clone();
    }
    breadcrumbs.reverse();

    Ok(GraphView {
        graph,
        breadcrumbs,
        nodes,
        edges,
    })
}

pub fn save_position(
    conn: &Connection,
    graph_id: &str,
    node_key: &str,
    x: f64,
    y: f64,
    pinned: bool,
) -> Result<()> {
    if !x.is_finite() || !y.is_finite() {
        return Err(GrapesError::InvalidPath("图谱坐标必须是有限数值".into()));
    }
    let exists = conn
        .query_row("SELECT 1 FROM graphs WHERE id = ?1", [graph_id], |_| Ok(()))
        .optional()?
        .is_some();
    if !exists {
        return Err(GrapesError::NotFound(format!("图谱 {graph_id}")));
    }
    conn.execute(
        "INSERT INTO graph_positions(graph_id, node_key, x, y, pinned)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(graph_id, node_key) DO UPDATE SET
             x = excluded.x,
             y = excluded.y,
             pinned = excluded.pinned",
        params![graph_id, node_key, x, y, pinned],
    )?;
    Ok(())
}

fn grouped_dimensions(
    snapshot: &WorkspaceSnapshot,
    conn: &Connection,
) -> Result<Vec<DimensionGraph>> {
    let mut result = vec![
        auto_dimension(snapshot, "date", DATE_ID, "Date"),
        auto_dimension(snapshot, "camera", CAMERA_ID, "Camera"),
        auto_dimension(snapshot, "lens", LENS_ID, "Lens"),
    ];
    for dimension in load_dimensions(conn)? {
        let key = if dimension.system_defined
            && matches!(
                dimension.kind.as_str(),
                "time" | "weather" | "location" | "type" | "tone" | "custom"
            ) {
            dimension.kind.clone()
        } else {
            dimension.id.clone()
        };
        let values = dimension
            .values
            .iter()
            .filter_map(|value| {
                let asset_ids = snapshot
                    .assets
                    .iter()
                    .filter(|asset| {
                        asset.classifications.iter().any(|classification| {
                            classification.status == "accepted"
                                && classification.value_id == value.id
                        })
                    })
                    .map(|asset| asset.id.clone())
                    .collect::<Vec<_>>();
                (!asset_ids.is_empty()).then(|| ValueGroup {
                    id: stable_graph_id(&dimension.id, &value.id),
                    key: value.id.clone(),
                    label: value.name.clone(),
                    asset_ids,
                })
            })
            .collect();
        result.push(DimensionGraph {
            key,
            id: format!("graph:{}", dimension.id),
            label: dimension.name,
            values,
        });
    }
    result.push(tag_dimension(snapshot));
    Ok(result)
}

fn auto_dimension(
    snapshot: &WorkspaceSnapshot,
    key: &str,
    id: &str,
    label: &str,
) -> DimensionGraph {
    let mut groups: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for asset in &snapshot.assets {
        let (value_key, value_label) = dimension_value(key, asset);
        groups
            .entry(value_key)
            .or_insert_with(|| (value_label, Vec::new()))
            .1
            .push(asset.id.clone());
    }
    DimensionGraph {
        key: key.into(),
        id: id.into(),
        label: label.into(),
        values: groups
            .into_iter()
            .map(|(value_key, (value_label, asset_ids))| ValueGroup {
                id: stable_graph_id(key, &value_key),
                key: value_key,
                label: value_label,
                asset_ids,
            })
            .collect(),
    }
}

fn is_root_dimension(dimension: &DimensionGraph) -> bool {
    dimension.key != "custom" && dimension.key != "tags"
}

fn tag_dimension(snapshot: &WorkspaceSnapshot) -> DimensionGraph {
    let mut groups: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
    for asset in &snapshot.assets {
        for tag in &asset.tags {
            let key = normalized_key(tag);
            if key.is_empty() {
                continue;
            }
            groups
                .entry(key)
                .or_insert_with(|| (tag.clone(), Vec::new()))
                .1
                .push(asset.id.clone());
        }
    }
    DimensionGraph {
        key: "tags".into(),
        id: TAGS_ID.into(),
        label: "Tags".into(),
        values: groups
            .into_iter()
            .map(|(value_key, (value_label, asset_ids))| ValueGroup {
                id: stable_graph_id("tags", &value_key),
                key: value_key,
                label: value_label,
                asset_ids,
            })
            .collect(),
    }
}

fn dimension_value(dimension: &str, asset: &AssetView) -> (String, String) {
    match dimension {
        "date" => {
            let value = asset
                .metadata
                .captured_at
                .as_deref()
                .filter(|value| value.len() >= 7)
                .map(|value| value[..7].replace(':', "-"))
                .filter(|value| {
                    value.len() == 7
                        && value.as_bytes()[4] == b'-'
                        && value[..4]
                            .chars()
                            .all(|character| character.is_ascii_digit())
                })
                .unwrap_or_else(|| "undated".into());
            let label = if value == "undated" {
                "Undated".into()
            } else {
                value.clone()
            };
            (value, label)
        }
        "camera" => {
            let label = asset
                .metadata
                .camera_model
                .clone()
                .or_else(|| asset.metadata.camera_make.clone())
                .unwrap_or_else(|| "Unknown camera".into());
            (normalized_key(&label), label)
        }
        "lens" => {
            let label = asset
                .metadata
                .lens_model
                .clone()
                .unwrap_or_else(|| "Unknown lens".into());
            (normalized_key(&label), label)
        }
        _ => ("unknown".into(), "Unknown".into()),
    }
}

fn normalized_key(value: &str) -> String {
    value.trim().to_lowercase()
}

pub fn create_smart_graph(
    conn: &Connection,
    name: &str,
    filters: &[SmartGraphFilter],
) -> Result<String> {
    if filters.is_empty() {
        return Err(GrapesError::InvalidPath(
            "组合图谱至少需要一个分类条件".into(),
        ));
    }
    for filter in filters {
        let dimension_exists = conn
            .query_row(
                "SELECT 1 FROM dimensions WHERE id = ?1",
                [&filter.dimension_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !dimension_exists {
            return Err(GrapesError::NotFound(format!(
                "分类维度 {}",
                filter.dimension_id
            )));
        }
        if filter.value_ids.is_empty() {
            return Err(GrapesError::InvalidPath(
                "每个组合图谱条件至少需要一个分类值".into(),
            ));
        }
        for value_id in &filter.value_ids {
            let belongs = conn
                .query_row(
                    "SELECT 1 FROM dimension_values
                     WHERE id = ?1 AND dimension_id = ?2",
                    params![value_id, filter.dimension_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !belongs {
                return Err(GrapesError::NotFound(format!("分类值 {value_id}")));
            }
        }
    }

    ensure_graph(
        conn,
        &GraphDefinition {
            id: GRAPHSPACES_ID.into(),
            parent_id: None,
            name: "GraphSpaces".into(),
            kind: "root",
            dimension: None,
            dimension_value: None,
        },
    )?;
    let id = format!("graph:smart:{}", uuid::Uuid::new_v4());
    let query = SmartGraphQuery {
        all: filters.to_vec(),
    };
    conn.execute(
        "INSERT INTO graphs(
             id, parent_graph_id, name, kind, query_json, layout_algorithm, created_at
         ) VALUES (?1, ?2, ?3, 'smart', ?4, 'forceatlas2', ?5)",
        params![
            id,
            GRAPHSPACES_ID,
            name,
            serde_json::to_string(&query)?,
            now()
        ],
    )?;
    Ok(id)
}

pub fn delete_smart_graph(conn: &Connection, graph_id: &str) -> Result<()> {
    let kind = conn
        .query_row("SELECT kind FROM graphs WHERE id = ?1", [graph_id], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .ok_or_else(|| GrapesError::NotFound(format!("图谱 {graph_id}")))?;
    if kind != "smart" {
        return Err(GrapesError::InvalidPath("只能删除组合图谱".into()));
    }

    let transaction = conn.unchecked_transaction()?;
    transaction.execute(
        "DELETE FROM graph_positions WHERE graph_id = ?1",
        [graph_id],
    )?;
    transaction.execute(
        "DELETE FROM graph_manual_members WHERE graph_id = ?1",
        [graph_id],
    )?;
    transaction.execute("DELETE FROM graphs WHERE id = ?1", [graph_id])?;
    transaction.commit()?;
    Ok(())
}

fn stable_graph_id(dimension: &str, value: &str) -> String {
    let hash = blake3::hash(format!("{dimension}\0{value}").as_bytes());
    let hex = hash.to_hex();
    format!(
        "graph:{dimension}:{}",
        hex.as_str().get(..16).unwrap_or("0000000000000000")
    )
}

fn load_smart_graphs(
    _snapshot: &WorkspaceSnapshot,
    conn: &Connection,
) -> Result<Vec<SmartGraphRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, query_json
         FROM graphs
         WHERE kind = 'smart' AND query_json IS NOT NULL
         ORDER BY created_at, name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut result = Vec::new();
    for (id, name, query_json) in rows {
        let query: SmartGraphQuery = serde_json::from_str(&query_json)?;
        result.push(SmartGraphRow {
            id,
            name,
            filters: query.all,
        });
    }
    Ok(result)
}

fn matching_smart_assets<'a>(
    snapshot: &'a WorkspaceSnapshot,
    filters: &[SmartGraphFilter],
) -> Vec<&'a AssetView> {
    snapshot
        .assets
        .iter()
        .filter(|asset| {
            filters.iter().all(|filter| {
                asset.classifications.iter().any(|classification| {
                    classification.status == "accepted"
                        && classification.dimension_id == filter.dimension_id
                        && filter.value_ids.contains(&classification.value_id)
                })
            })
        })
        .collect()
}

fn ensure_graph(conn: &Connection, graph: &GraphDefinition) -> Result<()> {
    conn.execute(
        "INSERT INTO graphs(
             id, parent_graph_id, name, kind, dimension, dimension_value,
             layout_algorithm, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'forceatlas2', ?7)
         ON CONFLICT(id) DO UPDATE SET
             parent_graph_id = excluded.parent_graph_id,
             name = excluded.name,
             kind = excluded.kind,
             dimension = excluded.dimension,
             dimension_value = excluded.dimension_value",
        params![
            graph.id,
            graph.parent_id,
            graph.name,
            graph.kind,
            graph.dimension,
            graph.dimension_value,
            now()
        ],
    )?;
    Ok(())
}

fn load_positions(conn: &Connection, graph_id: &str) -> Result<HashMap<String, Position>> {
    let mut stmt = conn.prepare(
        "SELECT node_key, x, y, pinned
         FROM graph_positions WHERE graph_id = ?1",
    )?;
    let rows = stmt.query_map([graph_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            Position {
                x: row.get(1)?,
                y: row.get(2)?,
                pinned: row.get::<_, i64>(3)? != 0,
            },
        ))
    })?;
    let mut result = HashMap::new();
    for row in rows {
        let (key, position) = row?;
        result.insert(key, position);
    }
    Ok(result)
}

fn graph_node(spec: GraphNodeSpec<'_>, position: Option<&Position>) -> GraphNode {
    GraphNode {
        id: spec.id.into(),
        kind: spec.kind,
        dummy: spec.dummy,
        label: spec.label.into(),
        subtitle: spec.subtitle,
        target_graph_id: spec.target_graph_id.map(str::to_owned),
        asset_id: spec.asset_id.map(str::to_owned),
        photo_count: spec.photo_count,
        available: spec.available,
        x: position.map(|position| position.x),
        y: position.map(|position| position.y),
        pinned: position.map(|position| position.pinned).unwrap_or(false),
    }
}

fn edge(source: &str, target: &str) -> GraphEdge {
    GraphEdge {
        id: format!("{source}->{target}"),
        source: source.into(),
        target: target.into(),
        weight: 0.15,
    }
}

fn append_semantic_neighbor_edges(
    conn: &Connection,
    nodes: &[GraphNode],
    edges: &mut Vec<GraphEdge>,
) -> Result<()> {
    let asset_ids = nodes
        .iter()
        .filter_map(|node| node.asset_id.clone())
        .collect::<Vec<_>>();
    if asset_ids.len() < 2 {
        return Ok(());
    }
    let embeddings = crate::semantic::latest_embeddings(conn)?;
    let mut seen = HashSet::new();
    for asset_id in &asset_ids {
        let Some(vector) = embeddings.get(asset_id) else {
            continue;
        };
        let mut nearest = asset_ids
            .iter()
            .filter(|other| *other != asset_id)
            .filter_map(|other| {
                let other_vector = embeddings.get(other)?;
                let similarity = cosine_similarity(vector, other_vector)? as f64;
                (similarity >= 0.55).then_some((other.clone(), similarity))
            })
            .collect::<Vec<_>>();
        nearest.sort_by(|left, right| right.1.total_cmp(&left.1));
        for (other, similarity) in nearest.into_iter().take(3) {
            let pair = if asset_id < &other {
                (asset_id.clone(), other)
            } else {
                (other, asset_id.clone())
            };
            if !seen.insert(pair.clone()) {
                continue;
            }
            edges.push(GraphEdge {
                id: format!("semantic:{}<->{}", pair.0, pair.1),
                source: format!("asset:{}", pair.0),
                target: format!("asset:{}", pair.1),
                weight: 0.25 + similarity * 0.75,
            });
        }
    }
    Ok(())
}

fn graph_count(catalog: &[GraphInfo], graph_id: Option<&str>) -> usize {
    graph_id
        .and_then(|id| catalog.iter().find(|graph| graph.id == id))
        .map(|graph| graph.photo_count)
        .unwrap_or(0)
}

fn asset_subtitle(asset: &AssetView) -> Option<String> {
    let date = asset
        .metadata
        .captured_at
        .as_deref()
        .map(|value| value.get(..10).unwrap_or(value));
    match (asset.metadata.camera_model.as_deref(), date) {
        (Some(camera), Some(date)) => Some(format!("{camera} · {date}")),
        (Some(camera), None) => Some(camera.into()),
        (None, Some(date)) => Some(date.into()),
        (None, None) => Some(asset.relative_path.clone()),
    }
}

fn find_value<'a>(values: &'a [ValueGroup], key: &str) -> Option<&'a ValueGroup> {
    values.iter().find(|value| value.key == key)
}
