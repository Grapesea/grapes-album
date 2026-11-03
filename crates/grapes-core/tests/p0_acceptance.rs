use std::fs;
use std::path::Path;

use std::collections::HashMap;

use grapes_core::{
    semantic::{classification_alias, fuse_plan_search_relaxed, fuse_search_results},
    GraphNodeKind, SearchPlan, SearchPlanFilter, SemanticAssetRecord, SemanticProviderConfig,
    SmartGraphFilter, Workspace,
};
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
    assert_eq!(snapshot.workspace.schema_version, 10);

    let connection = Connection::open(temp.path().join(".grapes/grapes.db")).expect("database");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("schema version");
    assert_eq!(version, 10);
    let table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table'
               AND name IN (
                   'folders', 'assets', 'fs_operations', 'asset_tags',
                   'graphs', 'graph_manual_members', 'graph_positions',
                   'dimensions', 'dimension_values', 'classifications',
                   'semantic_provider_configs', 'embedding_models',
                   'asset_embeddings', 'ask_grapes_provider_configs'
               )",
            [],
            |row| row.get(0),
        )
        .expect("table count");
    assert_eq!(table_count, 14);
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
        10
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
fn restores_ask_provider_settings_from_an_interim_schema_8_workspace() {
    let temp = tempfile::tempdir().expect("tempdir");
    let grapes_dir = temp.path().join(".grapes");
    fs::create_dir_all(&grapes_dir).expect("sidecar");
    let database_path = grapes_dir.join("grapes.db");
    {
        let connection = Connection::open(&database_path).expect("database");
        for migration in [
            include_str!("../src/database/migrations/001_core.sql"),
            include_str!("../src/database/migrations/002_operations.sql"),
            include_str!("../src/database/migrations/003_asset_tags.sql"),
            include_str!("../src/database/migrations/004_graphspaces.sql"),
            include_str!("../src/database/migrations/005_classifications.sql"),
            include_str!("../src/database/migrations/006_semantic_search.sql"),
        ] {
            connection
                .execute_batch(migration)
                .expect("schema migration");
        }
        connection
            .execute(
                "ALTER TABLE semantic_provider_configs ADD COLUMN api_key TEXT",
                [],
            )
            .expect("interim api key column");
        connection
            .pragma_update(None, "user_version", 8)
            .expect("interim schema version");
    }

    let workspace = Workspace::open_and_reconcile(temp.path()).expect("upgrade");
    assert_eq!(
        workspace
            .snapshot()
            .expect("snapshot")
            .workspace
            .schema_version,
        10
    );
    assert_eq!(
        workspace
            .ask_grapes_provider_config()
            .expect("restored Ask Grapes settings")
            .model,
        "Qwen/Qwen3.5-4B"
    );
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
fn exif_metadata_is_persisted_in_the_asset_snapshot() {
    let temp = tempfile::tempdir().expect("tempdir");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exif.jpg");
    fs::copy(fixture, temp.path().join("exif.jpg")).expect("copy fixture");

    let workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let asset = &workspace.snapshot().expect("snapshot").assets[0];
    assert_eq!(asset.metadata.width, Some(15));
    assert_eq!(asset.metadata.height, Some(7));
    assert_eq!(
        asset.metadata.captured_at.as_deref(),
        Some("2016-05-04T03:02:01")
    );
    assert!(asset.metadata.scanned);
}

#[test]
fn reconciliation_generates_and_reuses_thumbnail_cache() {
    let temp = tempfile::tempdir().expect("tempdir");
    image::RgbImage::from_fn(900, 600, |x, y| {
        image::Rgb([(x % 255) as u8, (y % 255) as u8, 90])
    })
    .save(temp.path().join("large.jpg"))
    .expect("save photo");

    let mut workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let first = workspace.snapshot().expect("snapshot");
    assert!(first.assets[0].has_thumbnail);
    assert!(
        first
            .last_reconcile
            .as_ref()
            .expect("report")
            .thumbnails_created
            >= 1
    );

    let asset_id = first.assets[0].id.clone();
    let thumbnail_path = workspace
        .asset_thumbnail_path(&asset_id)
        .expect("thumbnail");
    assert!(thumbnail_path.is_file());
    assert_eq!(
        workspace
            .asset_path(&asset_id)
            .expect("original asset path"),
        temp.path().join("large.jpg")
    );
    let second_report = workspace.reconcile().expect("second reconcile");
    assert_eq!(second_report.thumbnails_created, 0);
}

#[test]
fn graphspaces_build_automatic_metadata_graphs_and_persist_positions() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("Inbox/one.jpg"), b"one");
    write_photo(&temp.path().join("Inbox/two.jpg"), b"two");
    fs::create_dir(temp.path().join("Archive")).expect("archive");

    let mut workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let initial = workspace.snapshot().expect("snapshot");
    {
        let connection = Connection::open(temp.path().join(".grapes/grapes.db")).expect("database");
        connection
            .execute(
                "UPDATE assets SET
                    captured_at = '2026-07-21T20:30:00',
                    camera_model = 'Sony ZV-E10',
                    lens_model = 'Sigma 18-50'
                 WHERE id = ?1",
                [&initial.assets[0].id],
            )
            .expect("metadata one");
        connection
            .execute(
                "UPDATE assets SET
                    captured_at = '2026-08-02T09:00:00',
                    camera_model = 'Fujifilm X-T5',
                    lens_model = 'XF 33mm'
                 WHERE id = ?1",
                [&initial.assets[1].id],
            )
            .expect("metadata two");
    }

    let catalog = workspace.graph_catalog().expect("catalog");
    assert!(catalog.iter().any(|graph| graph.id == "graphspaces"));
    assert!(catalog
        .iter()
        .any(|graph| graph.kind == "dimension" && graph.dimension.as_deref() == Some("date")));
    assert!(catalog
        .iter()
        .any(|graph| graph.kind == "dimension" && graph.dimension.as_deref() == Some("camera")));
    assert!(catalog
        .iter()
        .any(|graph| graph.kind == "dimension" && graph.dimension.as_deref() == Some("lens")));
    assert!(catalog
        .iter()
        .any(|graph| graph.name == "2026-07" && graph.photo_count == 1));
    assert!(catalog
        .iter()
        .any(|graph| graph.name == "Sony ZV-E10" && graph.photo_count == 1));

    let root = workspace.graph_view(None).expect("root graph");
    let root_dummy = root
        .nodes
        .iter()
        .find(|node| node.id == "portal:graphspaces")
        .expect("root dummy");
    assert!(root_dummy.dummy);
    assert!(root_dummy.target_graph_id.is_none());
    for dimension in ["date", "camera", "lens"] {
        let node = root
            .nodes
            .iter()
            .find(|node| {
                node.label.eq_ignore_ascii_case(dimension)
                    || node.id == format!("portal:graph:dimension:{dimension}")
            })
            .expect("dimension portal");
        assert!(node.available);
        assert!(node.target_graph_id.is_some());
    }

    let date_graph = catalog
        .iter()
        .find(|graph| graph.id == "graph:dimension:date")
        .expect("date graph");
    let date_view = workspace
        .graph_view(Some(&date_graph.id))
        .expect("date view");
    assert!(date_view.nodes.first().expect("date dummy").dummy);
    assert_eq!(
        date_view.breadcrumbs.last().map(|graph| graph.id.as_str()),
        Some("graph:dimension:date")
    );
    assert!(date_view
        .nodes
        .iter()
        .any(|node| node.label == "2026-07" && node.target_graph_id.is_some()));

    let month_graph = catalog
        .iter()
        .find(|graph| graph.name == "2026-07")
        .expect("month graph");
    let month_view = workspace
        .graph_view(Some(&month_graph.id))
        .expect("month view");
    assert!(month_view.nodes.iter().any(|node| {
        node.kind == GraphNodeKind::Asset
            && !node.dummy
            && node.asset_id.as_deref() == Some(initial.assets[0].id.as_str())
    }));

    let asset_node = month_view
        .nodes
        .iter()
        .find(|node| node.kind == GraphNodeKind::Asset)
        .expect("asset node");
    workspace
        .save_graph_position(&month_graph.id, &asset_node.id, 120.5, -48.0, true)
        .expect("save position");
    let restored = workspace
        .graph_view(Some(&month_graph.id))
        .expect("restored view");
    let restored_node = restored
        .nodes
        .iter()
        .find(|node| node.id == asset_node.id)
        .expect("restored node");
    assert_eq!(restored_node.x, Some(120.5));
    assert_eq!(restored_node.y, Some(-48.0));
    assert!(restored_node.pinned);

    let archive_id = initial
        .folders
        .iter()
        .find(|folder| folder.name == "Archive")
        .expect("archive")
        .id
        .clone();
    workspace
        .move_asset(&initial.assets[0].id, &archive_id)
        .expect("move asset");
    let after_move = workspace
        .graph_view(Some(&month_graph.id))
        .expect("graph after move");
    assert!(after_move.nodes.iter().any(|node| {
        node.kind == GraphNodeKind::Asset
            && node.asset_id.as_deref() == Some(initial.assets[0].id.as_str())
    }));
}

