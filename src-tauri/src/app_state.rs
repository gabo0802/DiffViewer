use rusqlite::Connection;
use std::sync::Mutex;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub current_workspace_id: Mutex<String>,
    pub external_diff_group: Mutex<Option<(String, i64)>>,
}

impl AppState {
    pub fn new(conn: Connection, current_workspace_id: String) -> Self {
        Self {
            db: Mutex::new(conn),
            current_workspace_id: Mutex::new(current_workspace_id),
            external_diff_group: Mutex::new(None),
        }
    }

    pub fn current_workspace_id(&self) -> Result<String, String> {
        self.current_workspace_id
            .lock()
            .map(|id| id.clone())
            .map_err(|err| err.to_string())
    }
}
