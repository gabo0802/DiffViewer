use rusqlite::Connection;

use crate::{app_state::AppState, open_request::OpenRequest, workspace_controller};

pub fn handle_open_request(
    app_state: &AppState,
    request: OpenRequest,
) -> Result<String, String> {
    let mut conn = app_state.db.lock().map_err(|e| e.to_string())?;
    let workspace_id = app_state.current_workspace_id()?;
    
    match request {
        OpenRequest::OpenPatch { path } => {
            workspace_controller::import_patch(&conn, &workspace_id, &path)
        }
        OpenRequest::OpenTwoWay {
            left_path,
            right_path,
            title,
            ..
        } => execute_two_way_with_grouping(
            app_state,
            &mut *conn,
            &workspace_id,
            &left_path,
            &right_path,
            title.as_deref(),
        ),
        OpenRequest::OpenFiles { paths } => {
            if paths.len() == 1 && is_patch_path(&paths[0]) {
                workspace_controller::import_patch(&conn, &workspace_id, &paths[0])
            } else if paths.len() == 2 {
                execute_two_way_with_grouping(
                    app_state,
                    &mut *conn,
                    &workspace_id,
                    &paths[0],
                    &paths[1],
                    None,
                )
            } else {
                Err("Unsupported number of files".to_string())
            }
        }
        OpenRequest::OpenMerge { .. } => Err("Merge mode not yet implemented in V1".to_string()),
    }
}

fn execute_two_way_with_grouping(
    _app_state: &AppState,
    conn: &mut rusqlite::Connection,
    workspace_id: &str,
    left_path: &str,
    right_path: &str,
    title: Option<&str>,
) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp();
    
    let existing_id: Option<String> = conn.query_row(
        "SELECT diffset_id FROM diffsets WHERE provider = 'external' AND workspace_id = ?1 AND created_at >= ?2 ORDER BY created_at DESC LIMIT 1",
        rusqlite::params![workspace_id, now - 5],
        |row| row.get(0)
    ).ok();

    let diffset_id = workspace_controller::compare_two_files(
        conn,
        workspace_id,
        left_path,
        right_path,
        title,
        existing_id,
    )?;

    Ok(diffset_id)
}

pub fn handle_startup_request(app_state: &AppState, request: OpenRequest) {
    let debug = crate::debugging::DebugLogger::new("startup");
    let description = describe_request(&request);
    match handle_open_request(app_state, request) {
        Ok(_) => debug.log(format!("startup_open_succeeded {}", description)),
        Err(err) => debug.log(format!("startup_open_failed {} error={}", description, err)),
    }
}

fn is_patch_path(path: &str) -> bool {
    path.ends_with(".diff") || path.ends_with(".patch")
}

fn describe_request(request: &OpenRequest) -> String {
    match request {
        OpenRequest::OpenPatch { path } => format!("patch path={}", path),
        OpenRequest::OpenTwoWay {
            left_path,
            right_path,
            ..
        } => format!("two_way left={} right={}", left_path, right_path),
        OpenRequest::OpenFiles { paths } => format!("files count={}", paths.len()),
        OpenRequest::OpenMerge { .. } => "merge".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store;
    use std::fs;
    use tempfile::tempdir;

    fn setup_test_app_state() -> AppState {
        let conn = store::open_in_memory_db().unwrap();
        let inbox = store::ensure_inbox(&conn).unwrap();
        AppState::new(conn, inbox.workspace_id)
    }

    #[test]
    fn test_execute_two_way_grouping() {
        let app_state = setup_test_app_state();
        let dir = tempdir().unwrap();
        
        let file1 = dir.path().join("a.txt");
        let file2 = dir.path().join("b.txt");
        let file3 = dir.path().join("c.txt");
        let file4 = dir.path().join("d.txt");
        
        fs::write(&file1, "A").unwrap();
        fs::write(&file2, "B").unwrap();
        fs::write(&file3, "C").unwrap();
        fs::write(&file4, "D").unwrap();

        let mut conn = app_state.db.lock().unwrap();
        let workspace_id = app_state.current_workspace_id().unwrap();

        let diffset_id_1 = execute_two_way_with_grouping(
            &app_state,
            &mut conn,
            &workspace_id,
            file1.to_str().unwrap(),
            file2.to_str().unwrap(),
            None,
        ).unwrap();

        let diffset_id_2 = execute_two_way_with_grouping(
            &app_state,
            &mut conn,
            &workspace_id,
            file3.to_str().unwrap(),
            file4.to_str().unwrap(),
            None,
        ).unwrap();

        assert_eq!(diffset_id_1, diffset_id_2, "Files opened quickly should group into the same DiffSet");

        // Verify read-only status
        let diffsets = store::list_diffsets(&conn, &workspace_id).unwrap();
        assert_eq!(diffsets.len(), 1);
        let filediffs = store::list_filediffs(&conn, &diffset_id_1).unwrap();
        assert_eq!(filediffs.len(), 2);

        for fd in filediffs {
            assert_eq!(fd.write_target_json, r#"{"type":"read_only"}"#);
        }

        // To test expiration, we can modify the timestamp in the database
        conn.execute(
            "UPDATE diffsets SET created_at = created_at - 10 WHERE diffset_id = ?1",
            rusqlite::params![diffset_id_1],
        ).unwrap();

        let file5 = dir.path().join("e.txt");
        let file6 = dir.path().join("f.txt");
        fs::write(&file5, "E").unwrap();
        fs::write(&file6, "F").unwrap();

        let diffset_id_3 = execute_two_way_with_grouping(
            &app_state,
            &mut conn,
            &workspace_id,
            file5.to_str().unwrap(),
            file6.to_str().unwrap(),
            None,
        ).unwrap();

        assert_ne!(diffset_id_1, diffset_id_3, "After 5 seconds, a new diffset should be created");
        
        let diffsets_after = store::list_diffsets(&conn, &workspace_id).unwrap();
        assert_eq!(diffsets_after.len(), 2);
    }
}
