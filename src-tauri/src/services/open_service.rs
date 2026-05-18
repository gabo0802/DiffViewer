use rusqlite::Connection;

use crate::{open_request::OpenRequest, workspace_controller};

pub fn handle_open_request(
    conn: &Connection,
    workspace_id: &str,
    request: OpenRequest,
) -> Result<String, String> {
    match request {
        OpenRequest::OpenPatch { path } => {
            workspace_controller::import_patch(conn, workspace_id, &path)
        }
        OpenRequest::OpenTwoWay {
            left_path,
            right_path,
            title,
            ..
        } => workspace_controller::compare_two_files(
            conn,
            workspace_id,
            &left_path,
            &right_path,
            title.as_deref(),
        ),
        OpenRequest::OpenFiles { paths } => {
            if paths.len() == 1 && is_patch_path(&paths[0]) {
                workspace_controller::import_patch(conn, workspace_id, &paths[0])
            } else if paths.len() == 2 {
                workspace_controller::compare_two_files(
                    conn,
                    workspace_id,
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

pub fn handle_startup_request(conn: &Connection, workspace_id: &str, request: OpenRequest) {
    let debug = crate::debugging::DebugLogger::new("startup");
    let description = describe_request(&request);
    match handle_open_request(conn, workspace_id, request) {
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
