use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use grapes_core::{
    semantic::{fuse_search_results, RerankCandidate, SemanticProviderClient},
    DimensionView, GraphInfo, GraphView, SemanticIndexReport, SemanticIndexStatus,
    SemanticProviderConfig, SemanticSearchResponse, SmartGraphFilter, Workspace, WorkspaceSnapshot,
    WorkspaceWatcher,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

struct Session {
    workspace: Arc<Mutex<Workspace>>,
    _watcher: WorkspaceWatcher,
}

#[derive(Default)]
struct AppState {
    session: Mutex<Option<Session>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticSettingsView {
    qa: SemanticProviderConfig,
    embedding: SemanticProviderConfig,
    rerank: SemanticProviderConfig,
    index_status: SemanticIndexStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticIndexResult {
    report: SemanticIndexReport,
    status: SemanticIndexStatus,
}

#[tauri::command]
fn open_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = Workspace::open(path).map_err(error_string)?;
    let workspace = Arc::new(Mutex::new(workspace));

    let event_app = app.clone();
    let watcher = WorkspaceWatcher::start(workspace.clone(), move |result| match result {
        Ok(report) => {
            let _ = event_app.emit("workspace-changed", report);
        }
        Err(error) => {
            let _ = event_app.emit("workspace-error", error);
        }
    })
    .map_err(error_string)?;

    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .reconcile()
        .map_err(error_string)?;
    let snapshot = read_snapshot(&workspace)?;
    let mut session = state
        .session
        .lock()
        .map_err(|_| "应用状态锁已损坏".to_owned())?;
    *session = Some(Session {
        workspace,
        _watcher: watcher,
    });
    Ok(snapshot)
}

#[tauri::command]
fn workspace_snapshot(
    state: State<'_, AppState>,
) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace(&state)?;
    read_snapshot(&workspace)
}

#[tauri::command]
fn refresh_workspace(state: State<'_, AppState>) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .reconcile()
        .map_err(error_string)?;
    read_snapshot(&workspace)
}

#[tauri::command]
fn create_folder(
    state: State<'_, AppState>,
    parent_folder_id: String,
    name: String,
) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .create_folder(&parent_folder_id, &name)
        .map_err(error_string)?;
    read_snapshot(&workspace)
}

#[tauri::command]
fn move_folder(
    state: State<'_, AppState>,
    folder_id: String,
    destination_folder_id: String,
) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .move_folder(&folder_id, &destination_folder_id)
        .map_err(error_string)?;
    read_snapshot(&workspace)
}

#[tauri::command]
fn rename_folder(
    state: State<'_, AppState>,
    folder_id: String,
    new_name: String,
) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .rename_folder(&folder_id, &new_name)
        .map_err(error_string)?;
    read_snapshot(&workspace)
}

#[tauri::command]
fn move_asset(
    state: State<'_, AppState>,
    asset_id: String,
    destination_folder_id: String,
) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .move_asset(&asset_id, &destination_folder_id)
        .map_err(error_string)?;
    read_snapshot(&workspace)
}

#[tauri::command]
fn rename_asset(
    state: State<'_, AppState>,
    asset_id: String,
    new_name: String,
) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .rename_asset(&asset_id, &new_name)
        .map_err(error_string)?;
    read_snapshot(&workspace)
}

#[tauri::command]
fn set_asset_tags(
    state: State<'_, AppState>,
    asset_id: String,
    tags: Vec<String>,
) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .set_asset_tags(&asset_id, &tags)
        .map_err(error_string)?;
    read_snapshot(&workspace)
}

#[tauri::command]
fn graph_catalog(state: State<'_, AppState>) -> std::result::Result<Vec<GraphInfo>, String> {
    let workspace = active_workspace(&state)?;
    let catalog = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .graph_catalog()
        .map_err(error_string)?;
    Ok(catalog)
}

#[tauri::command]
fn graph_view(
    state: State<'_, AppState>,
    graph_id: Option<String>,
) -> std::result::Result<GraphView, String> {
    let workspace = active_workspace(&state)?;
    let view = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .graph_view(graph_id.as_deref())
        .map_err(error_string)?;
    Ok(view)
}

