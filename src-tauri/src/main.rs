#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod debugging;
mod diff_engine;
mod io;
mod open_request;
mod scm;
mod store;
mod workspace_controller;

use rusqlite::Connection;
use std::sync::Mutex;
use tauri::State;

use crate::debugging::DebugLogger;

struct AppState {
    db: Mutex<Connection>,
    current_workspace_id: Mutex<String>,
}

// ΓöÇΓöÇ Workspace commands ΓöÇΓöÇ
// test

#[tauri::command]
fn get_current_workspace(state: State<AppState>) -> Result<store::Workspace, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT workspace_id, name, created_at, last_opened_at, settings_json FROM workspaces WHERE workspace_id = ?1",
        rusqlite::params![*ws_id],
        |row| Ok(store::Workspace {
            workspace_id: row.get(0)?,
            name: row.get(1)?,
            created_at: row.get(2)?,
            last_opened_at: row.get(3)?,
            settings_json: row.get(4)?,
        }),
    ).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_workspaces(state: State<AppState>) -> Result<Vec<store::Workspace>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    store::list_workspaces(&conn)
}

#[tauri::command]
fn create_workspace(state: State<AppState>, name: String) -> Result<store::Workspace, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    store::create_workspace(&conn, &name)
}

#[tauri::command]
fn open_workspace(state: State<AppState>, id: String) -> Result<(), String> {
    let mut ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;
    *ws_id = id;
    Ok(())
}

// ΓöÇΓöÇ Diff creation commands ΓöÇΓöÇ

#[tauri::command]
fn import_patch(state: State<AppState>, path: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;
    workspace_controller::import_patch(&conn, &ws_id, &path)
}

#[tauri::command]
fn compare_two_files(
    state: State<AppState>,
    left_path: String,
    right_path: String,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;
    workspace_controller::compare_two_files(&conn, &ws_id, &left_path, &right_path, None)
}

#[tauri::command]
fn import_git_working_tree(state: State<AppState>, repo_path: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;
    scm::import_git_working_tree(&conn, &ws_id, &repo_path)
}

#[tauri::command]
fn import_git_commit(
    state: State<AppState>,
    repo_path: String,
    rev: String,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;
    scm::import_git_commit(&conn, &ws_id, &repo_path, &rev)
}

#[tauri::command]
fn import_p4_pending(
    state: State<AppState>,
    change: String,
    cwd: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;
    scm::import_p4_pending(&conn, &ws_id, &change, cwd.as_deref())
}

#[tauri::command]
fn import_p4_shelved(
    state: State<AppState>,
    change: String,
    cwd: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;
    scm::import_p4_shelved(&conn, &ws_id, &change, cwd.as_deref())
}

#[tauri::command]
fn import_p4_submitted(
    state: State<AppState>,
    change: String,
    cwd: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;
    scm::import_p4_submitted(&conn, &ws_id, &change, cwd.as_deref())
}

// ΓöÇΓöÇ Diff access commands ΓöÇΓöÇ

#[tauri::command]
fn list_diffsets(
    state: State<AppState>,
    workspace_id: String,
) -> Result<Vec<store::DiffSet>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    store::list_diffsets(&conn, &workspace_id)
}

#[tauri::command]
fn list_filediffs(
    state: State<AppState>,
    diffset_id: String,
) -> Result<Vec<store::FileDiff>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    store::list_filediffs(&conn, &diffset_id)
}

#[tauri::command]
fn refresh_workspace_diffsets(
    state: State<AppState>,
    workspace_id: String,
) -> Result<Vec<store::DiffSet>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let diffsets = store::list_diffsets(&conn, &workspace_id)?;
    for diffset in &diffsets {
        let _ = scm::refresh_diffset(&conn, &diffset.diffset_id)?;
    }
    store::list_diffsets(&conn, &workspace_id)
}

#[tauri::command]
fn delete_diffset(state: State<AppState>, diffset_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    store::delete_diffset(&conn, &diffset_id)
}

#[tauri::command]
fn get_rendered_diff(
    state: State<AppState>,
    filediff_id: String,
) -> Result<diff_engine::render::RenderedDiffModel, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let fd = store::get_filediff(&conn, &filediff_id)?;

    let left_text = resolve_content_source(&fd.content_left_json)?;
    let right_text = resolve_content_source(&fd.content_right_json)?;

    let hunks: Vec<diff_engine::unified_parser::Hunk> =
        serde_json::from_str(&fd.hunks_json).unwrap_or_default();

    let rows = diff_engine::twoway::build_alignment_rows(&left_text, &right_text, &hunks);
    Ok(diff_engine::render::build_rendered_model(
        &filediff_id,
        &rows,
    ))
}

