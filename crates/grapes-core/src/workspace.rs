use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::{
    insert_asset_tx, insert_folder_tx, load_asset_tags, load_assets, load_folders,
    record_reconcile_tx, update_asset_tx, update_folder_tx, AssetRow, AssetWrite, Database,
    FolderRow, DATABASE_SCHEMA_VERSION,
};
use crate::error::{GrapesError, Result};
use crate::filesystem::{
    file_identity, hash_file, hash_to_hex, is_image_file, scan_workspace,
    validation::validate_name, ScannedAsset,
};
use crate::media::{extract_metadata, MediaMetadata};
use crate::types::{AssetView, FolderView, ReconcileReport, WorkspaceSnapshot, WorkspaceView};

const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceManifest {
    id: String,
    root_folder_id: String,
    schema_version: u32,
    created_at: String,
}

pub struct Workspace {
    root: PathBuf,
    manifest: WorkspaceManifest,
    db: Database,
    last_reconcile: Option<ReconcileReport>,
}

#[derive(Debug)]
struct FolderPlan {
    id: String,
    parent_id: Option<String>,
    name: String,
    fs_file_id: Option<String>,
    created_at: String,
    is_new: bool,
    changed: bool,
}

#[derive(Debug)]
struct AssetPlan {
    id: String,
    folder_id: String,
    filename: String,
    hash: Vec<u8>,
    fs_file_id: Option<String>,
    file_size: i64,
    mtime_ns: i64,
    created_at: String,
    is_new: bool,
    changed: bool,
    metadata: MediaMetadata,
    metadata_json: String,
}

impl Workspace {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = fs::canonicalize(root.as_ref()).map_err(|error| {
            GrapesError::InvalidWorkspace(format!("无法打开 {}：{error}", root.as_ref().display()))
        })?;
        if !root.is_dir() {
            return Err(GrapesError::InvalidWorkspace(format!(
                "{} 不是文件夹",
                root.display()
            )));
        }

        let grapes_dir = root.join(".grapes");
        create_sidecar_layout(&grapes_dir)?;

        let manifest_path = grapes_dir.join("workspace.json");
        let manifest = if manifest_path.exists() {
            let bytes = fs::read(&manifest_path)?;
            let manifest: WorkspaceManifest = serde_json::from_slice(&bytes)?;
            if manifest.schema_version > MANIFEST_SCHEMA_VERSION {
                return Err(GrapesError::InvalidWorkspace(format!(
                    "workspace.json 版本 {} 高于当前支持的版本 {}",
                    manifest.schema_version, MANIFEST_SCHEMA_VERSION
                )));
            }
            manifest
        } else {
            let manifest = WorkspaceManifest {
                id: Uuid::new_v4().to_string(),
                root_folder_id: Uuid::new_v4().to_string(),
                schema_version: MANIFEST_SCHEMA_VERSION,
                created_at: now(),
            };
            write_manifest_atomic(&manifest_path, &manifest)?;
            manifest
        };

        let db = Database::open(&grapes_dir.join("grapes.db"))?;
        if db.schema_version()? != DATABASE_SCHEMA_VERSION {
            return Err(GrapesError::InvalidWorkspace(format!(
                "数据库迁移未完成：当前版本 {}，期望 {}",
                db.schema_version()?,
                DATABASE_SCHEMA_VERSION
            )));
        }
        db.mark_interrupted_operations()?;

        let root_path = root.to_string_lossy().into_owned();
        db.upsert_workspace(&manifest.id, &root_path, &manifest.created_at)?;