#[tauri::command]
fn save_graph_position(
    state: State<'_, AppState>,
    graph_id: String,
    node_key: String,
    x: f64,
    y: f64,
    pinned: bool,
) -> std::result::Result<(), String> {
    let workspace = active_workspace(&state)?;
    let result = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .save_graph_position(&graph_id, &node_key, x, y, pinned)
        .map_err(error_string);
    result
}

#[tauri::command]
fn classification_schema(
    state: State<'_, AppState>,
) -> std::result::Result<Vec<DimensionView>, String> {
    let workspace = active_workspace(&state)?;
    let schema = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .classification_schema()
        .map_err(error_string)?;
    Ok(schema)
}

#[tauri::command]
fn set_asset_classification(
    state: State<'_, AppState>,
    asset_id: String,
    value_id: String,
    selected: bool,
) -> std::result::Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .set_asset_classification(&asset_id, &value_id, selected)
        .map_err(error_string)?;
    read_snapshot(&workspace)
}

#[tauri::command]
fn create_custom_dimension(
    state: State<'_, AppState>,
    name: String,
) -> std::result::Result<Vec<DimensionView>, String> {
    let workspace = active_workspace(&state)?;
    let schema = {
        let mut workspace = workspace.lock().map_err(|_| "工作区锁已损坏".to_owned())?;
        workspace
            .create_custom_dimension(&name)
            .map_err(error_string)?;
        workspace.classification_schema().map_err(error_string)?
    };
    Ok(schema)
}

#[tauri::command]
fn create_dimension_value(
    state: State<'_, AppState>,
    dimension_id: String,
    name: String,
) -> std::result::Result<Vec<DimensionView>, String> {
    let workspace = active_workspace(&state)?;
    let schema = {
        let mut workspace = workspace.lock().map_err(|_| "工作区锁已损坏".to_owned())?;
        workspace
            .create_dimension_value(&dimension_id, &name)
            .map_err(error_string)?;
        workspace.classification_schema().map_err(error_string)?
    };
    Ok(schema)
}

#[tauri::command]
fn create_smart_graph(
    state: State<'_, AppState>,
    name: String,
    filters: Vec<SmartGraphFilter>,
) -> std::result::Result<GraphInfo, String> {
    let workspace = active_workspace(&state)?;
    let graph_info = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .create_smart_graph(&name, &filters)
        .map_err(error_string)?;
    Ok(graph_info)
}

#[tauri::command]
fn delete_smart_graph(
    state: State<'_, AppState>,
    graph_id: String,
) -> std::result::Result<Vec<GraphInfo>, String> {
    let workspace = active_workspace(&state)?;
    let catalog = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .delete_smart_graph(&graph_id)
        .map_err(error_string);
    catalog
}

fn semantic_settings_for(
    workspace: &Arc<Mutex<Workspace>>,
) -> std::result::Result<SemanticSettingsView, String> {
    let workspace = workspace.lock().map_err(|_| "工作区锁已损坏".to_owned())?;
    let configs = workspace
        .semantic_provider_configs()
        .map_err(error_string)?;
    let embedding = configs
        .iter()
        .find(|config| config.kind == "embedding")
        .cloned()
        .ok_or_else(|| "缺少 Embedding Provider 配置".to_owned())?;
    let rerank = configs
        .iter()
        .find(|config| config.kind == "rerank")
        .cloned()
        .ok_or_else(|| "缺少 Rerank Provider 配置".to_owned())?;
    let qa = workspace
        .ask_grapes_provider_config()
        .map_err(error_string)?;
    let index_status = workspace
        .semantic_index_status(&embedding)
        .map_err(error_string)?;
    Ok(SemanticSettingsView {
        qa,
        embedding,
        rerank,
        index_status,
    })
}

#[tauri::command]
fn semantic_settings(
    state: State<'_, AppState>,
) -> std::result::Result<SemanticSettingsView, String> {
    let workspace = active_workspace(&state)?;
    semantic_settings_for(&workspace)
}

