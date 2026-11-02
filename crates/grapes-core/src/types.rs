use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