#[tauri::command]
fn mark_reviewed(
    state: State<AppState>,
    filediff_id: String,
    reviewed: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    store::upsert_review_state(
        &conn,
        &store::ReviewState {
            filediff_id,
            reviewed,
            last_view_mode: "sideBySide".to_string(),
            last_scroll_pos: 0.0,
            last_cursor_json: "{}".to_string(),
            updated_at: now,
        },
    )
}

// ΓöÇΓöÇ Merge panel commands ΓöÇΓöÇ

#[tauri::command]
fn init_mergebuffer(
    state: State<AppState>,
    filediff_id: String,
) -> Result<store::MergeBuffer, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let fd = store::get_filediff(&conn, &filediff_id)?;
    let right_text = resolve_content_source(&fd.content_right_json)?;
    let merged = diff_engine::merge::init_merge_buffer(&right_text);
    let debug = DebugLogger::new("merge");
    debug.log_diff_line_counts(
        &fd.display_path,
        &fd.content_left_json,
        &fd.content_right_json,
    );
    debug.log_merge_line_count(&fd.display_path, &merged, "init");
    let now = chrono::Utc::now().timestamp();
    let mb = store::MergeBuffer {
        filediff_id: filediff_id.clone(),
        merged_content_json: serde_json::json!({ "type": "virtual", "text": merged }).to_string(),
        dirty: false,
        updated_at: now,
    };
    store::upsert_merge_buffer(&conn, &mb)?;
    Ok(mb)
}

#[tauri::command]
fn apply_hunk_to_mergebuffer(
    state: State<AppState>,
    filediff_id: String,
    hunk_id: String,
    source: String,
) -> Result<store::MergeBuffer, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let fd = store::get_filediff(&conn, &filediff_id)?;
    let mb = store::get_merge_buffer(&conn, &filediff_id)?;

    let merged_text = resolve_content_source(&mb.merged_content_json)?;
    let hunks: Vec<diff_engine::unified_parser::Hunk> =
        serde_json::from_str(&fd.hunks_json).unwrap_or_default();

    let hunk = hunks
        .iter()
        .find(|h| h.id == hunk_id)
        .ok_or_else(|| format!("Hunk {} not found", hunk_id))?;

    let new_merged = diff_engine::merge::apply_hunk_to_buffer(&merged_text, hunk, &source)?;
    DebugLogger::new("merge").log_merge_line_count(&fd.display_path, &new_merged, "apply_hunk");
    let now = chrono::Utc::now().timestamp();
    let updated = store::MergeBuffer {
        filediff_id: filediff_id.clone(),
        merged_content_json: serde_json::json!({ "type": "virtual", "text": new_merged })
            .to_string(),
        dirty: true,
        updated_at: now,
    };
    store::upsert_merge_buffer(&conn, &updated)?;
    Ok(updated)
}

#[tauri::command]
fn set_mergebuffer_text(
    state: State<AppState>,
    filediff_id: String,
    text: String,
) -> Result<store::MergeBuffer, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let fd = store::get_filediff(&conn, &filediff_id)?;
    DebugLogger::new("merge").log_merge_line_count(&fd.display_path, &text, "set_text");
    let now = chrono::Utc::now().timestamp();
    let mb = store::MergeBuffer {
        filediff_id: filediff_id.clone(),
        merged_content_json: serde_json::json!({ "type": "virtual", "text": text }).to_string(),
        dirty: true,
        updated_at: now,
    };
    store::upsert_merge_buffer(&conn, &mb)?;
    Ok(mb)
}