#[tauri::command]
fn save_semantic_provider(
    state: State<'_, AppState>,
    config: SemanticProviderConfig,
) -> std::result::Result<SemanticSettingsView, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .save_semantic_provider(&config)
        .map_err(error_string)?;
    semantic_settings_for(&workspace)
}

#[tauri::command]
async fn test_semantic_provider(
    state: State<'_, AppState>,
    kind: String,
) -> std::result::Result<String, String> {
    let workspace = active_workspace(&state)?;
    let config = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .semantic_provider_configs()
        .map_err(error_string)?
        .into_iter()
        .find(|config| config.kind == kind)
        .ok_or_else(|| format!("Provider {kind} 不存在"))?;
    if !config.enabled {
        return Ok("Provider 已关闭".into());
    }
    let api_key = config
        .api_key
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "请先填写 API Key".to_owned())?;
    let client = SemanticProviderClient::new().map_err(error_string)?;
    if kind == "embedding" {
        let vector = client
            .embed_text(&config, &api_key, "Grapes semantic search connection test")
            .await
            .map_err(error_string)?;
        Ok(format!("连接成功，向量维度 {}", vector.len()))
    } else {
        let result = client
            .rerank(
                &config,
                &api_key,
                "connection test",
                &[RerankCandidate {
                    asset_id: "test".into(),
                    text: "connection test image".into(),
                    image_jpeg: None,
                }],
            )
            .await
            .map_err(error_string)?;
        Ok(format!("连接成功，Rerank 模式 {}", result.mode))
    }
}

#[tauri::command]
async fn rebuild_semantic_index(
    state: State<'_, AppState>,
) -> std::result::Result<SemanticIndexResult, String> {
    let workspace = active_workspace(&state)?;
    let config = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .semantic_provider_configs()
        .map_err(error_string)?
        .into_iter()
        .find(|config| config.kind == "embedding")
        .ok_or_else(|| "缺少 Embedding Provider 配置".to_owned())?;
    if !config.enabled {
        return Err("Embedding Provider 已关闭".into());
    }
    let api_key = config
        .api_key
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "请先填写 Embedding API Key".to_owned())?;
    let client = SemanticProviderClient::new().map_err(error_string)?;
    let status = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .semantic_index_status(&config)
        .map_err(error_string)?;
    let sources = if let Some(model_id) = status.model_id.as_deref() {
        workspace
            .lock()
            .map_err(|_| "工作区锁已损坏".to_owned())?
            .pending_semantic_index_sources(model_id)
            .map_err(error_string)?
    } else {
        workspace
            .lock()
            .map_err(|_| "工作区锁已损坏".to_owned())?
            .semantic_index_sources()
            .map_err(error_string)?
    };
    let mut report = SemanticIndexReport::default();
    let mut model = status
        .model_id
        .clone()
        .zip(status.dimension)
        .map(|(id, dimension)| (id, dimension));
    let mut last_error = None;
    for source in sources {
        let bytes = match std::fs::read(&source.thumbnail_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                report.failed += 1;
                report
                    .warnings
                    .push(format!("{}: {error}", source.thumbnail_path.display()));
                continue;
            }
        };
        match client.embed_image(&config, &api_key, &bytes).await {
            Ok(vector) => {
                let dimension = vector.len();
                if let Some((_, existing_dimension)) = &model {
                    if *existing_dimension != dimension {
                        last_error = Some(format!(
                            "Embedding 维度从 {existing_dimension} 变为 {dimension}，请检查 Provider 配置"
                        ));
                        report.failed += 1;
                        continue;
                    }
                } else {
                    let id = workspace
                        .lock()
                        .map_err(|_| "工作区锁已损坏".to_owned())?
                        .ensure_semantic_model(&config, dimension)
                        .map_err(error_string)?;
                    model = Some((id, dimension));
                }
                let model_id = &model.as_ref().expect("model initialized").0;
                workspace
                    .lock()
                    .map_err(|_| "工作区锁已损坏".to_owned())?
                    .store_semantic_embedding(&source.asset_id, model_id, &vector)
                    .map_err(error_string)?;
                report.indexed += 1;
            }
            Err(error) => {
                last_error = Some(error.to_string());
                report.failed += 1;
                if report.warnings.len() < 8 {
                    report
                        .warnings
                        .push(format!("{}: {error}", source.asset_id));
                }
            }
        }
    }
    if report.indexed == 0 && report.failed > 0 {
        return Err(last_error.unwrap_or_else(|| "Embedding 索引失败".into()));
    }
    let status = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .semantic_index_status(&config)
        .map_err(error_string)?;
    Ok(SemanticIndexResult { report, status })
}

