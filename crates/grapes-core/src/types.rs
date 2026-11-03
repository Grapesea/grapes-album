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
    pub classifications: Vec<AssetClassificationView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DimensionValueView {
    pub id: String,
    pub dimension_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DimensionView {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub sort_order: i64,
    pub system_defined: bool,
    pub values: Vec<DimensionValueView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetClassificationView {
    pub dimension_id: String,
    pub dimension_name: String,
    pub dimension_kind: String,
    pub value_id: String,
    pub value_name: String,
    pub color: Option<String>,
    pub source: String,
    pub confidence: Option<f64>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SmartGraphFilter {
    pub dimension_id: String,
    pub value_ids: Vec<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphView {
    pub graph: GraphInfo,
    pub breadcrumbs: Vec<GraphInfo>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticProviderConfig {
    pub kind: String,
    pub provider_type: String,
    pub base_url: String,
    pub model: String,
    pub enabled: bool,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticIndexStatus {
    pub total_assets: usize,
    pub indexed_assets: usize,
    pub pending_assets: usize,
    pub model_id: Option<String>,
    pub dimension: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SemanticIndexReport {
    pub indexed: usize,
    pub failed: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticMatchReason {
    pub kind: String,
    pub label: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchResult {
    pub asset_id: String,
    pub filename: String,
    pub relative_path: String,
    pub score: f64,
    pub match_reasons: Vec<SemanticMatchReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SemanticSearchResponse {
    pub candidate_count: usize,
    pub reranked: bool,
    pub results: Vec<SemanticSearchResult>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SemanticIndexSource {
    pub asset_id: String,
    pub thumbnail_path: std::path::PathBuf,
}

#[derive(Debug, Clone)]
pub struct SemanticAssetRecord {
    pub asset_id: String,
    pub filename: String,
    pub relative_path: String,
    pub combined_text: String,
    pub classification_search_terms: Vec<String>,
    pub metadata_text: String,
    pub thumbnail_path: Option<std::path::PathBuf>,
}
