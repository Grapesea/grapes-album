use std::sync::{Arc, Mutex};

use grapes_core::{Workspace, WorkspaceSnapshot, WorkspaceWatcher};
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(AppState::default());
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
            set_asset_tags
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Grapes");
}
