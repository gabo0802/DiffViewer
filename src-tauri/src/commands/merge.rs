use tauri::State;

use crate::{app_state::AppState, services::merge_service, store};

#[tauri::command]
pub fn init_mergebuffer(
    state: State<AppState>,
    filediff_id: String,
) -> Result<store::MergeBuffer, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    merge_service::init_mergebuffer(&conn, &filediff_id)
}

#[tauri::command]
pub fn apply_hunk_to_mergebuffer(
    state: State<AppState>,
    filediff_id: String,
    hunk_id: String,
    source: String,
) -> Result<store::MergeBuffer, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    merge_service::apply_hunk_to_mergebuffer(&conn, &filediff_id, &hunk_id, &source)
}

#[tauri::command]
pub fn set_mergebuffer_text(
    state: State<AppState>,
    filediff_id: String,
    text: String,
) -> Result<store::MergeBuffer, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    merge_service::set_mergebuffer_text(&conn, &filediff_id, text)
}

#[tauri::command]
pub fn save_mergebuffer(state: State<AppState>, filediff_id: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    merge_service::save_mergebuffer(&conn, &filediff_id)
}

#[tauri::command]
pub fn save_mergebuffer_as(
    state: State<AppState>,
    filediff_id: String,
    path: String,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    merge_service::save_mergebuffer_as(&conn, &filediff_id, &path)
}
