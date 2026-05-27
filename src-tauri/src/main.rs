#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod commands;
mod content_source;
mod debugging;
mod diff_engine;
mod io;
mod open_request;
mod providers;
mod scm;
mod services;
mod store;
mod workspace_controller;

use app_state::AppState;
use tauri::{Emitter, Manager, UserAttentionType};

fn focus_and_highlight_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.request_user_attention(Some(UserAttentionType::Informational));
        let _ = window.set_focus();
    }
}

fn main() {
    let conn = store::open_db().expect("Failed to open database");
    let inbox = store::ensure_inbox(&conn).expect("Failed to ensure Inbox workspace");
    let current_workspace_id = inbox.workspace_id.clone();

    let app_state = AppState::new(conn, current_workspace_id);

    let args: Vec<String> = std::env::args().collect();
    debugging::configure_from_args(&args);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(request) = open_request::parse_argv(&argv) {
                let state = app.state::<AppState>();
                if let Err(err) = services::open_service::handle_open_request(&state, request) {
                    eprintln!("Failed to handle single-instance open request: {}", err);
                } else {
                    focus_and_highlight_main_window(app);
                    let _ = app.emit("refresh-workspace", ());
                }
            }
        }))
        .setup(move |app| {
            if let Some(request) = open_request::parse_argv(&args) {
                let state = app.state::<AppState>();
                services::open_service::handle_startup_request(&state, request);
            }
            Ok(())
        })
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::workspace::get_current_workspace,
            commands::workspace::get_current_workspace_settings,
            commands::workspace::list_workspaces,
            commands::workspace::create_workspace,
            commands::workspace::open_workspace,
            commands::workspace::save_current_workspace_location,
            commands::workspace::select_current_workspace_location,
            commands::workspace::remove_current_workspace_location,
            commands::workspace::update_scm_settings,
            commands::system::browse_for_directory,
            commands::diff::import_patch,
            commands::diff::compare_two_files,
            commands::diff::import_git_working_tree,
            commands::diff::import_git_commit,
            commands::diff::import_p4_pending,
            commands::diff::import_p4_shelved,
            commands::diff::import_p4_submitted,
            commands::diff::list_git_commits,
            commands::diff::list_git_branches,
            commands::diff::get_pull_requests,
            commands::diff::import_git_pull_request,
            commands::diff::list_p4_pending_changes,
            commands::diff::list_diffsets,
            commands::diff::list_filediffs,
            commands::diff::refresh_workspace_diffsets,
            commands::diff::refresh_diffset,
            commands::diff::delete_diffset,
            commands::diff::get_rendered_diff,
            commands::diff::mark_reviewed,
            commands::merge::init_mergebuffer,
            commands::merge::apply_hunk_to_mergebuffer,
            commands::merge::set_mergebuffer_text,
            commands::merge::save_mergebuffer,
            commands::merge::save_mergebuffer_as,
            commands::open::handle_open_request,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
