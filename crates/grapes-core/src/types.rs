use serde::{Deserialize, Serialize};

use crate::media::MediaMetadata;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub schema_version: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FolderView {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub relative_path: String,
    pub fs_file_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetView {
    pub id: String,
    pub folder_id: String,
    pub filename: String,
    pub relative_path: String,
    pub file_size: i64,
    pub mtime_ns: i64,
    pub fs_file_id: Option<String>,
    pub content_hash: Option<String>,
    pub tags: Vec<String>,
    pub metadata: MediaMetadata,
    pub has_thumbnail: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct ReconcileReport {
    pub scanned_folders: usize,
    pub scanned_assets: usize,
    pub folders_inserted: usize,
    pub folders_moved: usize,
    pub folders_removed: usize,
    pub assets_inserted: usize,
    pub assets_moved: usize,
    pub assets_removed: usize,
    pub content_changed: usize,
    pub thumbnails_created: usize,
    pub finished_at: String,
    pub warnings: Vec<String>,
}

impl ReconcileReport {
    pub fn is_empty(&self) -> bool {
        self.folders_inserted == 0
            && self.folders_moved == 0
            && self.folders_removed == 0
            && self.assets_inserted == 0
            && self.assets_moved == 0
            && self.assets_removed == 0
            && self.content_changed == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub workspace: WorkspaceView,
    pub folders: Vec<FolderView>,
    pub assets: Vec<AssetView>,
    pub root_folder_id: String,
    pub last_reconcile: Option<ReconcileReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GraphInfo {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub dimension: Option<String>,
    pub dimension_value: Option<String>,
    pub photo_count: usize,
    pub child_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GraphNodeKind {
    Portal,
    Asset,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub kind: GraphNodeKind,
    pub dummy: bool,
    pub label: String,
    pub subtitle: Option<String>,
    pub target_graph_id: Option<String>,
    pub asset_id: Option<String>,
    pub photo_count: usize,
    pub available: bool,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphView {
    pub graph: GraphInfo,
    pub breadcrumbs: Vec<GraphInfo>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}