#[test]
fn classifications_custom_dimensions_and_smart_graphs_persist() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("rain.jpg"), b"rainy photo");

    let mut workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let asset_id = workspace.snapshot().expect("snapshot").assets[0].id.clone();
    let schema = workspace.classification_schema().expect("schema");
    assert!(schema.iter().any(|dimension| dimension.kind == "time"));
    assert!(schema.iter().any(|dimension| dimension.kind == "weather"));
    assert!(schema.iter().any(|dimension| dimension.kind == "type"));
    assert!(schema.iter().any(|dimension| dimension.kind == "tone"));

    workspace
        .set_asset_classification(&asset_id, "dimension:time:night", true)
        .expect("time classification");
    workspace
        .set_asset_classification(&asset_id, "dimension:weather:rain", true)
        .expect("weather classification");
    workspace
        .set_asset_classification(&asset_id, "dimension:type:street", true)
        .expect("type classification");
    workspace
        .set_asset_tags(&asset_id, &["旅行".into()])
        .expect("tags for graph");
    let classified = workspace.snapshot().expect("snapshot");
    assert_eq!(classified.assets[0].classifications.len(), 3);
    assert!(classified.assets[0]
        .classifications
        .iter()
        .all(|classification| classification.source == "manual"));

    let mood = workspace
        .create_custom_dimension("Mood")
        .expect("custom dimension");
    let atmospheric = workspace
        .create_dimension_value(&mood.id, "Atmospheric")
        .expect("custom value");
    workspace
        .set_asset_classification(&asset_id, &atmospheric.id, true)
        .expect("custom classification");

    let catalog = workspace.graph_catalog().expect("catalog");
    assert!(catalog
        .iter()
        .any(|graph| graph.kind == "dimension" && graph.dimension.as_deref() == Some("time")));
    assert!(catalog
        .iter()
        .any(|graph| graph.name == "Night" && graph.photo_count == 1));
    assert!(catalog
        .iter()
        .any(|graph| graph.name == "Rain" && graph.photo_count == 1));
    assert!(catalog
        .iter()
        .any(|graph| graph.name == "Mood" && graph.child_count == 1));
    let tag_graph = catalog
        .iter()
        .find(|graph| {
            graph.kind == "value"
                && graph.dimension.as_deref() == Some("tags")
                && graph.name == "旅行"
        })
        .expect("tag value graph");
    assert_eq!(tag_graph.parent_id.as_deref(), Some("graphspaces"));
    assert_eq!(tag_graph.photo_count, 1);
    assert!(!catalog
        .iter()
        .any(|graph| graph.kind == "dimension" && graph.dimension.as_deref() == Some("tags")));
    let tag_view = workspace
        .graph_view(Some(&tag_graph.id))
        .expect("tag graph view");
    assert!(tag_view
        .nodes
        .iter()
        .any(|node| node.kind == GraphNodeKind::Asset && node.photo_count == 1));
    let root = workspace.graph_view(None).expect("root graph");
    assert!(!root
        .nodes
        .iter()
        .any(|node| node.id == "portal:graph:dimension:custom"));
    assert!(root
        .nodes
        .iter()
        .any(|node| node.label == "Mood" && node.target_graph_id.is_some()));
    let tag_portal = root
        .nodes
        .iter()
        .find(|node| node.label == "旅行")
        .expect("tag graph portal");
    assert!(tag_portal.available);
    assert_eq!(
        tag_portal.target_graph_id.as_deref(),
        Some(tag_graph.id.as_str())
    );
    assert!(!root
        .nodes
        .iter()
        .any(|node| node.id == "portal:graph:dimension:tags"));

    let smart = workspace
        .create_smart_graph(
            "Rainy Night",
            &[
                SmartGraphFilter {
                    dimension_id: "dimension:time".into(),
                    value_ids: vec!["dimension:time:night".into()],
                },
                SmartGraphFilter {
                    dimension_id: "dimension:weather".into(),
                    value_ids: vec!["dimension:weather:rain".into()],
                },
            ],
        )
        .expect("smart graph");
    assert_eq!(smart.kind, "smart");
    assert_eq!(smart.photo_count, 1);
    let smart_view = workspace
        .graph_view(Some(&smart.id))
        .expect("smart graph view");
    assert!(smart_view.nodes.iter().any(|node| {
        node.kind == GraphNodeKind::Asset && node.asset_id.as_deref() == Some(asset_id.as_str())
    }));
    workspace
        .save_graph_position(&smart.id, "asset:test", 24.0, -36.0, true)
        .expect("smart graph position");
    let catalog_after_delete = workspace
        .delete_smart_graph(&smart.id)
        .expect("delete smart graph");
    assert!(!catalog_after_delete
        .iter()
        .any(|graph| graph.id == smart.id));
    assert!(workspace.graph_view(Some(&smart.id)).is_err());

    workspace
        .set_asset_classification(&asset_id, "dimension:weather:rain", false)
        .expect("remove classification");
    let after_removal = workspace.snapshot().expect("snapshot");
    assert!(!after_removal.assets[0]
        .classifications
        .iter()
        .any(|classification| classification.value_id == "dimension:weather:rain"));
}

