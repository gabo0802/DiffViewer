use tauri::State;

use crate::{app_state::AppState, store};

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
