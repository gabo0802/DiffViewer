#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tauri::command]
fn get_current_workspace() -> Result<String, String> {
  // TODO: hook into SQLite store
  Ok("inbox".to_string())
}

fn main() {
  tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![get_current_workspace])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
