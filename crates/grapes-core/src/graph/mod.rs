use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};

use crate::database::now;
use crate::error::{GrapesError, Result};
use crate::types::{
    AssetView, GraphEdge, GraphInfo, GraphNode, GraphNodeKind, GraphView, WorkspaceSnapshot,
};

pub const GRAPHSPACES_ID: &str = "graphspaces";
const DATE_ID: &str = "graph:dimension:date";
const CAMERA_ID: &str = "graph:dimension:camera";
const LENS_ID: &str = "graph:dimension:lens";

#[derive(Debug, Clone, Copy)]
struct DimensionSpec {
    key: &'static str,
    id: &'static str,
    label: &'static str,
}

const DIMENSIONS: &[DimensionSpec] = &[
    DimensionSpec {
        key: "date",
        id: DATE_ID,
        label: "Date",
    },
    DimensionSpec {
        key: "camera",
        id: CAMERA_ID,
        label: "Camera",
    },
    DimensionSpec {
        key: "lens",
        id: LENS_ID,
        label: "Lens",
    },
];

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
    label: &'a str,
    subtitle: Option<String>,
    target_graph_id: Option<&'a str>,
    asset_id: Option<&'a str>,
    photo_count: usize,
    available: bool,
}

pub fn graph_catalog(snapshot: &WorkspaceSnapshot, conn: &Connection) -> Result<Vec<GraphInfo>> {
    let grouped = grouped_dimensions(snapshot);
    let mut catalog = Vec::new();
    catalog.push(GraphInfo {
        id: GRAPHSPACES_ID.into(),
        parent_id: None,
        name: "GraphSpaces".into(),
        kind: "root".into(),
        dimension: None,
        dimension_value: None,
        photo_count: snapshot.assets.len(),
        child_count: 9,
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

    for dimension in DIMENSIONS {
        let values = grouped.get(dimension.key).cloned().unwrap_or_default();
        let photo_ids = values
            .iter()
            .flat_map(|value| value.asset_ids.iter())
            .collect::<HashSet<_>>();
        catalog.push(GraphInfo {
            id: dimension.id.into(),
            parent_id: Some(GRAPHSPACES_ID.into()),
            name: dimension.label.into(),
            kind: "dimension".into(),
            dimension: Some(dimension.key.into()),
            dimension_value: None,
            photo_count: photo_ids.len(),
            child_count: values.len(),
        });
        ensure_graph(
            conn,
            &GraphDefinition {
                id: dimension.id.into(),
                parent_id: Some(GRAPHSPACES_ID.into()),
                name: dimension.label.into(),
                kind: "dimension",
                dimension: Some(dimension.key.into()),
                dimension_value: None,
            },
        )?;

        for value in values {
            catalog.push(GraphInfo {
                id: value.id.clone(),
                parent_id: Some(dimension.id.into()),
                name: value.label.clone(),
                kind: "value".into(),
                dimension: Some(dimension.key.into()),
                dimension_value: Some(value.key.clone()),
                photo_count: value.asset_ids.len(),
                child_count: 0,
            });
            ensure_graph(
                conn,
                &GraphDefinition {
                    id: value.id,
                    parent_id: Some(dimension.id.into()),
                    name: value.label,
                    kind: "value",
                    dimension: Some(dimension.key.into()),
                    dimension_value: Some(value.key),
                },
            )?;
        }
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
    let grouped = grouped_dimensions(snapshot);
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
                    label: "GraphSpaces",
                    subtitle: Some(format!("{} photos", snapshot.assets.len())),
                    target_graph_id: None,
                    asset_id: None,
                    photo_count: snapshot.assets.len(),
                    available: true,
                },
                positions.get(&center_id),
            ));
            for dimension in all_root_dimensions() {
                let target = DIMENSIONS
                    .iter()
                    .find(|known| known.key == dimension.key)
                    .map(|known| known.id);
                let id = format!("portal:{}", dimension.key);
                nodes.push(graph_node(
                    GraphNodeSpec {
                        id: &id,
                        kind: GraphNodeKind::Portal,
                        label: dimension.label,
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
        }
        "dimension" => {
            let dimension_key = graph.dimension.as_deref().unwrap_or_default();
            let dimension = DIMENSIONS
                .iter()
                .find(|dimension| dimension.key == dimension_key)
                .ok_or_else(|| GrapesError::State("未知的图谱维度".into()))?;
            let center_id = format!("portal:{}", graph.id);
            nodes.push(graph_node(
                GraphNodeSpec {
                    id: &center_id,
                    kind: GraphNodeKind::Portal,
                    label: dimension.label,
                    subtitle: Some(format!("{} photos", graph.photo_count)),
                    target_graph_id: None,
                    asset_id: None,
                    photo_count: graph.photo_count,
                    available: true,
                },
                positions.get(&center_id),
            ));
            for value in grouped.get(dimension_key).into_iter().flatten() {
                let value_graph = catalog
                    .iter()
                    .find(|candidate| {
                        candidate.parent_id.as_deref() == Some(dimension.id)
                            && candidate.dimension_value.as_deref() == Some(&value.key)
                    })
                    .ok_or_else(|| GrapesError::State("维度值图谱缺失".into()))?;
                let id = format!("portal:{}", value_graph.id);
                nodes.push(graph_node(
                    GraphNodeSpec {
                        id: &id,
                        kind: GraphNodeKind::Portal,
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
            let value = find_value(
                grouped
                    .get(graph.dimension.as_deref().unwrap_or_default())
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                graph.dimension_value.as_deref().unwrap_or_default(),
            )
            .ok_or_else(|| GrapesError::NotFound("维度值不存在".into()))?;
            let center_id = format!("portal:{}", graph.id);
            nodes.push(graph_node(
                GraphNodeSpec {
                    id: &center_id,
                    kind: GraphNodeKind::Portal,
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

fn grouped_dimensions(snapshot: &WorkspaceSnapshot) -> HashMap<String, Vec<ValueGroup>> {
    let mut result = HashMap::new();
    for dimension in DIMENSIONS {
        let mut groups: BTreeMap<String, (String, Vec<String>)> = BTreeMap::new();
        for asset in &snapshot.assets {
            let (key, label) = dimension_value(dimension.key, asset);
            groups
                .entry(key)
                .or_insert_with(|| (label, Vec::new()))
                .1
                .push(asset.id.clone());
        }
        let values = groups
            .into_iter()
            .map(|(key, (label, asset_ids))| ValueGroup {
                id: stable_graph_id(dimension.key, &key),
                key,
                label,
                asset_ids,
            })
            .collect();
        result.insert(dimension.key.to_string(), values);
    }
    result
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

fn stable_graph_id(dimension: &str, value: &str) -> String {
    let hash = blake3::hash(format!("{dimension}\0{value}").as_bytes());
    let hex = hash.to_hex();
    format!(
        "graph:{dimension}:{}",
        hex.as_str().get(..16).unwrap_or("0000000000000000")
    )
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
    }
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

fn all_root_dimensions() -> Vec<DimensionSpec> {
    vec![
        DimensionSpec {
            key: "date",
            id: DATE_ID,
            label: "Date",
        },
        DimensionSpec {
            key: "time",
            id: "",
            label: "Time",
        },
        DimensionSpec {
            key: "weather",
            id: "",
            label: "Weather",
        },
        DimensionSpec {
            key: "location",
            id: "",
            label: "Location",
        },
        DimensionSpec {
            key: "type",
            id: "",
            label: "Type",
        },
        DimensionSpec {
            key: "camera",
            id: CAMERA_ID,
            label: "Camera",
        },
        DimensionSpec {
            key: "lens",
            id: LENS_ID,
            label: "Lens",
        },
        DimensionSpec {
            key: "tone",
            id: "",
            label: "Tone",
        },
        DimensionSpec {
            key: "custom",
            id: "",
            label: "Custom",
        },
    ]
}

fn find_value<'a>(values: &'a [ValueGroup], key: &str) -> Option<&'a ValueGroup> {
    values.iter().find(|value| value.key == key)
}
