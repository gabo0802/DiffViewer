use tauri::State;

use crate::{
    app_state::AppState, diff_engine::render::RenderedDiffModel, scm, services::render_service,
    store, workspace_controller, providers::{ScmProvider, ImportTarget, git::GitProvider, p4::P4Provider},
};

#[tauri::command]
pub fn import_patch(state: State<AppState>, path: String) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    workspace_controller::import_patch(&conn, &workspace_id, &path)
}

#[tauri::command]
pub fn compare_two_files(
    state: State<AppState>,
    left_path: String,
    right_path: String,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    workspace_controller::compare_two_files(&conn, &workspace_id, &left_path, &right_path, None, None)
}

#[tauri::command]
pub fn import_git_working_tree(
    state: State<AppState>,
    repo_path: String,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    let target = ImportTarget::GitWorkingTree { repo_path };
    GitProvider.import_target(&conn, &workspace_id, &target)
}

#[tauri::command]
pub fn import_git_commit(
    state: State<AppState>,
    repo_path: String,
    rev: String,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    let target = ImportTarget::GitCommit { repo_path, rev };
    GitProvider.import_target(&conn, &workspace_id, &target)
}

#[tauri::command]
pub fn import_p4_pending(
    state: State<AppState>,
    change: String,
    cwd: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    let target = ImportTarget::P4Pending { change, cwd };
    P4Provider.import_target(&conn, &workspace_id, &target)
}

#[tauri::command]
pub fn import_p4_shelved(
    state: State<AppState>,
    change: String,
    cwd: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    let target = ImportTarget::P4Shelved { change, cwd };
    P4Provider.import_target(&conn, &workspace_id, &target)
}

#[tauri::command]
pub fn import_p4_submitted(
    state: State<AppState>,
    change: String,
    cwd: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    let target = ImportTarget::P4Submitted { change, cwd };
    P4Provider.import_target(&conn, &workspace_id, &target)
}

#[tauri::command]
pub fn list_git_commits(
    repo_path: String,
    limit: Option<usize>,
    branch: Option<String>,
) -> Result<Vec<scm::GitCommitSummary>, String> {
    scm::list_git_commits(&repo_path, limit.unwrap_or(30), branch.as_deref())
}

#[tauri::command]
pub fn list_git_branches(repo_path: String) -> Result<Vec<String>, String> {
    scm::list_git_branches(&repo_path)
}

#[tauri::command]
pub fn get_pull_requests(
    state: State<AppState>,
    repo_path: String,
) -> Result<Vec<scm::pr_api::PullRequestSummary>, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    scm::get_pull_requests(&conn, &workspace_id, &repo_path)
}

#[tauri::command]
pub fn import_git_pull_request(
    state: State<AppState>,
    repo_path: String,
    pr_id: String,
    target_branch: String,
    pr_title: Option<String>,
) -> Result<String, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let workspace_id = state.current_workspace_id()?;
    let target = ImportTarget::GitPullRequest { repo_path, pr_id, target_branch, pr_title };
    GitProvider.import_target(&conn, &workspace_id, &target)
}

#[tauri::command]
pub fn list_p4_pending_changes(
    cwd: Option<String>,
) -> Result<Vec<scm::P4PendingChangeSummary>, String> {
    scm::list_p4_pending_changes(cwd.as_deref())
}

#[tauri::command]
pub fn list_diffsets(
    state: State<AppState>,
    workspace_id: String,
) -> Result<Vec<store::DiffSet>, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    store::list_diffsets(&conn, &workspace_id)
}

#[tauri::command]
pub fn list_filediffs(
    state: State<AppState>,
    diffset_id: String,
) -> Result<Vec<store::FileDiff>, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    store::list_filediffs(&conn, &diffset_id)
}

#[tauri::command]
pub fn refresh_workspace_diffsets(
    state: State<AppState>,
    workspace_id: String,
) -> Result<Vec<store::DiffSet>, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    let diffsets = store::list_diffsets(&conn, &workspace_id)?;
    for diffset in &diffsets {
        scm::refresh_diffset(&conn, &diffset.diffset_id)?;
    }
    store::list_diffsets(&conn, &workspace_id)
}

#[tauri::command]
pub fn refresh_diffset(state: State<AppState>, diffset_id: String) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    scm::refresh_diffset(&conn, &diffset_id)
}

#[tauri::command]
pub fn delete_diffset(state: State<AppState>, diffset_id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    store::delete_diffset(&conn, &diffset_id)
}

#[tauri::command]
pub fn get_rendered_diff(
    state: State<AppState>,
    filediff_id: String,
) -> Result<RenderedDiffModel, String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    render_service::get_rendered_diff(&conn, &filediff_id)
}

#[tauri::command]
pub fn mark_reviewed(
    state: State<AppState>,
    filediff_id: String,
    reviewed: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|err| err.to_string())?;
    store::upsert_review_state(
        &conn,
        &store::ReviewState {
            filediff_id,
            reviewed,
            last_view_mode: "sideBySide".to_string(),
            last_scroll_pos: 0.0,
            last_cursor_json: "{}".to_string(),
            updated_at: chrono::Utc::now().timestamp(),
        },
    )
}