#[tauri::command]
fn save_mergebuffer(state: State<AppState>, filediff_id: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let fd = store::get_filediff(&conn, &filediff_id)?;
    let mb = store::get_merge_buffer(&conn, &filediff_id)?;

    let wt: serde_json::Value =
        serde_json::from_str(&fd.write_target_json).map_err(|e| e.to_string())?;

    match wt.get("type").and_then(|t| t.as_str()) {
        Some("path") => {
            let target_path = wt
                .get("path")
                .and_then(|p| p.as_str())
                .ok_or("Missing path in write_target")?;
            let merged_text = resolve_content_source(&mb.merged_content_json)?;
            let debug = DebugLogger::new("merge");
            debug.log_merge_line_count(&fd.display_path, &merged_text, "save");
            debug.log(format!("save_target path={}", target_path));

            // Preserve EOL style
            if let Ok(existing) = io::read_file_text(std::path::Path::new(target_path)) {
                let eol = io::detect_eol(&existing);
                let normalized = merged_text.replace("\r\n", "\n").replace('\n', eol);
                io::atomic_write(std::path::Path::new(target_path), normalized.as_bytes())?;
            } else {
                io::atomic_write(std::path::Path::new(target_path), merged_text.as_bytes())?;
            }

            // Mark as not dirty
            let now = chrono::Utc::now().timestamp();
            store::upsert_merge_buffer(
                &conn,
                &store::MergeBuffer {
                    filediff_id,
                    merged_content_json: mb.merged_content_json,
                    dirty: false,
                    updated_at: now,
                },
            )?;

            Ok("saved".to_string())
        }
        Some("save_as_required") => {
            Err("Save As required ΓÇö use save_mergebuffer_as with a target path".to_string())
        }
        _ => Err("Unknown write target type".to_string()),
    }
}

#[tauri::command]
fn save_mergebuffer_as(
    state: State<AppState>,
    filediff_id: String,
    path: String,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let fd = store::get_filediff(&conn, &filediff_id)?;
    let mb = store::get_merge_buffer(&conn, &filediff_id)?;
    let merged_text = resolve_content_source(&mb.merged_content_json)?;
    let resolved_path = resolve_save_as_target(&fd.write_target_json, &path)?;
    let debug = DebugLogger::new("merge");
    debug.log_merge_line_count(&fd.display_path, &merged_text, "save_as");
    debug.log(format!("save_as_target path={}", resolved_path));
    io::atomic_write(std::path::Path::new(&resolved_path), merged_text.as_bytes())?;

    // Reattach write target to the chosen path
    let now = chrono::Utc::now().timestamp();
    conn.execute(
        "UPDATE filediffs SET write_target_json = ?1 WHERE filediff_id = ?2",
        rusqlite::params![
            serde_json::json!({ "type": "path", "path": resolved_path }).to_string(),
            filediff_id
        ],
    )
    .map_err(|e| e.to_string())?;

    store::upsert_merge_buffer(
        &conn,
        &store::MergeBuffer {
            filediff_id,
            merged_content_json: mb.merged_content_json,
            dirty: false,
            updated_at: now,
        },
    )?;

    Ok("saved".to_string())
}

// ΓöÇΓöÇ Handle generic open request (from argv or IPC) ΓöÇΓöÇ

#[tauri::command]
fn handle_open_request(state: State<AppState>, request_json: String) -> Result<String, String> {
    let req: open_request::OpenRequest =
        serde_json::from_str(&request_json).map_err(|e| format!("Invalid open request: {}", e))?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let ws_id = state
        .current_workspace_id
        .lock()
        .map_err(|e| e.to_string())?;

    match req {
        open_request::OpenRequest::OpenPatch { path } => {
            workspace_controller::import_patch(&conn, &ws_id, &path)
        }
        open_request::OpenRequest::OpenTwoWay {
            left_path,
            right_path,
            title,
            ..
        } => workspace_controller::compare_two_files(
            &conn,
            &ws_id,
            &left_path,
            &right_path,
            title.as_deref(),
        ),
        open_request::OpenRequest::OpenFiles { paths } => {
            if paths.len() == 1 && (paths[0].ends_with(".diff") || paths[0].ends_with(".patch")) {
                workspace_controller::import_patch(&conn, &ws_id, &paths[0])
            } else if paths.len() == 2 {
                workspace_controller::compare_two_files(&conn, &ws_id, &paths[0], &paths[1], None)
            } else {
                Err("Unsupported number of files".to_string())
            }
        }
        open_request::OpenRequest::OpenMerge { .. } => {
            Err("Merge mode not yet implemented in V1".to_string())
        }
    }
}

// ΓöÇΓöÇ Helpers ΓöÇΓöÇ

fn resolve_content_source(json: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    match v.get("type").and_then(|t| t.as_str()) {
        Some("virtual") => Ok(v
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string()),
        Some("path") => {
            let path = v
                .get("path")
                .and_then(|p| p.as_str())
                .ok_or("Missing path")?;
            io::read_file_text(std::path::Path::new(path))
        }
        Some("snapshot") => {
            let snap_id = v
                .get("snapshot_id")
                .and_then(|s| s.as_str())
                .ok_or("Missing snapshot_id")?;
            let cache_path = io::snapshot_dir().join(snap_id);
            io::read_file_text(&cache_path)
        }
        _ => Err(format!("Unknown content source type in: {}", json)),
    }
}

