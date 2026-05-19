use crate::services::system_service;

#[tauri::command]
pub fn browse_for_directory(initial_path: Option<String>) -> Result<Option<String>, String> {
    system_service::browse_for_directory(initial_path.as_deref())
}
