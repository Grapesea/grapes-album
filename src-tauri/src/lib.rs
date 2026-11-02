use std::sync::{Arc, Mutex};

use grapes_core::{GraphInfo, GraphView, Workspace, WorkspaceSnapshot, WorkspaceWatcher};
use tauri::{AppHandle, Emitter, Manager, State};

struct Session {
    workspace: Arc<Mutex<Workspace>>,
    _watcher: WorkspaceWatcher,
}

#[derive(Default)]
struct AppState {
    session: Mutex<Option<Session>>,
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
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .graph_catalog()
        .map_err(error_string)
}

#[tauri::command]
fn graph_view(
    state: State<'_, AppState>,
    graph_id: Option<String>,
) -> std::result::Result<GraphView, String> {
    let workspace = active_workspace(&state)?;
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .graph_view(graph_id.as_deref())
        .map_err(error_string)
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
    workspace
        .lock()
        .map_err(|_| "工作区锁已损坏".to_owned())?
        .save_graph_position(&graph_id, &node_key, x, y, pinned)
        .map_err(error_string)
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
            save_graph_position
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Grapes");
}