fn resolve_save_as_target(write_target_json: &str, requested_path: &str) -> Result<String, String> {
    let requested = std::path::PathBuf::from(requested_path);
    if requested.is_absolute() {
        return Ok(requested.to_string_lossy().into_owned());
    }

    let write_target: serde_json::Value =
        serde_json::from_str(write_target_json).map_err(|e| e.to_string())?;
    if let Some(existing_path) = write_target.get("path").and_then(|value| value.as_str()) {
        let existing = std::path::Path::new(existing_path);
        if let Some(parent) = existing.parent() {
            return Ok(parent.join(&requested).to_string_lossy().into_owned());
        }
    }

    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    Ok(cwd.join(requested).to_string_lossy().into_owned())
}

fn main() {
    let conn = store::open_db().expect("Failed to open database");
    let inbox = store::ensure_inbox(&conn).expect("Failed to ensure Inbox workspace");
    let current_ws_id = inbox.workspace_id.clone();

    // Handle argv-based open requests before the UI starts.
    let args: Vec<String> = std::env::args().collect();
    debugging::configure_from_args(&args);
    let debug = DebugLogger::new("startup");
    if let Some(req) = open_request::parse_argv(&args) {
        match req {
            open_request::OpenRequest::OpenPatch { path } => {
                if let Err(e) = workspace_controller::import_patch(&conn, &current_ws_id, &path) {
                    debug.log(format!(
                        "failed_to_import_patch_from_argv path={} error={}",
                        path, e
                    ));
                } else {
                    debug.log(format!("imported_patch_from_argv path={}", path));
                }
            }
            open_request::OpenRequest::OpenTwoWay {
                left_path,
                right_path,
                title,
                ..
            } => {
                if let Err(e) = workspace_controller::compare_two_files(
                    &conn,
                    &current_ws_id,
                    &left_path,
                    &right_path,
                    title.as_deref(),
                ) {
                    debug.log(format!(
                        "failed_to_open_two_way_diff_from_argv left={} right={} error={}",
                        left_path, right_path, e
                    ));
                } else {
                    debug.log(format!(
                        "opened_two_way_diff_from_argv left={} right={}",
                        left_path, right_path
                    ));
                }
            }
            open_request::OpenRequest::OpenFiles { paths } => {
                if paths.len() == 1 && (paths[0].ends_with(".diff") || paths[0].ends_with(".patch"))
                {
                    if let Err(e) =
                        workspace_controller::import_patch(&conn, &current_ws_id, &paths[0])
                    {
                        debug.log(format!(
                            "failed_to_import_openfiles_patch_from_argv path={} error={}",
                            paths[0], e
                        ));
                    } else {
                        debug.log(format!(
                            "imported_openfiles_patch_from_argv path={}",
                            paths[0]
                        ));
                    }
                } else if paths.len() == 2 {
                    if let Err(e) = workspace_controller::compare_two_files(
                        &conn,
                        &current_ws_id,
                        &paths[0],
                        &paths[1],
                        None,
                    ) {
                        debug.log(format!(
                            "failed_to_open_openfiles_two_way_diff_from_argv left={} right={} error={}",
                            paths[0], paths[1], e
                        ));
                    } else {
                        debug.log(format!(
                            "opened_openfiles_two_way_diff_from_argv left={} right={}",
                            paths[0], paths[1]
                        ));
                    }
                }
            }
            open_request::OpenRequest::OpenMerge { .. } => {
                // Merge argv flow is deferred in V1.
            }
        }
    }

    tauri::Builder::default()
        .manage(AppState {
            db: Mutex::new(conn),
            current_workspace_id: Mutex::new(current_ws_id),
        })
        .invoke_handler(tauri::generate_handler![
            get_current_workspace,
            list_workspaces,
            create_workspace,
            open_workspace,
            import_patch,
            compare_two_files,
            import_git_working_tree,
            import_git_commit,
            import_p4_pending,
            import_p4_shelved,
            import_p4_submitted,
            list_diffsets,
            list_filediffs,
            refresh_workspace_diffsets,
            delete_diffset,
            get_rendered_diff,
            mark_reviewed,
            init_mergebuffer,
            apply_hunk_to_mergebuffer,
            set_mergebuffer_text,
            save_mergebuffer,
            save_mergebuffer_as,
            handle_open_request,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
