
pub use crate::providers::git::*;
pub use crate::providers::p4::*;
use crate::content_source::{ContentSource, WriteTarget};
use crate::debugging::DebugLogger;
use crate::diff_engine::unified_parser::{self, PatchFileDiff};
use crate::store::{self, DiffSet, FileDiff};
use crate::providers::{ScmProvider, ImportTarget, git::GitProvider, p4::P4Provider};

pub mod p4_config;
pub mod process;
pub mod pr_api;

use p4_config::P4Config;

pub fn refresh_diffset(conn: &rusqlite::Connection, diffset_id: &str) -> Result<bool, String> {
    let diffset = store::get_diffset(conn, diffset_id)?;
    let meta: serde_json::Value =
        serde_json::from_str(&diffset.source_meta_json).map_err(|e| e.to_string())?;

    match diffset.kind.as_str() {
        "gitWorkingTree" => {
            let repo_path = meta
                .get("repo_path")
                .and_then(|value| value.as_str())
                .ok_or("Missing repo_path for git working tree diffset")?;
            GitProvider.replace_target(conn, &diffset, &ImportTarget::GitWorkingTree { repo_path: repo_path.to_string() })?;
            Ok(true)
        }
        "p4Pending" | "p4PendingDefault" => {
            let change = meta
                .get("change")
                .and_then(|value| value.as_str())
                .ok_or("Missing change for p4 pending diffset")?;
            let cwd = meta.get("cwd").and_then(|value| value.as_str());
            P4Provider.replace_target(conn, &diffset, &ImportTarget::P4Pending { change: change.to_string(), cwd: cwd.map(|s| s.to_string()) })?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(crate) struct DiffSetDescriptor {
    pub(crate) title: String,
    pub(crate) source_type: String,
    pub(crate) provider: String,
    pub(crate) kind: String,
    pub(crate) source_meta: serde_json::Value,
    pub(crate) left_label: String,
    pub(crate) right_label: String,
    pub(crate) write_target_mode: WriteTargetMode,
}

#[derive(Clone)]
pub(crate) enum WriteTargetMode {
    GitWorkingTree {
        repo_path: String,
    },
    GitCommit {
        repo_path: String,
        rev: String,
    },
    P4Pending {
        cwd: Option<String>,
        config: P4Config,
    },
    P4ReadOnly {
        cwd: Option<String>,
        config: P4Config,
    },
}

pub(crate) fn prefetch_p4_contents(
    mode: &WriteTargetMode,
    parsed: &[PatchFileDiff],
) -> Result<std::collections::HashMap<String, String>, String> {
    match mode {
        WriteTargetMode::P4Pending { cwd, config } => {
            let mut paths = Vec::new();
            for pf in parsed {
                if pf.is_binary {
                    continue; // skip prefetching for binaries
                }
                if pf.old_path.starts_with("//") && pf.old_path != "/dev/null" {
                    paths.push(pf.old_path.clone());
                }
            }
            prefetch_p4_file_contents(&paths, cwd.as_deref(), config)
        }
        WriteTargetMode::P4ReadOnly { cwd, config } => {
            let mut paths = Vec::new();
            for pf in parsed {
                if pf.is_binary {
                    continue; // skip prefetching for binaries
                }
                if pf.old_path.starts_with("//") && pf.old_path != "/dev/null" {
                    paths.push(pf.old_path.clone());
                }
                if pf.new_path.starts_with("//") && pf.new_path != "/dev/null" {
                    paths.push(pf.new_path.clone());
                }
            }
            prefetch_p4_file_contents(&paths, cwd.as_deref(), config)
        }
        _ => Ok(std::collections::HashMap::new()),
    }
}


pub(crate) fn import_unified_diff_text(
    conn: &rusqlite::Connection,
    workspace_id: &str,
    diff_text: &str,
    descriptor: DiffSetDescriptor,
    action_by_path: Option<&std::collections::HashMap<String, String>>,
) -> Result<String, String> {
    let parsed = unified_parser::parse_unified_diff(diff_text);
    DebugLogger::new("scm").log(format!(
        "import_unified_diff_text provider={} kind={} parsed_files={} diff_len={}",
        descriptor.provider,
        descriptor.kind,
        parsed.len(),
        diff_text.len()
    ));
    import_parsed_diff_text(conn, workspace_id, &parsed, descriptor, action_by_path)
}

pub(crate) fn import_parsed_diff_text(
    conn: &rusqlite::Connection,
    workspace_id: &str,
    parsed: &[PatchFileDiff],
    descriptor: DiffSetDescriptor,
    action_by_path: Option<&std::collections::HashMap<String, String>>,
) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp();
    let diffset_id = uuid::Uuid::new_v4().to_string();
    let tx = conn
        .unchecked_transaction()
        .map_err(|err| err.to_string())?;

    store::insert_diffset(
        &tx,
        &DiffSet {
            diffset_id: diffset_id.clone(),
            workspace_id: workspace_id.to_string(),
            title: descriptor.title,
            source_type: descriptor.source_type,
            provider: descriptor.provider,
            kind: descriptor.kind,
            source_meta_json: descriptor.source_meta.to_string(),
            created_at: now,
        },
    )?;

    let p4_cache = prefetch_p4_contents(&descriptor.write_target_mode, parsed)?;

    for pf in parsed {
        insert_patch_file_diff(
            &tx,
            &diffset_id,
            pf,
            &descriptor.left_label,
            &descriptor.right_label,
            &descriptor.write_target_mode,
            action_by_path,
            now,
            &p4_cache,
        )?;
    }

    tx.commit().map_err(|err| err.to_string())?;
    Ok(diffset_id)
}

pub(crate) fn replace_diffset_contents(
    conn: &rusqlite::Connection,
    diffset: &DiffSet,
    diff_text: &str,
    descriptor: DiffSetDescriptor,
    action_by_path: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), String> {
    let parsed = unified_parser::parse_unified_diff(diff_text);
    replace_parsed_diffset_contents(conn, diffset, &parsed, descriptor, action_by_path)
}

pub(crate) fn replace_parsed_diffset_contents(
    conn: &rusqlite::Connection,
    diffset: &DiffSet,
    parsed: &[PatchFileDiff],
    descriptor: DiffSetDescriptor,
    action_by_path: Option<&std::collections::HashMap<String, String>>,
) -> Result<(), String> {
    DebugLogger::new("scm").log(format!(
        "replace_diffset_contents diffset_id={} provider={} kind={} parsed_files={}",
        diffset.diffset_id,
        descriptor.provider,
        descriptor.kind,
        parsed.len(),
    ));
    let DiffSetDescriptor {
        title,
        source_type,
        provider,
        kind,
        source_meta,
        left_label,
        right_label,
        write_target_mode,
    } = descriptor;
    let updated = DiffSet {
        diffset_id: diffset.diffset_id.clone(),
        workspace_id: diffset.workspace_id.clone(),
        title,
        source_type,
        provider,
        kind,
        source_meta_json: source_meta.to_string(),
        created_at: diffset.created_at,
    };
    let tx = conn
        .unchecked_transaction()
        .map_err(|err| err.to_string())?;
    store::update_diffset(&tx, &updated)?;
    store::delete_filediffs_for_diffset(&tx, &diffset.diffset_id)?;

    let p4_cache = prefetch_p4_contents(&write_target_mode, parsed)?;

    for pf in parsed {
        insert_patch_file_diff(
            &tx,
            &diffset.diffset_id,
            pf,
            &left_label,
            &right_label,
            &write_target_mode,
            action_by_path,
            chrono::Utc::now().timestamp(),
            &p4_cache,
        )?;
    }

    tx.commit().map_err(|err| err.to_string())?;
    Ok(())
}

fn insert_patch_file_diff(
    conn: &rusqlite::Connection,
    diffset_id: &str,
    pf: &PatchFileDiff,
    left_label: &str,
    right_label: &str,
    write_target_mode: &WriteTargetMode,
    action_by_path: Option<&std::collections::HashMap<String, String>>,
    now: i64,
    cache: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let hunks_json = serde_json::to_string(&pf.hunks).unwrap_or_else(|_| "[]".to_string());
    let display_path = display_path_for_patch(pf);
    let action = action_by_path
        .and_then(|map| map.get(&strip_p4_rev(&display_path)).cloned())
        .unwrap_or_else(|| pf.status.clone());
    let write_target = derive_write_target(write_target_mode, pf, &display_path);
    let (content_left_json, content_right_json) =
        derive_content_sources(write_target_mode, pf, &display_path, cache)?;
    DebugLogger::new("scm").log_diff_line_counts(
        &display_path,
        &content_left_json,
        &content_right_json,
    );

    store::insert_filediff(
        conn,
        &FileDiff {
            filediff_id: uuid::Uuid::new_v4().to_string(),
            diffset_id: diffset_id.to_string(),
            display_path,
            status: action,
            left_label: if left_label.is_empty() {
                pf.old_path.clone()
            } else {
                left_label.to_string()
            },
            right_label: if right_label.is_empty() {
                pf.new_path.clone()
            } else {
                right_label.to_string()
            },
            content_left_json,
            content_right_json,
            hunks_json,
            write_target_json: write_target.to_json_string(),
            created_at: now,
        },
    )
}

fn derive_content_sources(
    mode: &WriteTargetMode,
    pf: &PatchFileDiff,
    display_path: &str,
    cache: &std::collections::HashMap<String, String>,
) -> Result<(String, String), String> {
    if pf.is_binary {
        let empty = ContentSource::virtual_text("").to_json_string();
        return Ok((empty.clone(), empty));
    }

    match mode {
        WriteTargetMode::GitWorkingTree { repo_path } => {
            let old_rel = if pf.old_path != "/dev/null" {
                pf.old_path.as_str()
            } else {
                display_path
            };
            let right_abs = std::path::Path::new(repo_path).join(display_path);
            let left_text = if pf.old_path == "/dev/null" {
                String::new()
            } else {
                git_show_file(repo_path, old_rel)?
            };
            let right_json = if pf.new_path == "/dev/null" || !right_abs.exists() {
                ContentSource::virtual_text(reconstruct_new_text(pf)).to_json_string()
            } else {
                ContentSource::path(right_abs.to_string_lossy()).to_json_string()
            };
            Ok((
                ContentSource::virtual_text(left_text).to_json_string(),
                right_json,
            ))
        }
        WriteTargetMode::GitCommit { repo_path, rev } => {
            let old_rel = if pf.old_path != "/dev/null" {
                pf.old_path.as_str()
            } else {
                display_path
            };
            let new_rel = if pf.new_path != "/dev/null" {
                pf.new_path.as_str()
            } else {
                display_path
            };
            let left_text = if pf.old_path == "/dev/null" {
                String::new()
            } else {
                git_show_file_at_rev(repo_path, &format!("{}^", rev), old_rel)?
            };
            let right_text = if pf.new_path == "/dev/null" {
                String::new()
            } else {
                git_show_file_at_rev(repo_path, rev, new_rel)?
            };
            Ok((
                ContentSource::virtual_text(left_text).to_json_string(),
                ContentSource::virtual_text(right_text).to_json_string(),
            ))
        }
        WriteTargetMode::P4Pending { cwd, config } => {
            let left_json = if pf.old_path == "/dev/null" {
                ContentSource::virtual_text("").to_json_string()
            } else if pf.old_path.starts_with("//") {
                let text = if let Some(content) = cache.get(&pf.old_path) {
                    content.clone()
                } else {
                    p4_print_file(&pf.old_path, cwd.as_deref(), config)?
                };
                ContentSource::virtual_text(text).to_json_string()
            } else {
                ContentSource::virtual_text(reconstruct_old_text(pf)).to_json_string()
            };
            let right_json = if let Some(local_path) = pending_local_path(pf, cwd.as_deref()) {
                if std::path::Path::new(&local_path).exists() {
                    ContentSource::path(local_path).to_json_string()
                } else {
                    ContentSource::virtual_text(reconstruct_new_text(pf)).to_json_string()
                }
            } else {
                ContentSource::virtual_text(reconstruct_new_text(pf)).to_json_string()
            };
            Ok((left_json, right_json))
        }
        WriteTargetMode::P4ReadOnly { cwd, config } => {
            let left_text = if pf.old_path == "/dev/null" {
                String::new()
            } else if pf.old_path.starts_with("//") {
                if let Some(content) = cache.get(&pf.old_path) {
                    content.clone()
                } else {
                    p4_print_file(&pf.old_path, cwd.as_deref(), config)?
                }
            } else {
                reconstruct_old_text(pf)
            };
            let right_text = if pf.new_path == "/dev/null" {
                String::new()
            } else if pf.new_path.starts_with("//") {
                if let Some(content) = cache.get(&pf.new_path) {
                    content.clone()
                } else {
                    p4_print_file(&pf.new_path, cwd.as_deref(), config)?
                }
            } else {
                reconstruct_new_text(pf)
            };
            Ok((
                ContentSource::virtual_text(left_text).to_json_string(),
                ContentSource::virtual_text(right_text).to_json_string(),
            ))
        }
    }
}

fn derive_write_target(
    mode: &WriteTargetMode,
    pf: &PatchFileDiff,
    display_path: &str,
) -> WriteTarget {
    match mode {
        WriteTargetMode::GitWorkingTree { repo_path } => {
            let resolved = std::path::Path::new(repo_path).join(display_path);
            WriteTarget::path(resolved.to_string_lossy())
        }
        WriteTargetMode::GitCommit { .. } => WriteTarget::ReadOnly,
        WriteTargetMode::P4Pending { cwd, .. } => {
            if let Some(local_path) = pending_local_path(pf, cwd.as_deref()) {
                WriteTarget::path(local_path)
            } else {
                WriteTarget::SaveAsRequired
            }
        }
        WriteTargetMode::P4ReadOnly { .. } => WriteTarget::ReadOnly,
    }
}

pub(crate) fn display_path_for_patch(pf: &PatchFileDiff) -> String {
    if pf.old_path.starts_with("//") {
        strip_p4_rev(&pf.old_path)
    } else if pf.new_path.starts_with("//") {
        strip_p4_rev(&pf.new_path)
    } else if pf.new_path == "/dev/null" {
        strip_p4_rev(&pf.old_path)
    } else {
        strip_p4_rev(&pf.new_path)
    }
}

pub(crate) fn reconstruct_old_text(pf: &PatchFileDiff) -> String {
    let mut lines = Vec::new();
    for hunk in &pf.hunks {
        for hl in &hunk.lines {
            match hl.kind.as_str() {
                "context" | "del" => lines.push(hl.text.clone()),
                _ => {}
            }
        }
    }
    lines.join("\n")
}

pub(crate) fn reconstruct_new_text(pf: &PatchFileDiff) -> String {
    let mut lines = Vec::new();
    for hunk in &pf.hunks {
        for hl in &hunk.lines {
            match hl.kind.as_str() {
                "context" | "add" => lines.push(hl.text.clone()),
                _ => {}
            }
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_depot_display_path_for_pending_p4_diffs() {
        let pf = PatchFileDiff {
            old_path: "//depot/main/a.cpp".to_string(),
            new_path: "C:\\work\\a.cpp".to_string(),
            hunks: Vec::new(),
            status: "renamed".to_string(),
        };
        assert_eq!(display_path_for_patch(&pf), "//depot/main/a.cpp");
    }
}