        let root_name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "Workspace".into());
        db.ensure_folder(
            &manifest.root_folder_id,
            None,
            &root_name,
            file_identity(&root).as_deref(),
            &manifest.created_at,
        )?;

        let last_reconcile = db
            .last_reconcile()?
            .and_then(|json| serde_json::from_str(&json).ok());

        Ok(Self {
            root,
            manifest,
            db,
            last_reconcile,
        })
    }

    pub fn open_and_reconcile(root: impl AsRef<Path>) -> Result<Self> {
        let mut workspace = Self::open(root)?;
        workspace.reconcile()?;
        Ok(workspace)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn root_folder_id(&self) -> &str {
        &self.manifest.root_folder_id
    }

    pub fn snapshot(&self) -> Result<WorkspaceSnapshot> {
        let folders = load_folders(&self.db.conn)?;
        let assets = load_assets(&self.db.conn)?;
        let tags_by_asset = load_asset_tags(&self.db.conn)?;
        let folder_paths = compute_folder_paths(&folders, &self.manifest.root_folder_id);

        let mut folder_views = folders
            .into_iter()
            .map(|folder| FolderView {
                relative_path: folder_paths.get(&folder.id).cloned().unwrap_or_default(),
                id: folder.id,
                parent_id: folder.parent_id,
                name: folder.name,
                fs_file_id: folder.fs_file_id,
            })
            .collect::<Vec<_>>();
        folder_views.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        let mut asset_views = assets
            .into_iter()
            .map(|asset| {
                let tags = tags_by_asset.get(&asset.id).cloned().unwrap_or_default();
                let parent = folder_paths
                    .get(&asset.folder_id)
                    .cloned()
                    .unwrap_or_default();
                let relative_path = join_relative(&parent, &asset.filename);
                let metadata = metadata_from_row(&asset);
                AssetView {
                    id: asset.id,
                    folder_id: asset.folder_id,
                    filename: asset.filename,
                    relative_path,
                    file_size: asset.file_size,
                    mtime_ns: asset.mtime_ns,
                    fs_file_id: asset.fs_file_id,
                    content_hash: asset.content_hash.as_deref().map(hash_to_hex),
                    tags,
                    metadata,
                }
            })
            .collect::<Vec<_>>();
        asset_views.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        let root_name = folder_views
            .iter()
            .find(|folder| folder.id == self.manifest.root_folder_id)
            .map(|folder| folder.name.clone())
            .unwrap_or_else(|| "Workspace".into());

        Ok(WorkspaceSnapshot {
            workspace: WorkspaceView {
                id: self.manifest.id.clone(),
                name: root_name,
                root_path: user_visible_path(&self.root.to_string_lossy()),
                schema_version: DATABASE_SCHEMA_VERSION,
                created_at: self.manifest.created_at.clone(),
            },
            folders: folder_views,
            assets: asset_views,
            root_folder_id: self.manifest.root_folder_id.clone(),
            last_reconcile: self.last_reconcile.clone(),
        })
    }

    pub fn reconcile(&mut self) -> Result<ReconcileReport> {
        let scan = scan_workspace(&self.root)?;
        let old_folders = load_folders(&self.db.conn)?;
        let old_assets = load_assets(&self.db.conn)?;
        let old_folder_paths = compute_folder_paths(&old_folders, &self.manifest.root_folder_id);

        let mut report = ReconcileReport {
            scanned_folders: scan.folders.len() + 1,
            scanned_assets: scan.assets.len(),
            warnings: scan.warnings,
            ..ReconcileReport::default()
        };

        if !old_folders
            .iter()
            .any(|folder| folder.id == self.manifest.root_folder_id)
        {
            return Err(GrapesError::State("数据库缺少根文件夹".into()));
        }

        let mut used_folders = HashSet::new();
        used_folders.insert(self.manifest.root_folder_id.clone());
        let mut new_folder_ids = HashMap::new();
        new_folder_ids.insert(String::new(), self.manifest.root_folder_id.clone());
        let mut folder_plans = Vec::with_capacity(scan.folders.len());

        for scanned in &scan.folders {
            let candidates = old_folders
                .iter()
                .filter(|folder| {
                    folder.id != self.manifest.root_folder_id && !used_folders.contains(&folder.id)
                })
                .collect::<Vec<_>>();

            let mut matched = scanned.fs_file_id.as_ref().and_then(|fs_id| {
                candidates
                    .iter()
                    .copied()
                    .find(|folder| folder.fs_file_id.as_ref() == Some(fs_id))
            });

            if matched.is_none() {
                matched = candidates.iter().copied().find(|folder| {
                    old_folder_paths.get(&folder.id) == Some(&scanned.relative_path)
                });
            }

            let parent_id = new_folder_ids
                .get(scanned.parent_path.as_deref().unwrap_or(""))
                .cloned()
                .ok_or_else(|| {
                    GrapesError::State(format!("扫描目录顺序异常：{}", scanned.relative_path))
                })?;

            let (id, old_path, created_at, is_new) = match matched {
                Some(existing) => {
                    used_folders.insert(existing.id.clone());
                    (
                        existing.id.clone(),
                        old_folder_paths.get(&existing.id).cloned(),
                        existing.created_at.clone(),
                        false,
                    )
                }
                None => (Uuid::new_v4().to_string(), None, now(), true),
            };

            let changed = matched
                .map(|existing| {
                    existing.parent_id.as_deref() != Some(parent_id.as_str())
                        || existing.name != scanned.name
                        || existing.fs_file_id != scanned.fs_file_id
                })
                .unwrap_or(true);

            if is_new {
                report.folders_inserted += 1;
            } else if old_path.as_deref() != Some(scanned.relative_path.as_str()) {
                report.folders_moved += 1;
            }

            new_folder_ids.insert(scanned.relative_path.clone(), id.clone());
            folder_plans.push(FolderPlan {
                id,
                parent_id: Some(parent_id),
                name: scanned.name.clone(),
                fs_file_id: scanned.fs_file_id.clone(),
                created_at,
                is_new,
                changed,
            });
        }

        let mut old_asset_paths = HashMap::new();
        for asset in &old_assets {
            let parent = old_folder_paths
                .get(&asset.folder_id)
                .cloned()
                .unwrap_or_default();
            old_asset_paths.insert(asset.id.clone(), join_relative(&parent, &asset.filename));
        }

        let mut used_assets = HashSet::new();
        let mut asset_plans = Vec::with_capacity(scan.assets.len());
        let scanned_asset_paths = scan
            .assets
            .iter()
            .map(|asset| asset.relative_path.as_str())
            .collect::<HashSet<_>>();
        for scanned in &scan.assets {
            let folder_id = new_folder_ids
                .get(&scanned.folder_path)
                .cloned()
                .ok_or_else(|| {
                    GrapesError::State(format!("照片所在目录未建立：{}", scanned.folder_path))
                })?;

            let mut matched = old_assets.iter().find(|asset| {
                !used_assets.contains(&asset.id)
                    && scanned.fs_file_id.is_some()
                    && asset.fs_file_id == scanned.fs_file_id
            });

            if matched.is_none() {
                matched = old_assets.iter().find(|asset| {
                    !used_assets.contains(&asset.id)
                        && old_asset_paths.get(&asset.id) == Some(&scanned.relative_path)
                        && asset_identity_compatible(asset, scanned)
                });
            }

            let metadata_unchanged = matched
                .map(|asset| {
                    asset.content_hash.is_some()
                        && asset.file_size == scanned.file_size
                        && asset.mtime_ns == scanned.mtime_ns
                })
                .unwrap_or(false);
            let hash = if metadata_unchanged {
                matched
                    .and_then(|asset| asset.content_hash.clone())
                    .expect("checked content hash")
            } else {
                hash_file(&scanned.physical_path)?
            };

            if matched.is_none() {
                // Preserve identity when the filesystem identity changes in place,
                // for example when an entire Workspace is moved to another volume.
                let exact_path_candidates = old_assets
                    .iter()
                    .filter(|asset| {
                        !used_assets.contains(&asset.id)
                            && asset.content_hash.as_deref() == Some(hash.as_slice())
                            && old_asset_paths.get(&asset.id) == Some(&scanned.relative_path)
                    })
                    .collect::<Vec<_>>();
                if exact_path_candidates.len() == 1 {
                    matched = exact_path_candidates.first().copied();
                }
            }

            if matched.is_none() {
                let moved_hash_candidates = old_assets
                    .iter()
                    .filter(|asset| {
                        !used_assets.contains(&asset.id)
                            && asset.content_hash.as_deref() == Some(hash.as_slice())
                            && old_asset_paths
                                .get(&asset.id)
                                .map(|path| !scanned_asset_paths.contains(path.as_str()))
                                .unwrap_or(true)
                    })
                    .collect::<Vec<_>>();
                if moved_hash_candidates.len() == 1 {
                    matched = moved_hash_candidates.first().copied();
                } else if moved_hash_candidates.len() > 1 {
                    // EXIF disambiguation belongs to P1. For P0, only retain
                    // identity when filename or mtime identifies one candidate.
                    let same_name = moved_hash_candidates
                        .iter()
                        .copied()
                        .filter(|asset| asset.filename == scanned.filename)
                        .collect::<Vec<_>>();
                    if same_name.len() == 1 {
                        matched = same_name.first().copied();
                    }
                }
                if matched.is_none() && moved_hash_candidates.len() > 1 {
                    let same_mtime = moved_hash_candidates
                        .iter()
                        .copied()
                        .filter(|asset| asset.mtime_ns == scanned.mtime_ns)
                        .collect::<Vec<_>>();
                    if same_mtime.len() == 1 {
                        matched = same_mtime.first().copied();
                    }
                }
                if matched.is_none() && moved_hash_candidates.len() > 1 {
                    report.warnings.push(format!(
                        "{} 存在多个相同内容候选，无法稳定判定身份，已按新照片处理",
                        scanned.relative_path
                    ));
                }
            }

            let scan_metadata = matched
                .map(|asset| {
                    asset.metadata_json.is_none()
                        || asset.file_size != scanned.file_size
                        || asset.mtime_ns != scanned.mtime_ns
                        || asset.content_hash.as_deref() != Some(hash.as_slice())
                })
                .unwrap_or(true);
            let metadata = if scan_metadata {
                extract_metadata(&scanned.physical_path)
            } else {
                matched.map(metadata_from_row).unwrap_or_default()
            };
            let metadata_json = if scan_metadata {
                serde_json::to_string(&metadata)?
            } else {
                matched
                    .and_then(|asset| asset.metadata_json.clone())
                    .unwrap_or(serde_json::to_string(&metadata)?)
            };

            let (id, old_path, created_at, is_new, content_changed) = match matched {
                Some(existing) => {
                    used_assets.insert(existing.id.clone());
                    (
                        existing.id.clone(),
                        old_asset_paths.get(&existing.id).cloned(),
                        existing.created_at.clone(),
                        false,
                        existing.content_hash.as_deref() != Some(hash.as_slice()),
                    )
                }
                None => (Uuid::new_v4().to_string(), None, now(), true, true),
            };

            let changed = matched
                .map(|existing| {
                    existing.folder_id != folder_id
                        || existing.filename != scanned.filename
                        || existing.fs_file_id != scanned.fs_file_id
                        || existing.file_size != scanned.file_size
                        || existing.mtime_ns != scanned.mtime_ns
                        || existing.content_hash.as_deref() != Some(hash.as_slice())
                        || existing.metadata_json.as_deref() != Some(metadata_json.as_str())
                })
                .unwrap_or(true);

            if is_new {
                report.assets_inserted += 1;
            } else {
                if old_path.as_deref() != Some(scanned.relative_path.as_str()) {
                    report.assets_moved += 1;
                }
                if content_changed {
                    report.content_changed += 1;
                }
            }

            asset_plans.push(AssetPlan {
                id,
                folder_id,
                filename: scanned.filename.clone(),
                hash,
                fs_file_id: scanned.fs_file_id.clone(),
                file_size: scanned.file_size,
                mtime_ns: scanned.mtime_ns,
                created_at,
                is_new,
                changed,
                metadata,
                metadata_json,
            });
        }

        let removed_asset_ids = old_assets
            .iter()
            .filter(|asset| !used_assets.contains(&asset.id))
            .map(|asset| asset.id.clone())
            .collect::<Vec<_>>();
        report.assets_removed = removed_asset_ids.len();

        let mut removed_folder_ids = old_folders
            .iter()
            .filter(|folder| {
                folder.id != self.manifest.root_folder_id && !used_folders.contains(&folder.id)
            })
            .map(|folder| {
                (
                    folder.id.clone(),
                    old_folder_paths
                        .get(&folder.id)
                        .map(|path| path.matches('/').count())
                        .unwrap_or(0),
                )
            })
            .collect::<Vec<_>>();
        removed_folder_ids.sort_by(|left, right| right.1.cmp(&left.1));
        report.folders_removed = removed_folder_ids.len();
        report.finished_at = now();

        let tx = self.db.conn.transaction()?;
        for folder in folder_plans {
            if folder.is_new {
                insert_folder_tx(
                    &tx,
                    &FolderRow {
                        id: folder.id,
                        parent_id: folder.parent_id,
                        name: folder.name,
                        fs_file_id: folder.fs_file_id,
                        created_at: folder.created_at,
                    },
                )?;
            } else if folder.changed {
                update_folder_tx(
                    &tx,
                    &folder.id,
                    folder.parent_id.as_deref(),
                    &folder.name,
                    folder.fs_file_id.as_deref(),
                )?;
            }
        }

        for asset in asset_plans {
            let write = AssetWrite {
                id: &asset.id,
                folder_id: &asset.folder_id,
                filename: &asset.filename,
                hash: &asset.hash,
                fs_file_id: asset.fs_file_id.as_deref(),
                file_size: asset.file_size,
                mtime_ns: asset.mtime_ns,
                width: asset.metadata.width,
                height: asset.metadata.height,
                captured_at: asset.metadata.captured_at.as_deref(),
                camera_make: asset.metadata.camera_make.as_deref(),
                camera_model: asset.metadata.camera_model.as_deref(),
                lens_model: asset.metadata.lens_model.as_deref(),
                gps_lat: asset.metadata.gps_lat,
                gps_lon: asset.metadata.gps_lon,
                metadata_json: &asset.metadata_json,
                created_at: &asset.created_at,
            };
            if asset.is_new {
                insert_asset_tx(&tx, &write)?;
            } else if asset.changed {
                update_asset_tx(&tx, &write)?;
            }
        }

        for asset_id in &removed_asset_ids {
            tx.execute("DELETE FROM assets WHERE id = ?1", [asset_id])?;
        }
        for (folder_id, _) in removed_folder_ids {
            tx.execute("DELETE FROM folders WHERE id = ?1", [folder_id])?;
        }

        record_reconcile_tx(&tx, &report)?;
        tx.commit()?;

        self.last_reconcile = Some(report.clone());
        Ok(report)
    }

    pub fn create_folder(&mut self, parent_folder_id: &str, name: &str) -> Result<FolderView> {
        validate_name(name)?;
        let parent = self.db.folder(parent_folder_id)?;
        let paths = self.folder_paths()?;
        let parent_relative = paths
            .get(parent_folder_id)
            .cloned()
            .ok_or_else(|| GrapesError::State("父文件夹路径不可解析".into()))?;
        let parent_path = path_from_relative(&self.root, &parent_relative);
        let target = parent_path.join(name);
        if target.exists() {
            return Err(GrapesError::Conflict(format!("已存在同名项目：{name}")));
        }

        let id = Uuid::new_v4().to_string();
        let operation_id = self.db.begin_operation(
            "create_folder",
            &id,
            &target.to_string_lossy(),
            &target.to_string_lossy(),
        )?;

        if let Err(error) = fs::create_dir(&target) {
            let _ = self
                .db
                .finish_operation(&operation_id, "failed", Some(&error.to_string()));
            return Err(error.into());
        }

        let folder = FolderRow {
            id: id.clone(),
            parent_id: Some(parent.id),
            name: name.to_owned(),
            fs_file_id: file_identity(&target),
            created_at: now(),
        };
        if let Err(error) = self.db.insert_folder(&folder) {
            let _ = fs::remove_dir(&target);
            let _ = self
                .db
                .finish_operation(&operation_id, "failed", Some(&error.to_string()));
            return Err(error);
        }
        self.db.finish_operation(&operation_id, "succeeded", None)?;

        Ok(FolderView {
            id,
            parent_id: folder.parent_id,
            name: folder.name,
            relative_path: join_relative(&parent_relative, name),
            fs_file_id: folder.fs_file_id,
        })
    }

    pub fn move_folder(&mut self, folder_id: &str, destination_folder_id: &str) -> Result<()> {
        if folder_id == self.manifest.root_folder_id {
            return Err(GrapesError::InvalidPath("不能移动工作区根目录".into()));
        }
        let folder = self.db.folder(folder_id)?;
        self.relocate_folder(folder, destination_folder_id, None, "move_folder")
    }

    pub fn rename_folder(&mut self, folder_id: &str, new_name: &str) -> Result<()> {
        validate_name(new_name)?;
        if folder_id == self.manifest.root_folder_id {
            return Err(GrapesError::InvalidPath("不能重命名工作区根目录".into()));
        }
        let folder = self.db.folder(folder_id)?;
        let parent_id = folder
            .parent_id
            .clone()
            .ok_or_else(|| GrapesError::State("文件夹缺少父目录".into()))?;
        self.relocate_folder(folder, &parent_id, Some(new_name), "rename_folder")
    }

    pub fn move_asset(&mut self, asset_id: &str, destination_folder_id: &str) -> Result<()> {
        let asset = self.db.asset(asset_id)?;
        self.relocate_asset(asset, destination_folder_id, None, "move_asset")
    }

    pub fn rename_asset(&mut self, asset_id: &str, new_name: &str) -> Result<()> {
        validate_name(new_name)?;
        if !is_image_file(Path::new(new_name)) {
            return Err(GrapesError::InvalidPath(
                "照片文件名必须保留受支持的图片扩展名".into(),
            ));
        }
        let asset = self.db.asset(asset_id)?;
        let folder_id = asset.folder_id.clone();
        self.relocate_asset(asset, &folder_id, Some(new_name), "rename_asset")
    }

    pub fn set_asset_tags(&mut self, asset_id: &str, tags: &[String]) -> Result<()> {
        self.db.asset(asset_id)?;
        if tags.len() > 20 {
            return Err(GrapesError::InvalidPath(
                "每张照片最多添加 20 个标签".into(),
            ));
        }

        let mut seen = HashSet::new();
        let mut normalized = Vec::new();
        for tag in tags {
            let tag = tag.trim();
            if tag.is_empty() {
                continue;
            }
            if tag.chars().count() > 32 {
                return Err(GrapesError::InvalidPath(
                    "单个标签不能超过 32 个字符".into(),
                ));
            }
            if tag.chars().any(char::is_control) {
                return Err(GrapesError::InvalidPath("标签不能包含控制字符".into()));
            }
            if seen.insert(tag.to_lowercase()) {
                normalized.push(tag.to_owned());
            }
        }
        self.db.set_asset_tags(asset_id, &normalized)
    }

    fn relocate_folder(
        &mut self,
        folder: FolderRow,
        destination_folder_id: &str,
        new_name: Option<&str>,
        operation_kind: &str,
    ) -> Result<()> {
        let paths = self.folder_paths()?;
        let source_relative = paths
            .get(&folder.id)
            .cloned()
            .ok_or_else(|| GrapesError::State("源文件夹路径不可解析".into()))?;
        let source = path_from_relative(&self.root, &source_relative);
        let destination_parent_relative = paths
            .get(destination_folder_id)
            .cloned()
            .ok_or_else(|| GrapesError::NotFound(format!("目标文件夹 {destination_folder_id}")))?;
        let destination_parent = path_from_relative(&self.root, &destination_parent_relative);
        let name = new_name.unwrap_or(&folder.name);

        if folder.id == destination_folder_id
            || self.is_descendant(destination_folder_id, &folder.id)?
        {
            return Err(GrapesError::InvalidPath(
                "不能把文件夹移动到自身或其子目录中".into(),
            ));
        }

        let target = destination_parent.join(name);
        if source == target {
            return Ok(());
        }
        if target.exists() {
            return Err(GrapesError::Conflict(format!(
                "目标位置已存在同名项目：{}",
                target.display()
            )));
        }

        let operation_id = self.db.begin_operation(
            operation_kind,
            &folder.id,
            &source.to_string_lossy(),
            &target.to_string_lossy(),
        )?;
        if let Err(error) = fs::rename(&source, &target) {
            let _ = self
                .db
                .finish_operation(&operation_id, "failed", Some(&error.to_string()));
            return Err(error.into());
        }

        let update = self.db.conn.execute(
            "UPDATE folders
             SET parent_id = ?2, name = ?3, fs_file_id = ?4, updated_at = ?5
             WHERE id = ?1",
            params![
                folder.id,
                destination_folder_id,
                name,
                file_identity(&target),
                now()
            ],
        );
        if let Err(error) = update {
            let _ = fs::rename(&target, &source);
            let _ =
                self.db
                    .finish_operation(&operation_id, "rolled_back", Some(&error.to_string()));
            return Err(error.into());
        }

        self.db.finish_operation(&operation_id, "succeeded", None)?;
        Ok(())
    }

    fn relocate_asset(
        &mut self,
        asset: AssetRow,
        destination_folder_id: &str,
        new_name: Option<&str>,
        operation_kind: &str,
    ) -> Result<()> {
        let paths = self.folder_paths()?;
        let source_parent = paths
            .get(&asset.folder_id)
            .cloned()
            .ok_or_else(|| GrapesError::State("源照片目录不可解析".into()))?;
        let destination_parent = paths
            .get(destination_folder_id)
            .cloned()
            .ok_or_else(|| GrapesError::NotFound(format!("目标文件夹 {destination_folder_id}")))?;
        let source = path_from_relative(&self.root, &source_parent).join(&asset.filename);
        let name = new_name.unwrap_or(&asset.filename);
        let target = path_from_relative(&self.root, &destination_parent).join(name);

        if source == target {
            return Ok(());
        }
        if !source.is_file() {
            return Err(GrapesError::NotFound(format!(
                "原照片不存在：{}",
                source.display()
            )));
        }
        if target.exists() {
            return Err(GrapesError::Conflict(format!(
                "目标位置已存在同名文件：{}",
                target.display()
            )));
        }

        let operation_id = self.db.begin_operation(
            operation_kind,
            &asset.id,
            &source.to_string_lossy(),
            &target.to_string_lossy(),
        )?;
        if let Err(error) = fs::rename(&source, &target) {
            let _ = self
                .db
                .finish_operation(&operation_id, "failed", Some(&error.to_string()));
            return Err(error.into());
        }

        let update = self
            .db
            .update_asset_position(&asset.id, destination_folder_id, name);
        if let Err(error) = update {
            let _ = fs::rename(&target, &source);
            let _ =
                self.db
                    .finish_operation(&operation_id, "rolled_back", Some(&error.to_string()));
            return Err(error);
        }

        self.db.finish_operation(&operation_id, "succeeded", None)?;
        Ok(())
    }

    fn folder_paths(&self) -> Result<HashMap<String, String>> {
        Ok(compute_folder_paths(
            &load_folders(&self.db.conn)?,
            &self.manifest.root_folder_id,
        ))
    }

    fn is_descendant(&self, candidate_id: &str, ancestor_id: &str) -> Result<bool> {
        let folders = load_folders(&self.db.conn)?
            .into_iter()
            .map(|folder| (folder.id, folder.parent_id))
            .collect::<HashMap<_, _>>();
        let mut current = Some(candidate_id.to_owned());
        let mut visited = HashSet::new();
        while let Some(id) = current {
            if id == ancestor_id {
                return Ok(true);
            }
            if !visited.insert(id.clone()) {
                return Err(GrapesError::State("文件夹层级存在循环".into()));
            }
            current = folders.get(&id).cloned().flatten();
        }
        Ok(false)
    }
}

