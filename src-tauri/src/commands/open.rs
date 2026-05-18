use tauri::State;

use crate::{app_state::AppState, open_request::OpenRequest, services::open_service};

#[tauri::command]
pub fn handle_open_request(state: State<AppState>, request_json: String) -> Result<String, String> {
    let request: OpenRequest = serde_json::from_str(&request_json)
        .map_err(|err| format!("Invalid open request: {}", err))?;
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    open_service::handle_open_request(&conn, &workspace_id, request)
}
