use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use crate::diff_engine::unified_parser::{self, PatchFileDiff};
use crate::store::{self, DiffSet, FileDiff};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P4OpenedFile {
    pub depot_path: String,
    pub action: String,
    pub change: String,
}

pub fn import_git_working_tree(
    conn: &Connection,
    workspace_id: &str,
    repo_path: &str,
) -> Result<String, String> {
    let output = run_command(
        "git",
        &["-C", repo_path, "diff", "--no-color", "--no-ext-diff", "--unified=3"],
        None,
    )?;
    let title = format!("Git working tree: {}", display_repo_name(repo_path));
    import_unified_diff_text(
        conn,
        workspace_id,
        &output,
        DiffSetDescriptor {
            title,
            source_type: "Git".to_string(),
            provider: "git".to_string(),
            kind: "gitWorkingTree".to_string(),
            source_meta: serde_json::json!({
                "repo_path": repo_path,
                "file_count": unified_parser::parse_unified_diff(&output).len()
            }),
            left_label: "HEAD".to_string(),
            right_label: "working tree".to_string(),
            write_target: serde_json::json!({ "type": "save_as_required" }),
        },
        None,
    )
}

pub fn refresh_diffset(conn: &Connection, diffset_id: &str) -> Result<bool, String> {
    let diffset = store::get_diffset(conn, diffset_id)?;
    let meta: serde_json::Value =
        serde_json::from_str(&diffset.source_meta_json).map_err(|e| e.to_string())?;

    match diffset.kind.as_str() {
        "gitWorkingTree" => {
            let repo_path = meta
                .get("repo_path")
                .and_then(|value| value.as_str())
                .ok_or("Missing repo_path for git working tree diffset")?;
            replace_git_working_tree(conn, &diffset, repo_path)?;
            Ok(true)
        }
        "p4Pending" | "p4PendingDefault" => {
            let change = meta
                .get("change")
                .and_then(|value| value.as_str())
                .ok_or("Missing change for p4 pending diffset")?;
            let cwd = meta.get("cwd").and_then(|value| value.as_str());
            replace_p4_pending(conn, &diffset, change, cwd)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub fn import_git_commit(
    conn: &Connection,
    workspace_id: &str,
    repo_path: &str,
    rev: &str,
) -> Result<String, String> {
    let output = run_command(
        "git",
        &[
            "-C",
            repo_path,
            "show",
            "--format=medium",
            "--no-color",
            "--no-ext-diff",
            "--unified=3",
            rev,
        ],
        None,
    )?;
    let subject = output
        .lines()
        .find_map(|line| line.strip_prefix("    "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or(rev);
    import_unified_diff_text(
        conn,
        workspace_id,
        &output,
        DiffSetDescriptor {
            title: format!("{} {}", short_rev(rev), subject),
            source_type: "Git".to_string(),
            provider: "git".to_string(),
            kind: "gitCommit".to_string(),
            source_meta: serde_json::json!({
                "repo_path": repo_path,
                "rev": rev,
                "file_count": unified_parser::parse_unified_diff(&output).len()
            }),
            left_label: format!("{}^", short_rev(rev)),
            right_label: short_rev(rev),
            write_target: serde_json::json!({ "type": "save_as_required" }),
        },
        None,
    )
}

pub fn import_p4_pending(
    conn: &Connection,
    workspace_id: &str,
    change: &str,
    cwd: Option<&str>,
) -> Result<String, String> {
    let opened = run_command("p4", &["opened", "-c", change], cwd)?;
    let opened_files = parse_p4_opened(&opened);
    let mut args: Vec<String> = vec!["diff".to_string(), "-du".to_string()];
    args.extend(opened_files.iter().map(|file| file.depot_path.clone()));
    let diff = if opened_files.is_empty() {
        String::new()
    } else {
        run_command_owned("p4", &args, cwd)?
    };
    let title = if change == "default" {
        "P4 default pending changelist".to_string()
    } else {
        format!("P4 pending changelist {}", change)
    };
    let action_map = opened_files
        .iter()
        .map(|file| (strip_p4_rev(&file.depot_path), file.action.clone()))
        .collect::<HashMap<_, _>>();
    import_unified_diff_text(
        conn,
        workspace_id,
        &diff,
        DiffSetDescriptor {
            title,
            source_type: "Perforce".to_string(),
            provider: "p4".to_string(),
            kind: if change == "default" { "p4PendingDefault" } else { "p4Pending" }.to_string(),
            source_meta: serde_json::json!({
                "change": change,
                "status": if change == "default" { "Default" } else { "Pending" },
                "file_count": opened_files.len(),
                "cwd": cwd
            }),
            left_label: "have revision".to_string(),
            right_label: "workspace".to_string(),
            write_target: serde_json::json!({ "type": "save_as_required" }),
        },
        Some(&action_map),
    )
    .and_then(|diffset_id| {
        add_opened_files_without_diffs(conn, &diffset_id, &opened_files)?;
        Ok(diffset_id)
    })
}

pub fn import_p4_shelved(
    conn: &Connection,
    workspace_id: &str,
    change: &str,
    cwd: Option<&str>,
) -> Result<String, String> {
    let output = run_command("p4", &["describe", "-S", "-du", change], cwd)?;
    let actions = parse_p4_describe_actions(&output);
    import_p4_describe(
        conn,
        workspace_id,
        change,
        cwd,
        &output,
        "p4Shelved",
        "Shelved",
        &format!("P4 shelved changelist {}", change),
        "depot previous/have",
        &format!("shelf @={}", change),
        &actions,
    )
}

pub fn import_p4_submitted(
    conn: &Connection,
    workspace_id: &str,
    change: &str,
    cwd: Option<&str>,
) -> Result<String, String> {
    let output = run_command("p4", &["describe", "-du", change], cwd)?;
    let actions = parse_p4_describe_actions(&output);
    import_p4_describe(
        conn,
        workspace_id,
        change,
        cwd,
        &output,
        "p4Submitted",
        "Submitted",
        &format!("P4 submitted changelist {}", change),
        "#rev-1",
        &format!("#rev @{}", change),
        &actions,
    )
}

fn import_p4_describe(
    conn: &Connection,
    workspace_id: &str,
    change: &str,
    cwd: Option<&str>,
    output: &str,
    kind: &str,
    status: &str,
    fallback_title: &str,
    left_label: &str,
    right_label: &str,
    actions: &HashMap<String, String>,
) -> Result<String, String> {
    let desc = first_p4_description_line(output);
    let title = desc
        .map(|line| format!("{}: {}", fallback_title, line))
        .unwrap_or_else(|| fallback_title.to_string());
    import_unified_diff_text(
        conn,
        workspace_id,
        output,
        DiffSetDescriptor {
            title,
            source_type: "Perforce".to_string(),
            provider: "p4".to_string(),
            kind: kind.to_string(),
            source_meta: serde_json::json!({
                "change": change,
                "status": status,
                "file_count": actions.len().max(unified_parser::parse_unified_diff(output).len()),
                "cwd": cwd
            }),
            left_label: left_label.to_string(),
            right_label: right_label.to_string(),
            write_target: serde_json::json!({ "type": "save_as_required" }),
        },
        Some(actions),
    )
}

fn replace_git_working_tree(conn: &Connection, diffset: &DiffSet, repo_path: &str) -> Result<(), String> {
    let output = run_command(
        "git",
        &["-C", repo_path, "diff", "--no-color", "--no-ext-diff", "--unified=3"],
        None,
    )?;
    let title = format!("Git working tree: {}", display_repo_name(repo_path));
    replace_diffset_contents(
        conn,
        diffset,
        &output,
        DiffSetDescriptor {
            title,
            source_type: "Git".to_string(),
            provider: "git".to_string(),
            kind: "gitWorkingTree".to_string(),
            source_meta: serde_json::json!({
                "repo_path": repo_path,
                "file_count": unified_parser::parse_unified_diff(&output).len()
            }),
            left_label: "HEAD".to_string(),
            right_label: "working tree".to_string(),
            write_target: serde_json::json!({ "type": "save_as_required" }),
        },
        None,
    )
}

fn replace_p4_pending(
    conn: &Connection,
    diffset: &DiffSet,
    change: &str,
    cwd: Option<&str>,
) -> Result<(), String> {
    let opened = run_command("p4", &["opened", "-c", change], cwd)?;
    let opened_files = parse_p4_opened(&opened);
    let mut args: Vec<String> = vec!["diff".to_string(), "-du".to_string()];
    args.extend(opened_files.iter().map(|file| file.depot_path.clone()));
    let diff = if opened_files.is_empty() {
        String::new()
    } else {
        run_command_owned("p4", &args, cwd)?
    };
    let title = if change == "default" {
        "P4 default pending changelist".to_string()
    } else {
        format!("P4 pending changelist {}", change)
    };
    let action_map = opened_files
        .iter()
        .map(|file| (strip_p4_rev(&file.depot_path), file.action.clone()))
        .collect::<HashMap<_, _>>();
    replace_diffset_contents(
        conn,
        diffset,
        &diff,
        DiffSetDescriptor {
            title,
            source_type: "Perforce".to_string(),
            provider: "p4".to_string(),
            kind: if change == "default" { "p4PendingDefault" } else { "p4Pending" }.to_string(),
            source_meta: serde_json::json!({
                "change": change,
                "status": if change == "default" { "Default" } else { "Pending" },
                "file_count": opened_files.len(),
                "cwd": cwd
            }),
            left_label: "have revision".to_string(),
            right_label: "workspace".to_string(),
            write_target: serde_json::json!({ "type": "save_as_required" }),
        },
        Some(&action_map),
    )?;
    add_opened_files_without_diffs(conn, &diffset.diffset_id, &opened_files)?;
    Ok(())
}

struct DiffSetDescriptor {
    title: String,
    source_type: String,
    provider: String,
    kind: String,
    source_meta: serde_json::Value,
    left_label: String,
    right_label: String,
    write_target: serde_json::Value,
}

fn import_unified_diff_text(
    conn: &Connection,
    workspace_id: &str,
    diff_text: &str,
    descriptor: DiffSetDescriptor,
    action_by_path: Option<&HashMap<String, String>>,
) -> Result<String, String> {
    let parsed = unified_parser::parse_unified_diff(diff_text);
    let now = chrono::Utc::now().timestamp();
    let diffset_id = uuid::Uuid::new_v4().to_string();

    store::insert_diffset(
        conn,
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

    for pf in &parsed {
        insert_patch_file_diff(
            conn,
            &diffset_id,
            pf,
            &descriptor.left_label,
            &descriptor.right_label,
            descriptor.write_target.clone(),
            action_by_path,
            now,
        )?;
    }

    Ok(diffset_id)
}

fn replace_diffset_contents(
    conn: &Connection,
    diffset: &DiffSet,
    diff_text: &str,
    descriptor: DiffSetDescriptor,
    action_by_path: Option<&HashMap<String, String>>,
) -> Result<(), String> {
    let parsed = unified_parser::parse_unified_diff(diff_text);
    let DiffSetDescriptor {
        title,
        source_type,
        provider,
        kind,
        source_meta,
        left_label,
        right_label,
        write_target,
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
    store::update_diffset(conn, &updated)?;
    store::delete_filediffs_for_diffset(conn, &diffset.diffset_id)?;

    for pf in &parsed {
        insert_patch_file_diff(
            conn,
            &diffset.diffset_id,
            pf,
            &left_label,
            &right_label,
            write_target.clone(),
            action_by_path,
            chrono::Utc::now().timestamp(),
        )?;
    }

    Ok(())
}

fn insert_patch_file_diff(
    conn: &Connection,
    diffset_id: &str,
    pf: &PatchFileDiff,
    left_label: &str,
    right_label: &str,
    write_target: serde_json::Value,
    action_by_path: Option<&HashMap<String, String>>,
    now: i64,
) -> Result<(), String> {
    let hunks_json = serde_json::to_string(&pf.hunks).unwrap_or_else(|_| "[]".to_string());
    let left_text = reconstruct_old_text(pf);
    let right_text = reconstruct_new_text(pf);
    let display_path = display_path_for_patch(pf);
    let action = action_by_path
        .and_then(|map| map.get(&strip_p4_rev(&display_path)).cloned())
        .unwrap_or_else(|| pf.status.clone());

    store::insert_filediff(
        conn,
        &FileDiff {
            filediff_id: uuid::Uuid::new_v4().to_string(),
            diffset_id: diffset_id.to_string(),
            display_path,
            status: action,
            left_label: if left_label.is_empty() { pf.old_path.clone() } else { left_label.to_string() },
            right_label: if right_label.is_empty() { pf.new_path.clone() } else { right_label.to_string() },
            content_left_json: serde_json::json!({ "type": "virtual", "text": left_text }).to_string(),
            content_right_json: serde_json::json!({ "type": "virtual", "text": right_text }).to_string(),
            hunks_json,
            write_target_json: write_target.to_string(),
            created_at: now,
        },
    )
}

fn add_opened_files_without_diffs(
    conn: &Connection,
    diffset_id: &str,
    opened_files: &[P4OpenedFile],
) -> Result<(), String> {
    let existing = store::list_filediffs(conn, diffset_id)?
        .into_iter()
        .map(|fd| strip_p4_rev(&fd.display_path))
        .collect::<HashSet<_>>();
    let now = chrono::Utc::now().timestamp();

    for file in opened_files {
        let path = strip_p4_rev(&file.depot_path);
        if existing.contains(&path) {
            continue;
        }
        store::insert_filediff(
            conn,
            &FileDiff {
                filediff_id: uuid::Uuid::new_v4().to_string(),
                diffset_id: diffset_id.to_string(),
                display_path: path,
                status: format!("{} no-diff", file.action),
                left_label: "have revision".to_string(),
                right_label: "workspace".to_string(),
                content_left_json: serde_json::json!({ "type": "virtual", "text": "" }).to_string(),
                content_right_json: serde_json::json!({ "type": "virtual", "text": "" }).to_string(),
                hunks_json: "[]".to_string(),
                write_target_json: serde_json::json!({ "type": "save_as_required" }).to_string(),
                created_at: now,
            },
        )?;
    }

    Ok(())
}

pub fn parse_p4_opened(output: &str) -> Vec<P4OpenedFile> {
    output
        .lines()
        .filter_map(|line| {
            let (left, right) = line.split_once(" - ")?;
            let depot_path = left.split('#').next().unwrap_or(left).trim().to_string();
            let action = right.split_whitespace().next().unwrap_or("edit").to_string();
            let change = if right.contains(" default change") {
                "default".to_string()
            } else {
                right
                    .split(" change ")
                    .nth(1)
                    .and_then(|tail| tail.split_whitespace().next())
                    .unwrap_or("default")
                    .to_string()
            };
            Some(P4OpenedFile {
                depot_path,
                action,
                change,
            })
        })
        .collect()
}

pub fn parse_p4_describe_actions(output: &str) -> HashMap<String, String> {
    let mut actions = HashMap::new();
    for line in output.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("... ") else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let Some(path_with_rev) = parts.next() else {
            continue;
        };
        let Some(action) = parts.next() else {
            continue;
        };
        actions.insert(strip_p4_rev(path_with_rev), action.to_string());
    }
    actions
}

fn first_p4_description_line(output: &str) -> Option<String> {
    let mut in_desc = false;
    for line in output.lines() {
        if line.trim() == "Description:" {
            in_desc = true;
            continue;
        }
        if in_desc {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == "Affected files ..." {
                return None;
            }
            return Some(trimmed.to_string());
        }
    }
    None
}

fn run_command(program: &str, args: &[&str], cwd: Option<&str>) -> Result<String, String> {
    let args = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    run_command_owned(program, &args, cwd)
}

fn run_command_owned(program: &str, args: &[String], cwd: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(cwd) = cwd.filter(|value| !value.trim().is_empty()) {
        cmd.current_dir(cwd);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run {}: {}", program, e))?;
    if !output.status.success() {
        return Err(format!(
            "{} {} failed: {}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn display_repo_name(repo_path: &str) -> String {
    Path::new(repo_path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| repo_path.to_string())
}

fn short_rev(rev: &str) -> String {
    rev.chars().take(12).collect()
}

fn strip_p4_rev(path: &str) -> String {
    let path = path.trim();
    if path.starts_with("//") {
        if let Some((before, after)) = path.rsplit_once('#') {
            if after.chars().all(|ch| ch.is_ascii_digit()) {
                return before.to_string();
            }
        }
    }
    path.to_string()
}

fn display_path_for_patch(pf: &PatchFileDiff) -> String {
    if pf.new_path == "/dev/null" {
        strip_p4_rev(&pf.old_path)
    } else {
        strip_p4_rev(&pf.new_path)
    }
}

fn reconstruct_old_text(pf: &PatchFileDiff) -> String {
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

fn reconstruct_new_text(pf: &PatchFileDiff) -> String {
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
    fn parses_p4_opened_lines() {
        let files = parse_p4_opened(
            "//depot/main/a.cpp#3 - edit default change (text)\n//depot/main/b.cpp#1 - add change 12345 (text)\n",
        );
        assert_eq!(
            files,
            vec![
                P4OpenedFile {
                    depot_path: "//depot/main/a.cpp".to_string(),
                    action: "edit".to_string(),
                    change: "default".to_string(),
                },
                P4OpenedFile {
                    depot_path: "//depot/main/b.cpp".to_string(),
                    action: "add".to_string(),
                    change: "12345".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parses_p4_describe_actions() {
        let actions = parse_p4_describe_actions(
            "Affected files ...\n\n... //depot/main/a.cpp#4 edit\n... //depot/main/b.cpp#1 add\n",
        );
        assert_eq!(actions.get("//depot/main/a.cpp"), Some(&"edit".to_string()));
        assert_eq!(actions.get("//depot/main/b.cpp"), Some(&"add".to_string()));
    }
}
