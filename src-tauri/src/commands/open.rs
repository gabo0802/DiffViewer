use tauri::State;

use crate::{app_state::AppState, open_request::OpenRequest, services::open_service};

#[tauri::command]
pub fn handle_open_request(state: State<AppState>, request_json: String) -> Result<String, String> {
    let request: OpenRequest = serde_json::from_str(&request_json)
        .map_err(|err| format!("Invalid open request: {}", err))?;
    open_service::handle_open_request(&state, request)
}
