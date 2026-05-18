use tauri::State;

use crate::{
    app_state::AppState,
    services::workspace_service::{self, WorkspaceSettings},
    store,
};

#[tauri::command]
pub fn get_current_workspace(state: State<AppState>) -> Result<store::Workspace, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    store::get_workspace(&conn, &workspace_id)
}

#[tauri::command]
pub fn list_workspaces(state: State<AppState>) -> Result<Vec<store::Workspace>, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    store::list_workspaces(&conn)
}

#[tauri::command]
pub fn create_workspace(state: State<AppState>, name: String) -> Result<store::Workspace, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    store::create_workspace(&conn, &name)
}

#[tauri::command]
pub fn open_workspace(state: State<AppState>, id: String) -> Result<(), String> {
    let mut workspace_id = state
        .current_workspace_id
        .lock()
        .map_err(|err| err.to_string())?;
    *workspace_id = id;
    Ok(())
}

#[tauri::command]
pub fn get_current_workspace_settings(state: State<AppState>) -> Result<WorkspaceSettings, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    workspace_service::get_settings(&conn, &workspace_id)
}

#[tauri::command]
pub fn save_current_workspace_location(
    state: State<AppState>,
    provider: String,
    path: String,
) -> Result<WorkspaceSettings, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    let provider = workspace_service::WorkspaceLocationProvider::parse(&provider)?;
    workspace_service::save_location(&conn, &workspace_id, provider, &path)
}

#[tauri::command]
pub fn select_current_workspace_location(
    state: State<AppState>,
    provider: String,
    location_id: Option<String>,
) -> Result<WorkspaceSettings, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    let provider = workspace_service::WorkspaceLocationProvider::parse(&provider)?;
    workspace_service::select_location(&conn, &workspace_id, provider, location_id)
}

#[tauri::command]
pub fn remove_current_workspace_location(
    state: State<AppState>,
    provider: String,
    location_id: String,
) -> Result<WorkspaceSettings, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    let provider = workspace_service::WorkspaceLocationProvider::parse(&provider)?;
    workspace_service::remove_location(&conn, &workspace_id, provider, &location_id)
}
