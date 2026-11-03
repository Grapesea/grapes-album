use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures_util::{
    future::{select, Either},
    stream,
    task::AtomicWaker,
    StreamExt,
};
use grapes_core::{
    semantic::{
        average_embeddings, create_search_plan, fallback_search_plan, fuse_plan_search_relaxed,
        provider_jpeg, rerank_candidate_text, RerankCandidate, SemanticProviderClient,
    },
    DimensionView, GraphInfo, GraphView, SearchPlan, SemanticIndexReport, SemanticIndexStatus,
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
struct AskCancellation {
    cancelled: AtomicBool,
    waker: AtomicWaker,
}

impl AskCancellation {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.waker.wake();
    }

    async fn cancelled(self: Arc<Self>) {
        std::future::poll_fn(move |context| {
            if self.cancelled.load(Ordering::Acquire) {
                std::task::Poll::Ready(())
            } else {
                self.waker.register(context.waker());
                if self.cancelled.load(Ordering::Acquire) {
                    std::task::Poll::Ready(())
                } else {
                    std::task::Poll::Pending
                }
            }
        })
        .await;
    }
}

#[derive(Default)]
struct AppState {
    session: Mutex<Option<Session>>,
    ask_cancellations: Mutex<HashMap<String, Arc<AskCancellation>>>,
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AskGrapesProgress {
    request_id: String,
    stage: String,
    state: String,
    message: String,
    detail: Option<String>,
    elapsed_ms: u64,
}

fn emit_ask_progress(
    app: &AppHandle,
    request_id: &str,
    started: Instant,
    stage: &str,
    state: &str,
    message: impl Into<String>,
    detail: Option<String>,
) {
    let _ = app.emit(
        "ask-grapes-progress",
        AskGrapesProgress {
            request_id: request_id.to_owned(),
            stage: stage.to_owned(),
            state: state.to_owned(),
            message: message.into(),
            detail,
            elapsed_ms: started.elapsed().as_millis() as u64,
        },
    );
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
    let config = if kind == "qa" {
        workspace
            .lock()
            .map_err(|_| "工作区锁已损坏".to_owned())?
            .ask_grapes_provider_config()
            .map_err(error_string)?
    } else {
        workspace
            .lock()
            .map_err(|_| "工作区锁已损坏".to_owned())?
            .semantic_provider_configs()
            .map_err(error_string)?
            .into_iter()
            .find(|config| config.kind == kind)
            .ok_or_else(|| format!("Provider {kind} 不存在"))?
    };
    if !config.enabled {
        return Ok("Provider 已关闭".into());
    }
    let api_key = config
        .api_key
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "请先填写 API Key".to_owned())?;
    let client = SemanticProviderClient::new().map_err(error_string)?;
    if kind == "qa" {
        let content = client
            .chat(
                &config,
                &api_key,
                "You are a capability probe. Reply with one JSON object only.",
                r#"Return {"ok":true}"#,
                true,
            )
            .await
            .map_err(error_string)?;
        let value: serde_json::Value = serde_json::from_str(
            content
                .trim()
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim(),
        )
        .map_err(|error| format!("模型未返回合法 JSON：{error}"))?;
        if value.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err("模型未返回预期 JSON 能力标记".into());
        }
        Ok("连接成功：文本 ✓ / 结构化 JSON ✓".into())
    } else if kind == "embedding" {
        let vector = client
            .embed_text(&config, &api_key, "Grapes semantic search connection test")
            .await
            .map_err(error_string)?;
        let multimodal = config.model.to_ascii_lowercase().contains("vl");
        Ok(format!(
            "连接成功：文本维度 {} / 图片 {} / 共享多模态空间 {}",
            vector.len(),
            if multimodal { "✓" } else { "未声明" },
            if multimodal { "✓" } else { "未声明" }
        ))
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
    let indexed = stream::iter(sources)
        .map(|source| {
            let client = &client;
            let config = &config;
            let api_key = &api_key;
            async move {
                let result = match provider_jpeg(&source.thumbnail_path) {
                    Ok(bytes) => client.embed_image(config, api_key, &bytes).await,
                    Err(error) => Err(error),
                };
                (source, result)
            }
        })
        .buffer_unordered(config.max_concurrency.clamp(1, 8))
        .collect::<Vec<_>>()
        .await;
    for (source, result) in indexed {
        match result {
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
    app: AppHandle,
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
    request_id: String,
) -> std::result::Result<SemanticSearchResponse, String> {
    let started = Instant::now();
    let cancellation = Arc::new(AskCancellation::default());
    state
        .ask_cancellations
        .lock()
        .map_err(|_| "Ask Grapes 请求状态锁已损坏".to_owned())?
        .insert(request_id.clone(), cancellation.clone());
    let search = Box::pin(run_semantic_search(
        &app,
        state.inner(),
        query,
        limit,
        request_id.clone(),
        started,
    ));
    let cancelled = Box::pin(cancellation.cancelled());
    let result = match select(search, cancelled).await {
        Either::Left((result, _)) => result,
        Either::Right(((), _)) => {
            emit_ask_progress(
                &app,
                &request_id,
                started,
                "cancelled",
                "warning",
                "Ask Grapes 请求已由用户停止",
                Some("当前网络请求已取消".into()),
            );
            Err("Ask Grapes 请求已取消".into())
        }
    };
    if let Ok(mut jobs) = state.ask_cancellations.lock() {
        jobs.remove(&request_id);
    }
    result
}

#[tauri::command]
fn cancel_ask_grapes(
    state: State<'_, AppState>,
    request_id: String,
) -> std::result::Result<bool, String> {
    let cancellation = state
        .ask_cancellations
        .lock()
        .map_err(|_| "Ask Grapes 请求状态锁已损坏".to_owned())?
        .get(&request_id)
        .cloned();
    if let Some(cancellation) = cancellation {
        cancellation.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

async fn run_semantic_search(
    app: &AppHandle,
    state: &AppState,
    query: String,
    limit: Option<usize>,
    request_id: String,
    started: Instant,
) -> std::result::Result<SemanticSearchResponse, String> {
    let query = query.trim().to_owned();
    if query.is_empty() {
        return Err("请输入要查找的相片描述".into());
    }
    emit_ask_progress(
        app,
        &request_id,
        started,
        "query",
        "started",
        "已收到查询，正在读取 Provider 配置",
        Some(query.clone()),
    );
    let workspace = active_workspace_from(state)?;
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
    let qa_config = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .ask_grapes_provider_config()
        .map_err(error_string)?;
    let dimensions = workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .classification_schema()
        .map_err(error_string)?;
    let embedding_key = embedding_config.api_key.clone();
    let rerank_key = rerank_config
        .as_ref()
        .and_then(|config| config.api_key.clone());
    let client = SemanticProviderClient::new().map_err(error_string)?;
    let mut warnings = Vec::new();
    let (search_plan, plan_source) = if qa_config.enabled {
        if let Some(qa_key) = qa_config
            .api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            emit_ask_progress(
                app,
                &request_id,
                started,
                "planning",
                "waiting",
                "已发送 Ask Grapes 查询规划请求，等待模型响应",
                Some(format!(
                    "{} · {} · timeout {}ms · retries {}",
                    qa_config.base_url,
                    qa_config.model,
                    qa_config.request_timeout_ms,
                    qa_config.max_retries
                )),
            );
            match create_search_plan(&client, &qa_config, qa_key, &query, &dimensions).await {
                Ok(plan) => {
                    emit_ask_progress(
                        app,
                        &request_id,
                        started,
                        "planning",
                        "success",
                        "Search Plan 已生成并通过校验",
                        Some(format!(
                            "hard {} · soft {} · semantic {}",
                            plan.hard_filters.len(),
                            plan.soft_filters.len(),
                            plan.semantic_queries.len()
                        )),
                    );
                    (plan, "qa".to_owned())
                }
                Err(error) => {
                    emit_ask_progress(
                        app,
                        &request_id,
                        started,
                        "planning",
                        "warning",
                        "查询规划失败，改用原始查询继续检索",
                        Some(error.to_string()),
                    );
                    warnings.push(format!("Ask Grapes 查询规划失败，已使用原始查询：{error}"));
                    (fallback_search_plan(&query), "fallback".to_owned())
                }
            }
        } else {
            emit_ask_progress(
                app,
                &request_id,
                started,
                "planning",
                "warning",
                "Ask Grapes 未配置 API Key，跳过模型规划",
                None,
            );
            warnings.push("Ask Grapes 未配置 API Key，已使用原始查询".into());
            (fallback_search_plan(&query), "fallback".to_owned())
        }
    } else {
        emit_ask_progress(
            app,
            &request_id,
            started,
            "planning",
            "warning",
            "Ask Grapes QA 已关闭，使用原始查询",
            None,
        );
        warnings.push("Ask Grapes QA 已关闭，已使用原始查询".into());
        (fallback_search_plan(&query), "fallback".to_owned())
    };
    let mut query_vectors = Vec::new();
    if embedding_config.enabled {
        if let Some(embedding_key) = embedding_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            emit_ask_progress(
                app,
                &request_id,
                started,
                "embedding",
                "waiting",
                "正在生成文本查询向量",
                Some(format!(
                    "{} · {} · {} queries",
                    embedding_config.base_url,
                    embedding_config.model,
                    search_plan.semantic_queries.iter().take(4).count()
                )),
            );
            for semantic_query in search_plan.semantic_queries.iter().take(4) {
                match client
                    .embed_text(&embedding_config, embedding_key, semantic_query)
                    .await
                {
                    Ok(vector) => query_vectors.push(vector),
                    Err(error) => {
                        warnings.push(format!("语义查询“{semantic_query}”生成向量失败：{error}"))
                    }
                }
            }
            emit_ask_progress(
                app,
                &request_id,
                started,
                "embedding",
                if query_vectors.is_empty() {
                    "warning"
                } else {
                    "success"
                },
                if query_vectors.is_empty() {
                    "文本向量生成失败，将使用分类与 EXIF 检索"
                } else {
                    "文本查询向量已生成"
                },
                Some(format!("vectors {}", query_vectors.len())),
            );
        } else {
            warnings.push("Embedding 未配置 API Key，本次仅使用分类与 EXIF 检索".into());
        }
    } else {
        warnings.push("Embedding 已关闭，本次仅使用分类与 EXIF 检索".into());
    }
    let query_vector = average_embeddings(&query_vectors);
    let embedding_used = query_vector.is_some();
    emit_ask_progress(
        app,
        &request_id,
        started,
        "retrieval",
        "waiting",
        "正在融合分类、EXIF 与语义向量",
        Some(format!(
            "embedding {}",
            if embedding_used {
                "enabled"
            } else {
                "unavailable"
            }
        )),
    );

    let (assets, embeddings, status) = {
        let workspace = workspace.lock().map_err(|_| "工作区锁已损坏".to_owned())?;
        let assets = workspace.semantic_assets().map_err(error_string)?;
        let embeddings = if let Some(query_vector) = query_vector.as_deref() {
            let model_id = workspace
                .ensure_semantic_model(&embedding_config, query_vector.len())
                .map_err(error_string)?;
            workspace
                .semantic_embeddings(&model_id)
                .map_err(error_string)?
        } else {
            HashMap::new()
        };
        let status = workspace
            .semantic_index_status(&embedding_config)
            .map_err(error_string)?;
        (assets, embeddings, status)
    };
    let candidate_limit = limit.unwrap_or(20).max(1).min(100) * 2;
    let (mut results, relaxed) = fuse_plan_search_relaxed(
        &search_plan,
        &assets,
        &embeddings,
        query_vector.as_deref(),
        candidate_limit,
    );
    if let Some(relaxed) = relaxed {
        let message = match relaxed {
            "exclusionsIgnored" => "严格排除条件没有产生结果，已忽略排除项",
            "hardFiltersRelaxed" => "严格分类条件没有产生结果，已将必须条件降级为优先条件",
            _ => "所有严格过滤条件没有产生结果，已仅使用语义与元数据召回",
        };
        warnings.push(message.into());
        emit_ask_progress(
            app,
            &request_id,
            started,
            "retrieval",
            "warning",
            message,
            None,
        );
    }
    let candidate_count = results.len();
    emit_ask_progress(
        app,
        &request_id,
        started,
        "retrieval",
        "success",
        format!("候选相片检索完成：{candidate_count} 张"),
        None,
    );
    if embedding_used && status.pending_assets > 0 {
        warnings.push(format!(
            "{} 张相片尚未生成语义向量，当前结果只使用已索引相片",
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
                        let candidate = RerankCandidate {
                            asset_id: result.asset_id.clone(),
                            text: asset.combined_text.clone(),
                            image_jpeg: asset
                                .thumbnail_path
                                .as_deref()
                                .and_then(|path| provider_jpeg(path).ok()),
                        };
                        Some(RerankCandidate {
                            text: rerank_candidate_text(&search_plan, &candidate),
                            ..candidate
                        })
                    })
                    .collect::<Vec<_>>();
                if !rerank_candidates.is_empty() {
                    emit_ask_progress(
                        app,
                        &request_id,
                        started,
                        "rerank",
                        "waiting",
                        "正在重排序候选相片",
                        Some(format!(
                            "{} · {} · {} candidates",
                            rerank_config.base_url,
                            rerank_config.model,
                            rerank_candidates.len()
                        )),
                    );
                    match client
                        .rerank(
                            &rerank_config,
                            &rerank_key,
                            search_plan.rerank_instruction.as_deref().unwrap_or(&query),
                            &rerank_candidates,
                        )
                        .await
                    {
                        Ok(outcome) => {
                            reranked = true;
                            emit_ask_progress(
                                app,
                                &request_id,
                                started,
                                "rerank",
                                "success",
                                "候选相片重排序完成",
                                Some(format!("mode {}", outcome.mode)),
                            );
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
                            emit_ask_progress(
                                app,
                                &request_id,
                                started,
                                "rerank",
                                "warning",
                                "Rerank 失败，已返回融合检索结果",
                                Some(error.to_string()),
                            );
                            warnings.push(format!("Rerank 失败，已返回向量结果：{error}"))
                        }
                    }
                }
            } else {
                emit_ask_progress(
                    app,
                    &request_id,
                    started,
                    "rerank",
                    "warning",
                    "未配置 Rerank API Key，跳过重排序",
                    None,
                );
                warnings.push("未配置 Rerank API Key，已关闭重排序".into());
            }
        }
    }
    results.truncate(limit.unwrap_or(20).max(1).min(100));
    emit_ask_progress(
        app,
        &request_id,
        started,
        "answer",
        "success",
        "检索结果已生成，正在生成本地摘要",
        None,
    );
    let answer = Some(local_search_answer(&search_plan, &results));
    emit_ask_progress(
        app,
        &request_id,
        started,
        "complete",
        "success",
        "Ask Grapes 检索完成",
        Some(format!(
            "{} results · {:.1}s",
            results.len(),
            started.elapsed().as_secs_f64()
        )),
    );
    Ok(SemanticSearchResponse {
        query,
        answer,
        search_plan: Some(search_plan),
        plan_source,
        embedding_used,
        candidate_count,
        reranked,
        results,
        warnings,
    })
}

fn local_search_answer(plan: &SearchPlan, results: &[grapes_core::SemanticSearchResult]) -> String {
    let filters = plan
        .hard_filters
        .iter()
        .flat_map(|filter| {
            filter
                .value_names
                .iter()
                .map(move |value| format!("{} · {value}", filter.dimension_name))
        })
        .collect::<Vec<_>>();
    let names = results
        .iter()
        .take(3)
        .map(|result| result.filename.as_str())
        .collect::<Vec<_>>()
        .join("、");
    if results.is_empty() {
        "没有找到符合当前条件的相片。".into()
    } else if filters.is_empty() {
        format!("找到 {} 张相关相片：{names}", results.len())
    } else {
        format!(
            "找到 {} 张相关相片：{names}。主要条件：{}",
            results.len(),
            filters.join("；")
        )
    }
}

fn active_workspace(
    state: &State<'_, AppState>,
) -> std::result::Result<Arc<Mutex<Workspace>>, String> {
    active_workspace_from(state.inner())
}

fn active_workspace_from(state: &AppState) -> std::result::Result<Arc<Mutex<Workspace>>, String> {
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
            semantic_search,
            cancel_ask_grapes
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Grapes");
}