#[tauri::command]
async fn semantic_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> std::result::Result<SemanticSearchResponse, String> {
    let query = query.trim().to_owned();
    if query.is_empty() {
        return Err("请输入要查找的照片描述".into());
    }
    let workspace = active_workspace(&state)?;
    let configs = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .semantic_provider_configs()
        .map_err(error_string)?;
    let embedding_config = configs
        .iter()
        .find(|config| config.kind == "embedding")
        .cloned()
        .ok_or_else(|| "缺少 Embedding Provider 配置".to_owned())?;
    let rerank_config = configs
        .iter()
        .find(|config| config.kind == "rerank")
        .cloned();
    if !embedding_config.enabled {
        return Err("Embedding Provider 已关闭".into());
    }
    let embedding_key = embedding_config.api_key.clone();
    let rerank_key = rerank_config
        .as_ref()
        .and_then(|config| config.api_key.clone());
    let embedding_key = embedding_key.ok_or_else(|| "请先填写 Embedding API Key".to_owned())?;
    let client = SemanticProviderClient::new().map_err(error_string)?;
    let query_vector = client
        .embed_text(&embedding_config, &embedding_key, &query)
        .await
        .map_err(error_string)?;

    let (assets, embeddings, status) = {
        let workspace = workspace.lock().map_err(|_| "工作区锁已损坏".to_owned())?;
        let model_id = workspace
            .ensure_semantic_model(&embedding_config, query_vector.len())
            .map_err(error_string)?;
        let assets = workspace.semantic_assets().map_err(error_string)?;
        let embeddings = workspace
            .semantic_embeddings(&model_id)
            .map_err(error_string)?;
        let status = workspace
            .semantic_index_status(&embedding_config)
            .map_err(error_string)?;
        (assets, embeddings, status)
    };
    let candidate_limit = limit.unwrap_or(20).max(1).min(100) * 2;
    let mut results =
        fuse_search_results(&query, &assets, &embeddings, &query_vector, candidate_limit);
    let candidate_count = results.len();
    let mut warnings = Vec::new();
    if status.pending_assets > 0 {
        warnings.push(format!(
            "{} 张照片尚未生成语义向量，当前结果只使用已索引照片",
            status.pending_assets
        ));
    }
    let mut reranked = false;
    if let Some(rerank_config) = rerank_config {
        if rerank_config.enabled {
            if let Some(rerank_key) = rerank_key {
                let by_id = assets
                    .iter()
                    .map(|asset| (asset.asset_id.as_str(), asset))
                    .collect::<HashMap<_, _>>();
                let rerank_candidates = results
                    .iter()
                    .filter_map(|result| {
                        let asset = by_id.get(result.asset_id.as_str())?;
                        Some(RerankCandidate {
                            asset_id: result.asset_id.clone(),
                            text: asset.combined_text.clone(),
                            image_jpeg: asset
                                .thumbnail_path
                                .as_deref()
                                .and_then(|path| std::fs::read(path).ok()),
                        })
                    })
                    .collect::<Vec<_>>();
                if !rerank_candidates.is_empty() {
                    match client
                        .rerank(&rerank_config, &rerank_key, &query, &rerank_candidates)
                        .await
                    {
                        Ok(outcome) => {
                            reranked = true;
                            if outcome.mode == "text" {
                                warnings.push(
                                    "Rerank 多模态请求被拒绝，已降级为文本与元数据重排序".into(),
                                );
                            }
                            for result in &mut results {
                                if let Some(score) = outcome.scores.get(&result.asset_id) {
                                    result.score = 0.35 * result.score + 0.65 * score;
                                    result.match_reasons.push(grapes_core::SemanticMatchReason {
                                        kind: "rerank".into(),
                                        label: String::new(),
                                        score: *score,
                                    });
                                }
                            }
                            results.sort_by(|left, right| right.score.total_cmp(&left.score));
                        }
                        Err(error) => {
                            warnings.push(format!("Rerank 失败，已返回向量结果：{error}"))
                        }
                    }
                }
            } else {
                warnings.push("未配置 Rerank API Key，已关闭重排序".into());
            }
        }
    }
    results.truncate(limit.unwrap_or(20).max(1).min(100));
    Ok(SemanticSearchResponse {
        candidate_count,
        reranked,
        results,
        warnings,
    })
}

