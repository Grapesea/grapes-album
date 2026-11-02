use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::error::{GrapesError, Result};

pub const DATABASE_SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Clone)]
pub struct FolderRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub fs_file_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct AssetRow {
    pub id: String,
    pub folder_id: String,
    pub filename: String,
    pub content_hash: Option<Vec<u8>>,
    pub fs_file_id: Option<String>,
    pub file_size: i64,
    pub mtime_ns: i64,
    pub created_at: String,
}

pub(crate) struct AssetWrite<'a> {
    pub id: &'a str,
    pub folder_id: &'a str,
    pub filename: &'a str,
    pub hash: &'a [u8],
    pub fs_file_id: Option<&'a str>,
    pub file_size: i64,
    pub mtime_ns: i64,
    pub created_at: &'a str,
}

pub struct Database {
    pub(crate) conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    pub fn upsert_workspace(&self, id: &str, root_path: &str, created_at: &str) -> Result<()> {
        let now = now();
        self.conn.execute(
            "INSERT INTO workspaces(id, root_path, created_at, last_opened_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
                 root_path = excluded.root_path,
                 last_opened_at = excluded.last_opened_at",
            params![id, root_path, created_at, now],
        )?;
        Ok(())
    }

    pub fn ensure_folder(
        &self,
        id: &str,
        parent_id: Option<&str>,
        name: &str,
        fs_file_id: Option<&str>,
        created_at: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO folders(id, parent_id, name, fs_file_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
                 parent_id = excluded.parent_id,
                 name = excluded.name,
                 fs_file_id = excluded.fs_file_id,
                 updated_at = excluded.updated_at",
            params![id, parent_id, name, fs_file_id, created_at],
        )?;
        Ok(())
    }

    pub fn folders(&self) -> Result<Vec<FolderRow>> {
        load_folders(&self.conn)
    }

    pub fn assets(&self) -> Result<Vec<AssetRow>> {
        load_assets(&self.conn)
    }

    pub fn folder(&self, id: &str) -> Result<FolderRow> {
        self.conn
            .query_row(
                "SELECT id, parent_id, name, fs_file_id, created_at
                 FROM folders WHERE id = ?1",
                [id],
                map_folder,
            )
            .optional()?
            .ok_or_else(|| GrapesError::NotFound(format!("文件夹 {id}")))
    }

    pub fn asset(&self, id: &str) -> Result<AssetRow> {
        self.conn
            .query_row(
                "SELECT id, folder_id, filename, content_hash, fs_file_id,
                        COALESCE(file_size, 0), COALESCE(mtime_ns, 0), created_at
                 FROM assets WHERE id = ?1",
                [id],
                map_asset,
            )
            .optional()?
            .ok_or_else(|| GrapesError::NotFound(format!("照片 {id}")))
    }

    pub fn insert_folder(&self, folder: &FolderRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO folders(id, parent_id, name, fs_file_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                folder.id,
                folder.parent_id,
                folder.name,
                folder.fs_file_id,
                folder.created_at
            ],
        )?;
        Ok(())
    }

    pub fn update_folder_position(
        &self,
        id: &str,
        parent_id: Option<&str>,
        name: &str,
        fs_file_id: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE folders
             SET parent_id = ?2, name = ?3, fs_file_id = ?4, updated_at = ?5
             WHERE id = ?1",
            params![id, parent_id, name, fs_file_id, now()],
        )?;
        Ok(())
    }

    pub fn delete_folder(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM folders WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn update_asset_position(&self, id: &str, folder_id: &str, filename: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE assets
             SET folder_id = ?2, filename = ?3, updated_at = ?4
             WHERE id = ?1",
            params![id, folder_id, filename, now()],
        )?;
        Ok(())
    }

    pub fn begin_operation(
        &self,
        kind: &str,
        entity_id: &str,
        source_path: &str,
        target_path: &str,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO fs_operations(
                 id, kind, entity_id, source_path, target_path, status, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
            params![id, kind, entity_id, source_path, target_path, now()],
        )?;
        Ok(id)
    }

    pub fn finish_operation(&self, id: &str, status: &str, error: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE fs_operations
             SET status = ?2, error = ?3, completed_at = ?4
             WHERE id = ?1",
            params![id, status, error, now()],
        )?;
        Ok(())
    }

    pub fn set_asset_tags(&mut self, asset_id: &str, tags: &[String]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM asset_tags WHERE asset_id = ?1", [asset_id])?;
        let created_at = now();
        for tag in tags {
            tx.execute(
                "INSERT INTO asset_tags(asset_id, tag, created_at)
                 VALUES (?1, ?2, ?3)",
                params![asset_id, tag, created_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn mark_interrupted_operations(&self) -> Result<usize> {
        Ok(self.conn.execute(
            "UPDATE fs_operations
             SET status = 'interrupted',
                 error = '应用退出前未收到操作完成确认；启动对账将恢复文件索引',
                 completed_at = ?1
             WHERE status = 'pending'",
            [now()],
        )?)
    }

    pub fn last_reconcile(&self) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT report_json FROM reconciliation_runs
                 ORDER BY created_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?)
    }
}

fn migrate(conn: &mut Connection) -> Result<()> {
    const MIGRATIONS: &[(i64, &str)] = &[
        (1, include_str!("migrations/001_core.sql")),
        (2, include_str!("migrations/002_operations.sql")),
        (3, include_str!("migrations/003_asset_tags.sql")),
    ];

    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current > DATABASE_SCHEMA_VERSION {
        return Err(GrapesError::InvalidWorkspace(format!(
            "数据库 schema 版本 {current} 高于当前支持的版本 {DATABASE_SCHEMA_VERSION}"
        )));
    }

    for (version, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }

        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
    }

    Ok(())
}