fn create_sidecar_layout(grapes_dir: &Path) -> Result<()> {
    for relative in ["", "thumbnails", "cache", "graph", "graph/layouts"] {
        fs::create_dir_all(grapes_dir.join(relative))?;
    }
    Ok(())
}

fn write_manifest_atomic(path: &Path, manifest: &WorkspaceManifest) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(manifest)?;
    fs::write(&temporary, bytes)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)?;
    Ok(())
}

fn compute_folder_paths(folders: &[FolderRow], root_folder_id: &str) -> HashMap<String, String> {
    let by_id = folders
        .iter()
        .map(|folder| (folder.id.as_str(), folder))
        .collect::<HashMap<_, _>>();
    let mut memo = HashMap::new();
    for folder in folders {
        resolve_folder_path(folder.id.as_str(), &by_id, root_folder_id, &mut memo, 0);
    }
    memo
}

fn resolve_folder_path(
    id: &str,
    folders: &HashMap<&str, &FolderRow>,
    root_folder_id: &str,
    memo: &mut HashMap<String, String>,
    depth: usize,
) -> String {
    if let Some(path) = memo.get(id) {
        return path.clone();
    }
    if id == root_folder_id || depth > 1024 {
        memo.insert(id.into(), String::new());
        return String::new();
    }
    let Some(folder) = folders.get(id) else {
        return String::new();
    };
    let parent = folder
        .parent_id
        .as_deref()
        .map(|parent_id| resolve_folder_path(parent_id, folders, root_folder_id, memo, depth + 1))
        .unwrap_or_default();
    let path = join_relative(&parent, &folder.name);
    memo.insert(id.into(), path.clone());
    path
}