fn active_workspace(
    state: &State<'_, AppState>,
) -> std::result::Result<Arc<Mutex<Workspace>>, String> {
    state
        .session
        .lock()
        .map_err(|_| "应用状态锁已损坏".to_owned())?
        .as_ref()
        .map(|session| session.workspace.clone())
        .ok_or_else(|| "请先打开一个 Workspace".to_owned())
}

fn read_snapshot(
    workspace: &Arc<Mutex<Workspace>>,
) -> std::result::Result<WorkspaceSnapshot, String> {
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .snapshot()
        .map_err(error_string)
}

fn error_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn thumbnail_response(
    app: &AppHandle,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{header, Response, StatusCode};

    let asset_id = request.uri().path().trim_matches('/');
    let state = app.state::<AppState>();
    let workspace = state
        .session
        .lock()
        .ok()
        .and_then(|session| session.as_ref().map(|session| session.workspace.clone()));
    let path = workspace.and_then(|workspace| {
        workspace
            .lock()
            .ok()
            .and_then(|workspace| workspace.asset_thumbnail_path(asset_id).ok())
    });
    let Some(path) = path else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(b"thumbnail not found".to_vec())
            .unwrap_or_else(|_| Response::new(b"thumbnail not found".to_vec()));
    };
    let bytes = std::fs::read(path).unwrap_or_default();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(bytes)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn photo_response(
    app: &AppHandle,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    use tauri::http::{header, Response, StatusCode};

    let asset_id = request.uri().path().trim_matches('/');
    let state = app.state::<AppState>();
    let workspace = state
        .session
        .lock()
        .ok()
        .and_then(|session| session.as_ref().map(|session| session.workspace.clone()));
    let path = workspace.and_then(|workspace| {
        workspace
            .lock()
            .ok()
            .and_then(|workspace| workspace.asset_path(asset_id).ok())
    });
    let Some(path) = path else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(b"photo not found".to_vec())
            .unwrap_or_else(|_| Response::new(b"photo not found".to_vec()));
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(b"photo not readable".to_vec())
            .unwrap_or_else(|_| Response::new(b"photo not readable".to_vec()));
    };
    let content_type = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "avif" => "image/avif",
        "heic" | "heif" => "image/heic",
        _ => "image/jpeg",
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable",
        )
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(bytes)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("grapes-thumb", |context, request| {
            thumbnail_response(context.app_handle(), request)
        })
        .register_uri_scheme_protocol("grapes-photo", |context, request| {
            photo_response(context.app_handle(), request)
        })
        .setup(|app| {
            app.manage(AppState::default());
            if let (Some(icon), Some(window)) = (
                app.default_window_icon().cloned(),
                app.get_webview_window("main"),
            ) {
                window.set_icon(icon)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_workspace,
            workspace_snapshot,
            refresh_workspace,
            create_folder,
            move_folder,
            rename_folder,
            move_asset,
            rename_asset,
            set_asset_tags,
            graph_catalog,
            graph_view,
            save_graph_position,
            classification_schema,
            set_asset_classification,
            create_custom_dimension,
            create_dimension_value,
            create_smart_graph,
            delete_smart_graph,
            semantic_settings,
            save_semantic_provider,
            test_semantic_provider,
            rebuild_semantic_index,
            semantic_search
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Grapes");
}