pub(crate) fn load_folders(conn: &Connection) -> Result<Vec<FolderRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, parent_id, name, fs_file_id, created_at
         FROM folders",
    )?;
    let rows = stmt
        .query_map([], map_folder)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn load_assets(conn: &Connection) -> Result<Vec<AssetRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, folder_id, filename, content_hash, fs_file_id,
                COALESCE(file_size, 0), COALESCE(mtime_ns, 0), created_at
         FROM assets",
    )?;
    let rows = stmt
        .query_map([], map_asset)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn load_asset_tags(conn: &Connection) -> Result<BTreeMap<String, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT asset_id, tag
         FROM asset_tags
         ORDER BY asset_id, tag COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut tags: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        let (asset_id, tag) = row?;
        tags.entry(asset_id).or_default().insert(tag);
    }
    Ok(tags
        .into_iter()
        .map(|(asset_id, values)| (asset_id, values.into_iter().collect()))
        .collect())
}

pub(crate) fn insert_folder_tx(tx: &Transaction<'_>, folder: &FolderRow) -> Result<()> {
    tx.execute(
        "INSERT INTO folders(id, parent_id, name, fs_file_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![
            folder.id,
            folder.parent_id,
            folder.name,
            folder.fs_file_id,
            folder.created_at
        ],
    )?;
    Ok(())
}

pub(crate) fn update_folder_tx(
    tx: &Transaction<'_>,
    id: &str,
    parent_id: Option<&str>,
    name: &str,
    fs_file_id: Option<&str>,
) -> Result<()> {
    tx.execute(
        "UPDATE folders
         SET parent_id = ?2, name = ?3, fs_file_id = ?4, updated_at = ?5
         WHERE id = ?1",
        params![id, parent_id, name, fs_file_id, now()],
    )?;
    Ok(())
}

pub(crate) fn insert_asset_tx(tx: &Transaction<'_>, asset: &AssetWrite<'_>) -> Result<()> {
    tx.execute(
        "INSERT INTO assets(
             id, folder_id, filename, content_hash, fs_file_id, file_size,
             mtime_ns, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            asset.id,
            asset.folder_id,
            asset.filename,
            asset.hash,
            asset.fs_file_id,
            asset.file_size,
            asset.mtime_ns,
            asset.created_at
        ],
    )?;
    Ok(())
}

pub(crate) fn update_asset_tx(tx: &Transaction<'_>, asset: &AssetWrite<'_>) -> Result<()> {
    tx.execute(
        "UPDATE assets SET
             folder_id = ?2, filename = ?3, content_hash = ?4,
             fs_file_id = ?5, file_size = ?6, mtime_ns = ?7, updated_at = ?8
         WHERE id = ?1",
        params![
            asset.id,
            asset.folder_id,
            asset.filename,
            asset.hash,
            asset.fs_file_id,
            asset.file_size,
            asset.mtime_ns,
            now()
        ],
    )?;
    Ok(())
}

pub(crate) fn record_reconcile_tx(
    tx: &Transaction<'_>,
    report: &crate::types::ReconcileReport,
) -> Result<()> {
    let report_json = serde_json::to_string(report)?;
    tx.execute(
        "INSERT INTO reconciliation_runs(id, report_json, created_at)
         VALUES (?1, ?2, ?3)",
        params![
            uuid::Uuid::new_v4().to_string(),
            report_json,
            report.finished_at
        ],
    )?;
    tx.execute(
        "DELETE FROM reconciliation_runs
         WHERE id NOT IN (
             SELECT id FROM reconciliation_runs ORDER BY created_at DESC LIMIT 25
         )",
        [],
    )?;
    Ok(())
}

fn map_folder(row: &rusqlite::Row<'_>) -> rusqlite::Result<FolderRow> {
    Ok(FolderRow {
        id: row.get(0)?,
        parent_id: row.get(1)?,
        name: row.get(2)?,
        fs_file_id: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn map_asset(row: &rusqlite::Row<'_>) -> rusqlite::Result<AssetRow> {
    Ok(AssetRow {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        filename: row.get(2)?,
        content_hash: row.get(3)?,
        fs_file_id: row.get(4)?,
        file_size: row.get(5)?,
        mtime_ns: row.get(6)?,
        created_at: row.get(7)?,
    })
}

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
