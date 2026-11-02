use std::fs;
use std::path::Path;

use grapes_core::Workspace;
use rusqlite::Connection;

fn write_photo(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("photo parent");
    }
    fs::write(path, contents).expect("photo");
}

#[test]
fn initializes_sidecar_and_database_migrations() {
    let temp = tempfile::tempdir().expect("tempdir");
    let workspace = Workspace::open_and_reconcile(temp.path()).expect("open workspace");
    let snapshot = workspace.snapshot().expect("snapshot");

    for relative in [
        ".grapes/workspace.json",
        ".grapes/grapes.db",
        ".grapes/thumbnails",
        ".grapes/cache",
        ".grapes/graph/layouts",
    ] {
        assert!(
            temp.path().join(relative).exists(),
            "missing sidecar item {relative}"
        );
    }

    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(temp.path().join(".grapes/workspace.json")).expect("manifest"),
    )
    .expect("manifest json");
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(snapshot.workspace.schema_version, 3);

    let connection = Connection::open(temp.path().join(".grapes/grapes.db")).expect("database");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("schema version");
    assert_eq!(version, 3);
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table'
               AND name IN ('folders', 'assets', 'fs_operations', 'asset_tags')",
            [],
            |row| row.get(0),
        )
        .expect("table count");
    assert_eq!(table_count, 4);
}

#[test]
fn upgrades_an_existing_v1_database_in_place() {
    let temp = tempfile::tempdir().expect("tempdir");
    let grapes_dir = temp.path().join(".grapes");
    fs::create_dir_all(&grapes_dir).expect("sidecar");
    let database_path = grapes_dir.join("grapes.db");
    {
        let connection = Connection::open(&database_path).expect("database");
        connection
            .execute_batch(include_str!("../src/database/migrations/001_core.sql"))
            .expect("v1 schema");
        connection
            .pragma_update(None, "user_version", 1)
            .expect("v1 version");
    }

    let workspace = Workspace::open_and_reconcile(temp.path()).expect("upgrade");
    assert_eq!(
        workspace
            .snapshot()
            .expect("snapshot")
            .workspace
            .schema_version,
        3
    );
    let connection = Connection::open(database_path).expect("database");
    let operations_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'fs_operations'",
            [],
            |row| row.get(0),
        )
        .expect("operations table");
    assert_eq!(operations_table, 1);
}

#[test]
fn tags_are_user_readable_and_survive_file_moves() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("Inbox/photo.jpg"), b"tagged photo");
    fs::create_dir(temp.path().join("Archive")).expect("archive");

    let mut workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let before = workspace.snapshot().expect("snapshot");
    let asset_id = before.assets[0].id.clone();
    let archive_id = before
        .folders
        .iter()
        .find(|folder| folder.name == "Archive")
        .expect("archive")
        .id
        .clone();

    workspace
        .set_asset_tags(
            &asset_id,
            &["旅行".into(), " 旅行 ".into(), "Vancouver".into()],
        )
        .expect("set tags");
    workspace
        .move_asset(&asset_id, &archive_id)
        .expect("move asset");
    drop(workspace);

    let reopened = Workspace::open_and_reconcile(temp.path()).expect("reopen");
    let tagged = reopened
        .snapshot()
        .expect("snapshot")
        .assets
        .into_iter()
        .find(|asset| asset.id == asset_id)
        .expect("tagged asset");
    assert_eq!(tagged.tags, vec!["Vancouver", "旅行"]);
    assert_eq!(tagged.relative_path, "Archive/photo.jpg");
}

#[test]
fn startup_marks_crashed_operations_interrupted_before_reconcile() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("photo.jpg"), b"photo");
    let workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    drop(workspace);

    {
        let connection = Connection::open(temp.path().join(".grapes/grapes.db")).expect("database");
        connection
            .execute(
                "INSERT INTO fs_operations(
                    id, kind, entity_id, source_path, target_path, status, created_at
                 ) VALUES ('crashed', 'move_asset', NULL, 'a', 'b', 'pending', 'now')",
                [],
            )
            .expect("pending operation");
    }

    let workspace = Workspace::open_and_reconcile(temp.path()).expect("reopen");
    drop(workspace);
    let connection = Connection::open(temp.path().join(".grapes/grapes.db")).expect("database");
    let status: String = connection
        .query_row(
            "SELECT status FROM fs_operations WHERE id = 'crashed'",
            [],
            |row| row.get(0),
        )
        .expect("operation status");
    assert_eq!(status, "interrupted");
}