#[test]
fn semantic_provider_vectors_and_fused_search_persist_by_model() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("rain.jpg"), b"rainy photo");
    let workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let asset_id = workspace.snapshot().expect("snapshot").assets[0].id.clone();

    let configs = workspace
        .semantic_provider_configs()
        .expect("semantic settings");
    let embedding = configs
        .iter()
        .find(|config| config.kind == "embedding")
        .expect("embedding settings")
        .clone();
    assert_eq!(embedding.provider_type, "siliconflow");
    assert!(embedding.model.contains("Qwen3-VL-Embedding"));
    assert!(!embedding.enabled);
    assert!(configs
        .iter()
        .find(|config| config.kind == "rerank")
        .is_some_and(|config| !config.enabled));
    let ask_grapes = workspace
        .ask_grapes_provider_config()
        .expect("Ask Grapes settings");
    assert_eq!(ask_grapes.model, "Qwen/Qwen3.5-4B");
    assert_eq!(ask_grapes.provider_type, "siliconflow");
    assert_eq!(ask_grapes.request_timeout_ms, 90_000);
    assert_eq!(ask_grapes.max_concurrency, 2);
    assert_eq!(ask_grapes.max_retries, 2);
    let custom = SemanticProviderConfig {
        enabled: true,
        base_url: "https://example.test/v1".into(),
        model: "custom-vl-embedding".into(),
        api_key: Some("test-secret-key".into()),
        request_timeout_ms: 60_000,
        max_concurrency: 4,
        max_retries: 3,
        ..embedding
    };
    workspace
        .save_semantic_provider(&custom)
        .expect("save embedding config");
    let saved = workspace
        .semantic_provider_configs()
        .expect("saved settings")
        .into_iter()
        .find(|config| config.kind == "embedding")
        .expect("embedding");
    assert_eq!(saved.base_url, "https://example.test/v1");
    assert_eq!(saved.model, "custom-vl-embedding");
    assert_eq!(saved.api_key.as_deref(), Some("test-secret-key"));
    assert_eq!(saved.request_timeout_ms, 60_000);
    assert_eq!(saved.max_concurrency, 4);
    assert_eq!(saved.max_retries, 3);

    let model_id = workspace
        .ensure_semantic_model(&custom, 3)
        .expect("embedding model");
    workspace
        .store_semantic_embedding(&asset_id, &model_id, &[1.0, 0.0, 0.0])
        .expect("store embedding");
    let status = workspace
        .semantic_index_status(&custom)
        .expect("index status");
    assert_eq!(status.indexed_assets, 1);
    assert_eq!(status.pending_assets, 0);
    assert_eq!(status.dimension, Some(3));

    let records = vec![
        SemanticAssetRecord {
            asset_id: asset_id.clone(),
            filename: "rain.jpg".into(),
            relative_path: "rain.jpg".into(),
            combined_text: "rain.jpg 雨天 冷色调 风光".into(),
            classification_search_terms: vec!["Weather: Rain".into(), "雨".into(), "冷色".into()],
            classification_values: vec![],
            metadata_text: "rain.jpg".into(),
            thumbnail_path: None,
        },
        SemanticAssetRecord {
            asset_id: "other".into(),
            filename: "sun.jpg".into(),
            relative_path: "sun.jpg".into(),
            combined_text: "sun.jpg".into(),
            classification_search_terms: vec!["Weather: Sunny".into(), "晴".into()],
            classification_values: vec![],
            metadata_text: "sun.jpg".into(),
            thumbnail_path: None,
        },
    ];
    let embeddings = HashMap::from([
        (asset_id.clone(), vec![1.0, 0.0, 0.0]),
        ("other".into(), vec![0.0, 1.0, 0.0]),
    ]);
    let results = fuse_search_results(
        "寻找雨天冷色调相片",
        &records,
        &embeddings,
        &[1.0, 0.0, 0.0],
        10,
    );
    assert_eq!(results[0].asset_id, asset_id);
    assert!(results[0]
        .match_reasons
        .iter()
        .any(|reason| reason.kind == "classification"));
    assert_eq!(
        classification_alias("weather", "dimension:weather:rain"),
        Some("雨")
    );
    let overly_strict_plan = SearchPlan {
        hard_filters: vec![SearchPlanFilter {
            dimension_id: "dimension:weather".into(),
            dimension_name: "Weather".into(),
            value_ids: vec!["dimension:weather:snow".into()],
            value_names: vec!["Snow".into()],
        }],
        semantic_queries: vec!["雨天冷色调相片".into()],
        ..SearchPlan::default()
    };
    let (relaxed_results, relaxation) = fuse_plan_search_relaxed(
        &overly_strict_plan,
        &records,
        &embeddings,
        Some(&[1.0, 0.0, 0.0]),
        10,
    );
    assert!(relaxation.is_some());
    assert!(!relaxed_results.is_empty());

    drop(workspace);
    let reopened = Workspace::open(temp.path()).expect("reopen workspace");
    assert_eq!(
        reopened
            .semantic_provider_configs()
            .expect("reloaded provider configs")
            .into_iter()
            .find(|config| config.kind == "embedding")
            .and_then(|config| config.api_key)
            .as_deref(),
        Some("test-secret-key")
    );
    let mut ask_grapes = reopened
        .ask_grapes_provider_config()
        .expect("reloaded Ask Grapes config");
    ask_grapes.api_key = Some("persisted-ask-key".into());
    reopened
        .save_semantic_provider(&ask_grapes)
        .expect("save Ask Grapes key");
    assert_eq!(
        reopened
            .ask_grapes_provider_config()
            .expect("saved Ask Grapes config")
            .api_key
            .as_deref(),
        Some("persisted-ask-key")
    );
}