fn asset_identity_compatible(existing: &AssetRow, scanned: &ScannedAsset) -> bool {
    match (
        existing.fs_file_id.as_deref(),
        scanned.fs_file_id.as_deref(),
    ) {
        (Some(left), Some(right)) => left == right,
        _ => true,
    }
}

fn metadata_from_row(asset: &AssetRow) -> MediaMetadata {
    let mut metadata = asset
        .metadata_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<MediaMetadata>(json).ok())
        .unwrap_or_default();
    metadata.width = asset.width;
    metadata.height = asset.height;
    metadata.captured_at = asset.captured_at.clone();
    metadata.camera_make = asset.camera_make.clone();
    metadata.camera_model = asset.camera_model.clone();
    metadata.lens_model = asset.lens_model.clone();
    metadata.gps_lat = asset.gps_lat;
    metadata.gps_lon = asset.gps_lon;
    metadata.scanned = true;
    metadata
}

fn path_from_relative(root: &Path, relative: &str) -> PathBuf {
    if relative.is_empty() {
        return root.to_owned();
    }
    let mut path = root.to_owned();
    for component in relative.split('/') {
        if !component.is_empty() {
            path.push(component);
        }
    }
    path
}

fn join_relative(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn user_visible_path(path: &str) -> String {
    let Some(rest) = path.strip_prefix(r"\\?\") else {
        return path.to_owned();
    };
    if let Some(network_path) = rest.strip_prefix("UNC\\") {
        return format!(r"\\{network_path}");
    }
    let bytes = rest.as_bytes();
    if bytes.len() >= 3 && bytes[1] == b':' && matches!(bytes[2], b'\\' | b'/') {
        return rest.to_owned();
    }
    path.to_owned()
}

#[cfg(test)]
mod tests {
    use super::user_visible_path;

    #[test]
    fn removes_windows_verbatim_prefix_for_display() {
        assert_eq!(
            user_visible_path(r"\\?\H:\Captures\hongkong"),
            r"H:\Captures\hongkong"
        );
        assert_eq!(
            user_visible_path(r"\\?\UNC\server\share\photos"),
            r"\\server\share\photos"
        );
        assert_eq!(
            user_visible_path(r"H:\Captures\hongkong"),
            r"H:\Captures\hongkong"
        );
    }
}
