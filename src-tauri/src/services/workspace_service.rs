use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::store;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorkspaceSettings {
    pub saved_git_directories: Vec<SavedWorkspaceLocation>,
    pub saved_p4_directories: Vec<SavedWorkspaceLocation>,
    pub selected_git_directory_id: Option<String>,
    pub selected_p4_directory_id: Option<String>,
    pub github_pat: Option<String>,
    pub gitlab_pat: Option<String>,
    pub gitlab_host_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedWorkspaceLocation {
    pub id: String,
    pub path: String,
    pub label: String,
    pub created_at: i64,
    pub last_used_at: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum WorkspaceLocationProvider {
    Git,
    P4,
}

impl WorkspaceLocationProvider {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "git" => Ok(Self::Git),
            "p4" | "perforce" => Ok(Self::P4),
            other => Err(format!("Unsupported workspace provider: {}", other)),
        }
    }
}

pub fn get_settings(conn: &Connection, workspace_id: &str) -> Result<WorkspaceSettings, String> {
    let workspace = store::get_workspace(conn, workspace_id)?;
    parse_settings(&workspace.settings_json)
}

pub fn save_location(
    conn: &Connection,
    workspace_id: &str,
    provider: WorkspaceLocationProvider,
    path: &str,
) -> Result<WorkspaceSettings, String> {
    let trimmed_path = path.trim();
    if trimmed_path.is_empty() {
        return Err("Workspace path is required".to_string());
    }

    let mut settings = get_settings(conn, workspace_id)?;
    let now = chrono::Utc::now().timestamp();
    let locations = locations_mut(&mut settings, provider);
    let existing = locations
        .iter_mut()
        .find(|location| same_path(&location.path, trimmed_path));

    let selected_id = if let Some(location) = existing {
        location.path = trimmed_path.to_string();
        location.label = display_label(trimmed_path);
        location.last_used_at = now;
        location.id.clone()
    } else {
        let location = SavedWorkspaceLocation {
            id: uuid::Uuid::new_v4().to_string(),
            path: trimmed_path.to_string(),
            label: display_label(trimmed_path),
            created_at: now,
            last_used_at: now,
        };
        let id = location.id.clone();
        locations.push(location);
        id
    };

    locations.sort_by(|left, right| right.last_used_at.cmp(&left.last_used_at));
    set_selected_id(&mut settings, provider, Some(selected_id));
    persist_settings(conn, workspace_id, &settings)?;
    Ok(settings)
}

pub fn select_location(
    conn: &Connection,
    workspace_id: &str,
    provider: WorkspaceLocationProvider,
    location_id: Option<String>,
) -> Result<WorkspaceSettings, String> {
    let mut settings = get_settings(conn, workspace_id)?;
    let maybe_id = location_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if let Some(selected_id) = maybe_id.as_deref() {
        let now = chrono::Utc::now().timestamp();
        let locations = locations_mut(&mut settings, provider);
        let Some(location) = locations.iter_mut().find(|entry| entry.id == selected_id) else {
            return Err("Saved workspace location not found".to_string());
        };
        location.last_used_at = now;
        locations.sort_by(|left, right| right.last_used_at.cmp(&left.last_used_at));
    }

    set_selected_id(&mut settings, provider, maybe_id);
    persist_settings(conn, workspace_id, &settings)?;
    Ok(settings)
}

pub fn remove_location(
    conn: &Connection,
    workspace_id: &str,
    provider: WorkspaceLocationProvider,
    location_id: &str,
) -> Result<WorkspaceSettings, String> {
    let trimmed_id = location_id.trim();
    if trimmed_id.is_empty() {
        return Err("Saved workspace id is required".to_string());
    }

    let mut settings = get_settings(conn, workspace_id)?;
    let locations = locations_mut(&mut settings, provider);
    let before = locations.len();
    locations.retain(|location| location.id != trimmed_id);
    if before == locations.len() {
        return Err("Saved workspace location not found".to_string());
    }

    let selected_id = selected_id(&settings, provider)
        .filter(|current| *current != trimmed_id)
        .map(str::to_string);
    set_selected_id(&mut settings, provider, selected_id);
    persist_settings(conn, workspace_id, &settings)?;
    Ok(settings)
}

pub fn update_scm_settings(
    conn: &Connection,
    workspace_id: &str,
    github_pat: Option<String>,
    gitlab_pat: Option<String>,
    gitlab_host_url: Option<String>,
) -> Result<WorkspaceSettings, String> {
    let mut settings = get_settings(conn, workspace_id)?;
    settings.github_pat = github_pat;
    settings.gitlab_pat = gitlab_pat;
    settings.gitlab_host_url = gitlab_host_url;
    persist_settings(conn, workspace_id, &settings)?;
    Ok(settings)
}

fn parse_settings(settings_json: &str) -> Result<WorkspaceSettings, String> {
    serde_json::from_str(settings_json).map_err(|err| err.to_string())
}

fn persist_settings(
    conn: &Connection,
    workspace_id: &str,
    settings: &WorkspaceSettings,
) -> Result<(), String> {
    let settings_json = serde_json::to_string(settings).map_err(|err| err.to_string())?;
    store::update_workspace_settings(conn, workspace_id, &settings_json)
}

fn locations_mut(
    settings: &mut WorkspaceSettings,
    provider: WorkspaceLocationProvider,
) -> &mut Vec<SavedWorkspaceLocation> {
    match provider {
        WorkspaceLocationProvider::Git => &mut settings.saved_git_directories,
        WorkspaceLocationProvider::P4 => &mut settings.saved_p4_directories,
    }
}

fn selected_id(settings: &WorkspaceSettings, provider: WorkspaceLocationProvider) -> Option<&str> {
    match provider {
        WorkspaceLocationProvider::Git => settings.selected_git_directory_id.as_deref(),
        WorkspaceLocationProvider::P4 => settings.selected_p4_directory_id.as_deref(),
    }
}

fn set_selected_id(
    settings: &mut WorkspaceSettings,
    provider: WorkspaceLocationProvider,
    value: Option<String>,
) {
    match provider {
        WorkspaceLocationProvider::Git => settings.selected_git_directory_id = value,
        WorkspaceLocationProvider::P4 => settings.selected_p4_directory_id = value,
    }
}

fn display_label(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(path)
        .to_string()
}

fn same_path(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}