#[test]
fn semantic_neighbors_add_layout_weight_without_user_relations() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_photo(&temp.path().join("a.jpg"), b"first");
    write_photo(&temp.path().join("b.jpg"), b"second");
    let mut workspace = Workspace::open_and_reconcile(temp.path()).expect("workspace");
    let assets = workspace.snapshot().expect("snapshot").assets;
    assert_eq!(assets.len(), 2);
    for asset in &assets {
        workspace
            .set_asset_tags(&asset.id, &["series".into()])
            .expect("tag");
    }
    let embedding = workspace
        .semantic_provider_configs()
        .expect("provider settings")
        .into_iter()
        .find(|config| config.kind == "embedding")
        .expect("embedding");
    let model_id = workspace
        .ensure_semantic_model(&embedding, 2)
        .expect("model");
    workspace
        .store_semantic_embedding(&assets[0].id, &model_id, &[1.0, 0.0])
        .expect("first vector");
    workspace
        .store_semantic_embedding(&assets[1].id, &model_id, &[0.99, 0.01])
        .expect("second vector");

    let tag_graph = workspace
        .graph_catalog()
        .expect("catalog")
        .into_iter()
        .find(|graph| graph.kind == "value" && graph.name == "series")
        .expect("tag graph");
    let view = workspace.graph_view(Some(&tag_graph.id)).expect("tag view");
    let semantic_edge = view
        .edges
        .iter()
        .find(|edge| edge.id.starts_with("semantic:"))
        .expect("semantic neighbor edge");
    assert!(semantic_edge.weight > 0.9);
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