#[test]
fn internal_move_and_rename_preserve_asset_and_folder_ids() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("Canada/DSC001.jpg"), b"photo one");
    fs::create_dir(temp.path().join("Archive")).expect("archive");

    let mut workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let before = workspace.snapshot().expect("snapshot");
    let asset_id = before.assets[0].id.clone();
    let canada_id = before
        .folders
        .iter()
        .find(|folder| folder.name == "Canada")
        .expect("Canada")
        .id
        .clone();
    let archive_id = before
        .folders
        .iter()
        .find(|folder| folder.name == "Archive")
        .expect("Archive")
        .id
        .clone();
    let root_id = before.root_folder_id.clone();

    workspace
        .move_asset(&asset_id, &archive_id)
        .expect("move asset");
    let moved = workspace.snapshot().expect("snapshot");
    let moved_asset = moved
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .expect("stable asset");
    assert_eq!(moved_asset.relative_path, "Archive/DSC001.jpg");
    assert!(temp.path().join("Archive/DSC001.jpg").is_file());

    workspace
        .rename_asset(&asset_id, "DSC001-final.jpg")
        .expect("rename asset");
    let renamed = workspace.snapshot().expect("snapshot");
    assert_eq!(
        renamed
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .expect("stable asset")
            .relative_path,
        "Archive/DSC001-final.jpg"
    );

    workspace
        .move_folder(&canada_id, &archive_id)
        .expect("move folder");
    workspace
        .rename_folder(&canada_id, "Canada Archive")
        .expect("rename folder");
    let folder_moved = workspace.snapshot().expect("snapshot");
    assert!(folder_moved
        .folders
        .iter()
        .any(|folder| folder.id == canada_id && folder.name == "Canada Archive"));
    assert!(temp.path().join("Archive/Canada Archive").is_dir());

    workspace.reconcile().expect("reconcile");
    let after = workspace.snapshot().expect("snapshot");
    assert!(after.assets.iter().any(|asset| asset.id == asset_id));
    assert!(after.folders.iter().any(|folder| folder.id == canada_id));
    assert!(after.folders.iter().any(|folder| folder.id == root_id));

    let operation_rows: (i64, i64) = {
        let connection = Connection::open(temp.path().join(".grapes/grapes.db")).expect("database");
        connection
            .query_row(
                "SELECT
                    SUM(status = 'succeeded'),
                    SUM(status = 'pending')
                 FROM fs_operations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("operation rows")
    };
    assert_eq!(operation_rows, (4, 0));
}

#[test]
fn external_move_is_reconciled_without_changing_identity() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("Inbox/a.jpg"), b"same file");
    fs::create_dir(temp.path().join("Inbox/Nested")).expect("nested");

    let mut workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let before = workspace.snapshot().expect("snapshot");
    let asset_id = before.assets[0].id.clone();
    let inbox_id = before
        .folders
        .iter()
        .find(|folder| folder.name == "Inbox")
        .expect("Inbox")
        .id
        .clone();

    fs::rename(
        temp.path().join("Inbox/a.jpg"),
        temp.path().join("Inbox/Nested/b.jpg"),
    )
    .expect("external file move");
    fs::rename(temp.path().join("Inbox"), temp.path().join("Imported"))
        .expect("external folder move");

    let report = workspace.reconcile().expect("reconcile");
    assert!(report.assets_moved >= 1);
    assert!(report.folders_moved >= 1);

    let after = workspace.snapshot().expect("snapshot");
    let asset = after
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .expect("asset identity preserved");
    assert_eq!(asset.relative_path, "Imported/Nested/b.jpg");
    assert!(after
        .folders
        .iter()
        .any(|folder| folder.id == inbox_id && folder.name == "Imported"));
}

#[test]
fn startup_reconcile_recovers_missed_external_rename() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("original.jpg"), b"startup recovery");

    let workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let original_id = workspace.snapshot().expect("snapshot").assets[0].id.clone();
    drop(workspace);

    fs::rename(
        temp.path().join("original.jpg"),
        temp.path().join("renamed-while-closed.jpg"),
    )
    .expect("external rename");

    let reopened = Workspace::open_and_reconcile(temp.path()).expect("reopen");
    let snapshot = reopened.snapshot().expect("snapshot");
    assert_eq!(snapshot.assets.len(), 1);
    assert_eq!(snapshot.assets[0].id, original_id);
    assert_eq!(snapshot.assets[0].filename, "renamed-while-closed.jpg");
}

#[test]
fn copied_photo_does_not_steal_identity_from_the_original() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("original.jpg"), b"same bytes");
    let mut workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let original_id = workspace.snapshot().expect("snapshot").assets[0].id.clone();

    // The copy sorts before the original, exercising the hash fallback order.
    fs::copy(
        temp.path().join("original.jpg"),
        temp.path().join("0-copy.jpg"),
    )
    .expect("copy");
    workspace.reconcile().expect("reconcile");

    let snapshot = workspace.snapshot().expect("snapshot");
    assert_eq!(snapshot.assets.len(), 2);
    assert_eq!(
        snapshot
            .assets
            .iter()
            .find(|asset| asset.filename == "original.jpg")
            .expect("original")
            .id,
        original_id
    );
    assert_ne!(
        snapshot
            .assets
            .iter()
            .find(|asset| asset.filename == "0-copy.jpg")
            .expect("copy")
            .id,
        original_id
    );
}

#[test]
fn folder_and_file_operations_roll_back_on_name_conflicts() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("Source/a.jpg"), b"a");
    write_photo(&temp.path().join("Destination/a.jpg"), b"different");

    let mut workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let snapshot = workspace.snapshot().expect("snapshot");
    let source_asset = snapshot
        .assets
        .iter()
        .find(|asset| asset.relative_path == "Source/a.jpg")
        .expect("source asset")
        .id
        .clone();
    let destination_id = snapshot
        .folders
        .iter()
        .find(|folder| folder.name == "Destination")
        .expect("destination")
        .id
        .clone();

    let error = workspace
        .move_asset(&source_asset, &destination_id)
        .expect_err("collision must fail");
    assert!(error.to_string().contains("目标位置"));
    assert!(temp.path().join("Source/a.jpg").is_file());

    let after = workspace.snapshot().expect("snapshot");
    assert_eq!(
        after
            .assets
            .iter()
            .find(|asset| asset.id == source_asset)
            .expect("asset")
            .relative_path,
        "Source/a.jpg"
    );
}
