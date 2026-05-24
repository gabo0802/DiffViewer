use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::content_source::{ContentSource, WriteTarget};
use crate::debugging::DebugLogger;
use crate::diff_engine::{twoway, unified_parser::{self, PatchFileDiff}};
use crate::io;
use crate::store::{self, DiffSet, FileDiff};

mod p4_config;
mod process;
pub mod pr_api;

use p4_config::{load_p4_config, P4Config};
use process::{run_command, run_p4, run_p4_owned};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P4OpenedFile {
    pub depot_path: String,
    pub action: String,
    pub change: String,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct P4DescribeFile {
    depot_path: String,
    rev: Option<u32>,
    action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P4PendingChangeSummary {
    pub change: String,
    pub description: String,
    pub client: Option<String>,
    pub user: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitSummary {
    pub rev: String,
    pub short_rev: String,
    pub subject: String,
    pub relative_time: String,
}

pub fn import_git_working_tree(
    conn: &Connection,
    workspace_id: &str,
    repo_path: &str,
) -> Result<String, String> {
    let parsed = load_git_working_tree_file_diffs(repo_path)?;
    let title = format!("Git working tree: {}", display_repo_name(repo_path));
    import_parsed_diff_text(
        conn,
        workspace_id,
        &parsed,
        DiffSetDescriptor {
            title,
            source_type: "Git".to_string(),
            provider: "git".to_string(),
            kind: "gitWorkingTree".to_string(),
            source_meta: serde_json::json!({
                "repo_path": repo_path,
                "file_count": parsed.len()
            }),
            left_label: "HEAD".to_string(),
            right_label: "working tree".to_string(),
            write_target_mode: WriteTargetMode::GitWorkingTree {
                repo_path: repo_path.to_string(),
            },
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
            write_target_mode: WriteTargetMode::GitCommit {
                repo_path: repo_path.to_string(),
                rev: rev.to_string(),
            },
        },
        None,
    )
}

pub fn list_git_commits(repo_path: &str, limit: usize, branch: Option<&str>) -> Result<Vec<GitCommitSummary>, String> {
    let max_count = limit.max(1).min(100).to_string();
    let mut args = vec![
        "-C",
        repo_path,
        "log",
        "--max-count",
        &max_count,
        "--pretty=format:%H%x1f%h%x1f%s%x1f%cr",
    ];
    if let Some(b) = branch {
        args.push(b);
    }

    let output = run_command("git", &args, None)?;

    Ok(output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            let rev = parts.next()?.trim();
            let short_rev = parts.next()?.trim();
            let subject = parts.next()?.trim();
            let relative_time = parts.next()?.trim();
            if rev.is_empty() {
                return None;
            }
            Some(GitCommitSummary {
                rev: rev.to_string(),
                short_rev: short_rev.to_string(),
                subject: subject.to_string(),
                relative_time: relative_time.to_string(),
            })
        })
        .collect())
}

pub fn list_git_branches(repo_path: &str) -> Result<Vec<String>, String> {
    let output = run_command(
        "git",
        &[
            "-C",
            repo_path,
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/",
            "refs/remotes/",
        ],
        None,
    )?;

    let mut branches: Vec<String> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect();
    
    branches.sort();
    branches.dedup();
    Ok(branches)
}

pub fn get_pull_requests(
    conn: &Connection,
    workspace_id: &str,
    repo_path: &str,
) -> Result<Vec<pr_api::PullRequestSummary>, String> {
    let remote_url = run_command(
        "git",
        &["-C", repo_path, "remote", "get-url", "origin"],
        None,
    ).map(|s| s.trim().to_string())?;

    let settings = crate::services::workspace_service::get_settings(conn, workspace_id)?;
    let repo_info = pr_api::parse_remote_url(&remote_url, settings.gitlab_host_url.as_deref())
        .ok_or_else(|| "Could not parse origin remote URL as GitHub or GitLab".to_string())?;

    pr_api::get_pull_requests(&repo_info, settings.github_pat.as_deref(), settings.gitlab_pat.as_deref())
}

pub fn import_git_pull_request(
    conn: &Connection,
    workspace_id: &str,
    repo_path: &str,
    pr_id: &str,
    target_branch: &str,
    pr_title: Option<&str>,
) -> Result<String, String> {
    // 1. Determine host
    let remote_url = run_command(
        "git",
        &["-C", repo_path, "remote", "get-url", "origin"],
        None,
    ).map(|s| s.trim().to_string())?;
    
    let settings = crate::services::workspace_service::get_settings(conn, workspace_id)?;
    let repo_info = pr_api::parse_remote_url(&remote_url, settings.gitlab_host_url.as_deref())
        .ok_or_else(|| "Could not parse origin remote URL".to_string())?;

    // 2. Fetch the PR/MR ref
    let fetch_ref = match repo_info.host {
        pr_api::RepoHost::GitHub => format!("pull/{}/head", pr_id),
        pr_api::RepoHost::GitLab(_) => format!("merge-requests/{}/head", pr_id),
    };
    
    run_command(
        "git",
        &["-C", repo_path, "fetch", "origin", &fetch_ref],
        None,
    )?;

    // 3. Compute the diff using git diff target_branch...FETCH_HEAD
    let output = run_command(
        "git",
        &[
            "-C",
            repo_path,
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--unified=3",
            &format!("{}...FETCH_HEAD", target_branch),
        ],
        None,
    )?;

    let (title_prefix, pr_type) = match repo_info.host {
        pr_api::RepoHost::GitHub => ("PR", "PR"),
        pr_api::RepoHost::GitLab(_) => ("MR", "MR"),
    };
    
    let title = if let Some(t) = pr_title {
        format!("{} #{}: {}", title_prefix, pr_id, t)
    } else {
        format!("{} #{}", title_prefix, pr_id)
    };
    
    import_unified_diff_text(
        conn,
        workspace_id,
        &output,
        DiffSetDescriptor {
            title: title.clone(),
            source_type: "Git".to_string(),
            provider: "git".to_string(),
            kind: "gitPullRequest".to_string(),
            source_meta: serde_json::json!({
                "repo_path": repo_path,
                "pr_id": pr_id,
                "target_branch": target_branch,
                "pr_type": pr_type,
                "file_count": unified_parser::parse_unified_diff(&output).len()
            }),
            left_label: target_branch.to_string(),
            right_label: format!("PR #{}", pr_id),
            write_target_mode: WriteTargetMode::GitCommit {
                repo_path: repo_path.to_string(),
                rev: "FETCH_HEAD".to_string(),
            },
        },
        None,
    )
}

pub fn list_p4_pending_changes(cwd: Option<&str>) -> Result<Vec<P4PendingChangeSummary>, String> {
    let p4_config = load_p4_config(cwd);
    let mut args = vec![
        "changes".to_string(),
        "-s".to_string(),
        "pending".to_string(),
    ];
    if let Some(client) = p4_config
        .client
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.push("-c".to_string());
        args.push(client.to_string());
    }

    let output = run_p4_owned(&args, cwd, &p4_config)?;
    let mut changes = vec![P4PendingChangeSummary {
        change: "default".to_string(),
        description: "Default pending changelist".to_string(),
        client: p4_config.client.clone(),
        user: p4_config.user.clone(),
        is_default: true,
    }];

    for raw_line in output.lines() {
        let Some(summary) = parse_p4_pending_change_line(raw_line) else {
            continue;
        };
        changes.push(summary);
    }

    Ok(changes)
}

pub fn import_p4_pending(
    conn: &Connection,
    workspace_id: &str,
    change: &str,
    cwd: Option<&str>,
) -> Result<String, String> {
    let debug = DebugLogger::new("scm");
    debug.log(format!(
        "import_p4_pending change={:?} cwd={:?}",
        change, cwd
    ));
    let p4_config = load_p4_config(cwd);
    debug.log(format!("import_p4_pending config={:?}", p4_config));
    let opened = run_p4(&opened_args(change, &p4_config), cwd, &p4_config)?;
    let mut opened_files = parse_p4_opened(&opened);
    debug.log(format!(
        "import_p4_pending opened_files={} configured_client={:?}",
        opened_files.len(),
        p4_config.client
    ));
    populate_p4_local_paths(&mut opened_files, cwd, &p4_config)?;
    let diff = collect_pending_p4_diff(&opened_files, cwd, &p4_config)?;
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
            kind: if change == "default" {
                "p4PendingDefault"
            } else {
                "p4Pending"
            }
            .to_string(),
            source_meta: serde_json::json!({
                "change": change,
                "status": if change == "default" { "Default" } else { "Pending" },
                "file_count": opened_files.len(),
                "cwd": cwd
            }),
            left_label: "have revision".to_string(),
            right_label: "workspace".to_string(),
            write_target_mode: WriteTargetMode::P4Pending {
                cwd: cwd.map(|value| value.to_string()),
                config: p4_config,
            },
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
    let debug = DebugLogger::new("scm");
    debug.log(format!(
        "import_p4_shelved change={:?} cwd={:?}",
        change, cwd
    ));
    let p4_config = load_p4_config(cwd);
    debug.log(format!("import_p4_shelved config={:?}", p4_config));
    let output = run_p4(&["describe", "-S", "-du", change], cwd, &p4_config)?;
    let described_files = parse_p4_describe_files(&output);
    let actions = describe_action_map(&described_files);
    debug.log(format!(
        "import_p4_shelved describe_actions={} output_len={}",
        actions.len(),
        output.len()
    ));
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
        &described_files,
    )
}

pub fn import_p4_submitted(
    conn: &Connection,
    workspace_id: &str,
    change: &str,
    cwd: Option<&str>,
) -> Result<String, String> {
    let debug = DebugLogger::new("scm");
    debug.log(format!(
        "import_p4_submitted change={:?} cwd={:?}",
        change, cwd
    ));
    let p4_config = load_p4_config(cwd);
    debug.log(format!("import_p4_submitted config={:?}", p4_config));
    let output = run_p4(&["describe", "-du", change], cwd, &p4_config)?;
    let described_files = parse_p4_describe_files(&output);
    let actions = describe_action_map(&described_files);
    debug.log(format!(
        "import_p4_submitted describe_actions={} output_len={}",
        actions.len(),
        output.len()
    ));
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
        &described_files,
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
    described_files: &[P4DescribeFile],
) -> Result<String, String> {
    let desc = first_p4_description_line(output);
    let p4_config = load_p4_config(cwd);
    let title = desc
        .map(|line| format!("{}: {}", fallback_title, line))
        .unwrap_or_else(|| fallback_title.to_string());
    let parsed = normalize_p4_describe_files(
        unified_parser::parse_unified_diff(output),
        described_files,
        kind,
        change,
    );
    import_parsed_diff_text(
        conn,
        workspace_id,
        &parsed,
        DiffSetDescriptor {
            title,
            source_type: "Perforce".to_string(),
            provider: "p4".to_string(),
            kind: kind.to_string(),
            source_meta: serde_json::json!({
                "change": change,
                "status": status,
                "file_count": described_files.len().max(parsed.len()),
                "cwd": cwd
            }),
            left_label: left_label.to_string(),
            right_label: right_label.to_string(),
            write_target_mode: WriteTargetMode::P4ReadOnly {
                cwd: cwd.map(|value| value.to_string()),
                config: p4_config,
            },
        },
        Some(&describe_action_map(described_files)),
    )
}

fn replace_git_working_tree(
    conn: &Connection,
    diffset: &DiffSet,
    repo_path: &str,
) -> Result<(), String> {
    let parsed = load_git_working_tree_file_diffs(repo_path)?;
    let title = format!("Git working tree: {}", display_repo_name(repo_path));
    replace_parsed_diffset_contents(
        conn,
        diffset,
        &parsed,
        DiffSetDescriptor {
            title,
            source_type: "Git".to_string(),
            provider: "git".to_string(),
            kind: "gitWorkingTree".to_string(),
            source_meta: serde_json::json!({
                "repo_path": repo_path,
                "file_count": parsed.len()
            }),
            left_label: "HEAD".to_string(),
            right_label: "working tree".to_string(),
            write_target_mode: WriteTargetMode::GitWorkingTree {
                repo_path: repo_path.to_string(),
            },
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
    let debug = DebugLogger::new("scm");
    debug.log(format!(
        "replace_p4_pending diffset_id={} change={:?} cwd={:?}",
        diffset.diffset_id, change, cwd
    ));
    let p4_config = load_p4_config(cwd);
    debug.log(format!("replace_p4_pending config={:?}", p4_config));
    let opened = run_p4(&opened_args(change, &p4_config), cwd, &p4_config)?;
    let mut opened_files = parse_p4_opened(&opened);
    debug.log(format!(
        "replace_p4_pending opened_files={} configured_client={:?}",
        opened_files.len(),
        p4_config.client
    ));
    populate_p4_local_paths(&mut opened_files, cwd, &p4_config)?;
    let diff = collect_pending_p4_diff(&opened_files, cwd, &p4_config)?;
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
            kind: if change == "default" {
                "p4PendingDefault"
            } else {
                "p4Pending"
            }
            .to_string(),
            source_meta: serde_json::json!({
                "change": change,
                "status": if change == "default" { "Default" } else { "Pending" },
                "file_count": opened_files.len(),
                "cwd": cwd
            }),
            left_label: "have revision".to_string(),
            right_label: "workspace".to_string(),
            write_target_mode: WriteTargetMode::P4Pending {
                cwd: cwd.map(|value| value.to_string()),
                config: p4_config,
            },
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
    write_target_mode: WriteTargetMode,
}

#[derive(Clone)]
enum WriteTargetMode {
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

fn import_unified_diff_text(
    conn: &Connection,
    workspace_id: &str,
    diff_text: &str,
    descriptor: DiffSetDescriptor,
    action_by_path: Option<&HashMap<String, String>>,
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

fn import_parsed_diff_text(
    conn: &Connection,
    workspace_id: &str,
    parsed: &[PatchFileDiff],
    descriptor: DiffSetDescriptor,
    action_by_path: Option<&HashMap<String, String>>,
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
        )?;
    }

    tx.commit().map_err(|err| err.to_string())?;
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
    replace_parsed_diffset_contents(conn, diffset, &parsed, descriptor, action_by_path)
}

fn replace_parsed_diffset_contents(
    conn: &Connection,
    diffset: &DiffSet,
    parsed: &[PatchFileDiff],
    descriptor: DiffSetDescriptor,
    action_by_path: Option<&HashMap<String, String>>,
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
        )?;
    }

    tx.commit().map_err(|err| err.to_string())?;
    Ok(())
}

fn load_git_working_tree_file_diffs(repo_path: &str) -> Result<Vec<PatchFileDiff>, String> {
    let mut parsed = unified_parser::parse_unified_diff(&git_working_tree_diff_text(repo_path)?);
    append_untracked_git_file_diffs(&mut parsed, repo_path)?;
    Ok(parsed)
}

fn git_working_tree_diff_text(repo_path: &str) -> Result<String, String> {
    if git_ref_exists(repo_path, "HEAD") {
        run_command(
            "git",
            &[
                "-C",
                repo_path,
                "diff",
                "--no-color",
                "--no-ext-diff",
                "--unified=3",
                "HEAD",
            ],
            None,
        )
    } else {
        run_command(
            "git",
            &[
                "-C",
                repo_path,
                "diff",
                "--cached",
                "--no-color",
                "--no-ext-diff",
                "--unified=3",
            ],
            None,
        )
    }
}

fn git_ref_exists(repo_path: &str, rev: &str) -> bool {
    run_command("git", &["-C", repo_path, "rev-parse", "--verify", rev], None).is_ok()
}

fn append_untracked_git_file_diffs(
    parsed: &mut Vec<PatchFileDiff>,
    repo_path: &str,
) -> Result<(), String> {
    let existing_paths = parsed
        .iter()
        .map(display_path_for_patch)
        .collect::<HashSet<_>>();

    for rel_path in list_untracked_git_paths(repo_path)? {
        if existing_paths.contains(&rel_path) {
            continue;
        }

        let abs_path = Path::new(repo_path).join(&rel_path);
        if !abs_path.is_file() {
            continue;
        }

        let text = io::read_file_text(&abs_path)?;
        parsed.push(synthetic_git_added_file_diff(&rel_path, &text));
    }

    Ok(())
}

fn list_untracked_git_paths(repo_path: &str) -> Result<Vec<String>, String> {
    let output = run_command(
        "git",
        &[
            "-C",
            repo_path,
            "ls-files",
            "--others",
            "--exclude-standard",
            "--full-name",
        ],
        None,
    )?;

    Ok(output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn synthetic_git_added_file_diff(rel_path: &str, text: &str) -> PatchFileDiff {
    PatchFileDiff {
        old_path: "/dev/null".to_string(),
        new_path: rel_path.to_string(),
        hunks: twoway::compute_hunks("", text),
        status: "added".to_string(),
    }
}

fn insert_patch_file_diff(
    conn: &Connection,
    diffset_id: &str,
    pf: &PatchFileDiff,
    left_label: &str,
    right_label: &str,
    write_target_mode: &WriteTargetMode,
    action_by_path: Option<&HashMap<String, String>>,
    now: i64,
) -> Result<(), String> {
    let hunks_json = serde_json::to_string(&pf.hunks).unwrap_or_else(|_| "[]".to_string());
    let display_path = display_path_for_patch(pf);
    let action = action_by_path
        .and_then(|map| map.get(&strip_p4_rev(&display_path)).cloned())
        .unwrap_or_else(|| pf.status.clone());
    let write_target = derive_write_target(write_target_mode, pf, &display_path);
    let (content_left_json, content_right_json) =
        derive_content_sources(write_target_mode, pf, &display_path)?;
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
) -> Result<(String, String), String> {
    match mode {
        WriteTargetMode::GitWorkingTree { repo_path } => {
            let old_rel = if pf.old_path != "/dev/null" {
                pf.old_path.as_str()
            } else {
                display_path
            };
            let right_abs = Path::new(repo_path).join(display_path);
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
                ContentSource::virtual_text(p4_print_file(&pf.old_path, cwd.as_deref(), config)?)
                    .to_json_string()
            } else {
                ContentSource::virtual_text(reconstruct_old_text(pf)).to_json_string()
            };
            let right_json = if let Some(local_path) = pending_local_path(pf, cwd.as_deref()) {
                if Path::new(&local_path).exists() {
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
                p4_print_file(&pf.old_path, cwd.as_deref(), config)?
            } else {
                reconstruct_old_text(pf)
            };
            let right_text = if pf.new_path == "/dev/null" {
                String::new()
            } else if pf.new_path.starts_with("//") {
                p4_print_file(&pf.new_path, cwd.as_deref(), config)?
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
            let resolved = Path::new(repo_path).join(display_path);
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

fn pending_local_path(pf: &PatchFileDiff, cwd: Option<&str>) -> Option<String> {
    if pf.new_path != "/dev/null" {
        let path = Path::new(&pf.new_path);
        if path.is_absolute() {
            return Some(path.to_string_lossy().into_owned());
        }
        if let Some(cwd) = cwd {
            return Some(Path::new(cwd).join(path).to_string_lossy().into_owned());
        }
    }

    if pf.old_path != "/dev/null" {
        let path = Path::new(&pf.old_path);
        if path.is_absolute() {
            return Some(path.to_string_lossy().into_owned());
        }
    }

    None
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
        let (status, content_left_json, content_right_json) =
            pending_no_diff_filediff_payload(file);
        DebugLogger::new("scm").log_diff_line_counts(
            &path,
            &content_left_json,
            &content_right_json,
        );
        store::insert_filediff(
            conn,
            &FileDiff {
                filediff_id: uuid::Uuid::new_v4().to_string(),
                diffset_id: diffset_id.to_string(),
                display_path: path,
                status,
                left_label: "have revision".to_string(),
                right_label: "workspace".to_string(),
                content_left_json,
                content_right_json,
                hunks_json: "[]".to_string(),
                write_target_json: file
                    .local_path
                    .as_ref()
                    .map(|path| WriteTarget::path(path).to_json_string())
                    .unwrap_or_else(|| WriteTarget::SaveAsRequired.to_json_string()),
                created_at: now,
            },
        )?;
    }

    Ok(())
}

fn pending_no_diff_filediff_payload(file: &P4OpenedFile) -> (String, String, String) {
    let empty_json = ContentSource::virtual_text("").to_json_string();
    let right_json = file
        .local_path
        .as_ref()
        .map(|path| ContentSource::path(path).to_json_string())
        .unwrap_or_else(|| empty_json.clone());

    match file.action.as_str() {
        "add" | "branch" | "move/add" => ("added".to_string(), empty_json, right_json),
        _ => (
            format!("{} no-diff", file.action),
            empty_json,
            ContentSource::virtual_text("").to_json_string(),
        ),
    }
}

fn populate_p4_local_paths(
    opened_files: &mut [P4OpenedFile],
    cwd: Option<&str>,
    p4_config: &P4Config,
) -> Result<(), String> {
    let depot_paths = opened_files
        .iter()
        .map(|file| file.depot_path.clone())
        .collect::<Vec<_>>();
    if depot_paths.is_empty() {
        return Ok(());
    }

    let local_paths = resolve_p4_local_paths(&depot_paths, cwd, p4_config)?;
    for file in opened_files.iter_mut() {
        file.local_path = local_paths.get(&file.depot_path).cloned();
    }
    Ok(())
}

fn resolve_p4_local_paths(
    depot_paths: &[String],
    cwd: Option<&str>,
    p4_config: &P4Config,
) -> Result<HashMap<String, String>, String> {
    let mut mapping = HashMap::new();
    for chunk in depot_paths.chunks(32) {
        let mut args = vec!["where".to_string()];
        args.extend(chunk.iter().cloned());
        let output = run_p4_owned(&args, cwd, p4_config)?;

        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('-') {
                continue;
            }

            let parts = trimmed.split_whitespace().collect::<Vec<_>>();
            if parts.len() >= 3 && parts[0].starts_with("//") {
                mapping.insert(parts[0].to_string(), parts[2].to_string());
            }
        }
    }

    Ok(mapping)
}

fn collect_pending_p4_diff(
    opened_files: &[P4OpenedFile],
    cwd: Option<&str>,
    p4_config: &P4Config,
) -> Result<String, String> {
    let mut sections = Vec::new();
    let debug = DebugLogger::new("scm");
    for (index, chunk) in opened_files.chunks(24).enumerate() {
        debug.log(format!(
            "collect_pending_p4_diff chunk={} files={} cwd={:?} client={:?}",
            index,
            chunk.len(),
            cwd,
            p4_config.client
        ));
        let mut args = vec!["diff".to_string(), "-du".to_string()];
        args.extend(chunk.iter().map(|file| file.depot_path.clone()));
        let output = run_p4_owned(&args, cwd, p4_config)?;
        debug.log(format!(
            "collect_pending_p4_diff chunk={} output_len={}",
            index,
            output.len()
        ));
        if !output.trim().is_empty() {
            sections.push(output);
        }
    }
    Ok(sections.join("\n"))
}

fn git_show_file(repo_path: &str, rel_path: &str) -> Result<String, String> {
    git_show_file_at_rev(repo_path, "HEAD", rel_path)
}

fn git_show_file_at_rev(repo_path: &str, rev: &str, rel_path: &str) -> Result<String, String> {
    run_command(
        "git",
        &["-C", repo_path, "show", &format!("{}:{}", rev, rel_path)],
        None,
    )
}

pub fn track_generated_p4_backup(backup_path: &Path, cwd: Option<&str>) -> Result<(), String> {
    let backup = backup_path.to_string_lossy().into_owned();
    let working_cwd = cwd
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            backup_path
                .parent()
                .map(|parent| parent.to_string_lossy().into_owned())
        });
    let p4_config = load_p4_config(working_cwd.as_deref());
    let debug = DebugLogger::new("scm");
    debug.log(format!(
        "track_generated_p4_backup path={} cwd={:?} config={:?}",
        backup, working_cwd, p4_config
    ));

    match run_p4_owned(
        &["add".to_string(), backup.clone()],
        working_cwd.as_deref(),
        &p4_config,
    ) {
        Ok(_) => Ok(()),
        Err(err)
            if err.contains("already opened for add")
                || err.contains("currently opened for add")
                || err.contains("can't add existing file") =>
        {
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn p4_print_file(path: &str, cwd: Option<&str>, p4_config: &P4Config) -> Result<String, String> {
    run_p4_owned(
        &["print".to_string(), "-q".to_string(), path.to_string()],
        cwd,
        p4_config,
    )
}

pub fn parse_p4_opened(output: &str) -> Vec<P4OpenedFile> {
    output
        .lines()
        .filter_map(|line| {
            let (left, right) = line.split_once(" - ")?;
            let depot_path = left.split('#').next().unwrap_or(left).trim().to_string();
            let action = right
                .split_whitespace()
                .next()
                .unwrap_or("edit")
                .to_string();
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
                local_path: None,
            })
        })
        .collect()
}

#[cfg(test)]
fn parse_p4_describe_actions(output: &str) -> HashMap<String, String> {
    describe_action_map(&parse_p4_describe_files(output))
}

fn parse_p4_describe_files(output: &str) -> Vec<P4DescribeFile> {
    let mut files = Vec::new();
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
        let (depot_path, rev) = split_p4_path_rev(path_with_rev);
        files.push(P4DescribeFile {
            depot_path,
            rev,
            action: action.to_string(),
        });
    }
    files
}

fn describe_action_map(files: &[P4DescribeFile]) -> HashMap<String, String> {
    files
        .iter()
        .map(|file| (file.depot_path.clone(), file.action.clone()))
        .collect()
}

fn split_p4_path_rev(path: &str) -> (String, Option<u32>) {
    if let Some((before, after)) = path.rsplit_once('#') {
        if let Ok(rev) = after.parse::<u32>() {
            return (before.to_string(), Some(rev));
        }
    }
    (path.to_string(), None)
}

fn normalize_p4_describe_files(
    parsed: Vec<PatchFileDiff>,
    described_files: &[P4DescribeFile],
    kind: &str,
    change: &str,
) -> Vec<PatchFileDiff> {
    let metadata_by_path = described_files
        .iter()
        .map(|file| (file.depot_path.clone(), file.clone()))
        .collect::<HashMap<_, _>>();
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();

    for mut pf in parsed {
        let display_path = describe_display_path(&pf);
        if let Some(file) = metadata_by_path.get(&display_path) {
            apply_describe_revision_info(&mut pf, file, kind, change);
            pf.status = describe_status(&file.action).to_string();
            seen.insert(file.depot_path.clone());
        }
        normalized.push(pf);
    }

    for file in described_files {
        if seen.contains(&file.depot_path) {
            continue;
        }
        normalized.push(synthetic_describe_patch_file(file, kind, change));
    }

    normalized
}

fn synthetic_describe_patch_file(file: &P4DescribeFile, kind: &str, change: &str) -> PatchFileDiff {
    let (old_path, new_path) = describe_content_paths(file, kind, change);
    PatchFileDiff {
        old_path,
        new_path,
        hunks: Vec::new(),
        status: describe_status(&file.action).to_string(),
    }
}

fn apply_describe_revision_info(
    pf: &mut PatchFileDiff,
    file: &P4DescribeFile,
    kind: &str,
    change: &str,
) {
    let (old_path, new_path) = describe_content_paths(file, kind, change);
    pf.old_path = old_path;
    pf.new_path = new_path;
}

fn describe_content_paths(file: &P4DescribeFile, kind: &str, change: &str) -> (String, String) {
    match kind {
        "p4Shelved" => match file.action.as_str() {
            "add" | "branch" | "move/add" => (
                "/dev/null".to_string(),
                format!("{}@={}", file.depot_path, change),
            ),
            "delete" | "move/delete" => (
                file_with_rev(file).unwrap_or_else(|| file.depot_path.clone()),
                "/dev/null".to_string(),
            ),
            _ => (
                file_with_rev(file).unwrap_or_else(|| file.depot_path.clone()),
                format!("{}@={}", file.depot_path, change),
            ),
        },
        _ => match file.action.as_str() {
            "add" | "branch" | "move/add" => (
                "/dev/null".to_string(),
                file_with_rev(file).unwrap_or_else(|| file.depot_path.clone()),
            ),
            "delete" | "move/delete" => (
                previous_rev_path(file).unwrap_or_else(|| file.depot_path.clone()),
                "/dev/null".to_string(),
            ),
            _ => (
                previous_rev_path(file).unwrap_or_else(|| file.depot_path.clone()),
                file_with_rev(file).unwrap_or_else(|| file.depot_path.clone()),
            ),
        },
    }
}

fn file_with_rev(file: &P4DescribeFile) -> Option<String> {
    file.rev.map(|rev| format!("{}#{}", file.depot_path, rev))
}

fn previous_rev_path(file: &P4DescribeFile) -> Option<String> {
    file.rev
        .and_then(|rev| rev.checked_sub(1))
        .map(|rev| format!("{}#{}", file.depot_path, rev))
}

fn describe_status(action: &str) -> &str {
    match action {
        "add" | "branch" | "move/add" => "added",
        "delete" | "move/delete" => "deleted",
        _ => "modified",
    }
}

fn describe_display_path(pf: &PatchFileDiff) -> String {
    if pf.old_path.starts_with("//") {
        strip_p4_rev(&pf.old_path)
    } else if pf.new_path.starts_with("//") {
        strip_p4_rev(&pf.new_path)
    } else {
        display_path_for_patch(pf)
    }
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

fn parse_p4_pending_change_line(raw_line: &str) -> Option<P4PendingChangeSummary> {
    let line = raw_line.trim();
    let change = line
        .strip_prefix("Change ")?
        .split_whitespace()
        .next()?
        .trim()
        .to_string();

    let user_client = line
        .split(" by ")
        .nth(1)?
        .split(" on ")
        .next()
        .or_else(|| line.split(" by ").nth(1))
        .unwrap_or_default();
    let (user, client) = user_client
        .split_once('@')
        .map(|(user, client)| {
            (
                Some(user.trim().to_string()),
                Some(client.trim().to_string()),
            )
        })
        .unwrap_or((None, None));

    let description = line
        .split('\'')
        .nth(1)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Pending changelist")
        .to_string();

    Some(P4PendingChangeSummary {
        change,
        description,
        client,
        user,
        is_default: false,
    })
}

fn opened_args<'a>(change: &'a str, p4_config: &P4Config) -> Vec<&'a str> {
    if change == "default" || p4_config.client.is_some() {
        vec!["opened", "-c", change]
    } else {
        vec!["opened", "-a", "-c", change]
    }
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
                    local_path: None,
                },
                P4OpenedFile {
                    depot_path: "//depot/main/b.cpp".to_string(),
                    action: "add".to_string(),
                    change: "12345".to_string(),
                    local_path: None,
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

    #[test]
    fn parses_p4_describe_file_metadata() {
        let files = parse_p4_describe_files(
            "Affected files ...\n\n... //depot/main/a.cpp#4 edit\n... //depot/main/b.cpp#1 add\n",
        );
        assert_eq!(
            files,
            vec![
                P4DescribeFile {
                    depot_path: "//depot/main/a.cpp".to_string(),
                    rev: Some(4),
                    action: "edit".to_string(),
                },
                P4DescribeFile {
                    depot_path: "//depot/main/b.cpp".to_string(),
                    rev: Some(1),
                    action: "add".to_string(),
                },
            ]
        );
    }

    #[test]
    fn normalizes_submitted_describe_paths_and_adds_missing_files() {
        let parsed = vec![PatchFileDiff {
            old_path: "//depot/main/a.cpp#4".to_string(),
            new_path: "//depot/main/a.cpp#4".to_string(),
            hunks: Vec::new(),
            status: "modified".to_string(),
        }];
        let described = vec![
            P4DescribeFile {
                depot_path: "//depot/main/a.cpp".to_string(),
                rev: Some(4),
                action: "edit".to_string(),
            },
            P4DescribeFile {
                depot_path: "//depot/main/b.cpp".to_string(),
                rev: Some(1),
                action: "add".to_string(),
            },
        ];

        let normalized = normalize_p4_describe_files(parsed, &described, "p4Submitted", "12345");
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].old_path, "//depot/main/a.cpp#3");
        assert_eq!(normalized[0].new_path, "//depot/main/a.cpp#4");
        assert_eq!(normalized[1].old_path, "/dev/null");
        assert_eq!(normalized[1].new_path, "//depot/main/b.cpp#1");
    }

    #[test]
    fn normalizes_shelved_describe_paths_for_edit_and_add() {
        let parsed = vec![PatchFileDiff {
            old_path: "//depot/main/a.cpp#4".to_string(),
            new_path: "//depot/main/a.cpp#4".to_string(),
            hunks: Vec::new(),
            status: "modified".to_string(),
        }];
        let described = vec![
            P4DescribeFile {
                depot_path: "//depot/main/a.cpp".to_string(),
                rev: Some(4),
                action: "edit".to_string(),
            },
            P4DescribeFile {
                depot_path: "//depot/main/b.cpp".to_string(),
                rev: Some(1),
                action: "add".to_string(),
            },
        ];

        let normalized = normalize_p4_describe_files(parsed, &described, "p4Shelved", "54321");
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].old_path, "//depot/main/a.cpp#4");
        assert_eq!(normalized[0].new_path, "//depot/main/a.cpp@=54321");
        assert_eq!(normalized[1].old_path, "/dev/null");
        assert_eq!(normalized[1].new_path, "//depot/main/b.cpp@=54321");
    }

    #[test]
    fn pending_added_file_without_unified_diff_uses_workspace_content() {
        let file = P4OpenedFile {
            depot_path: "//depot/main/new_file.cpp".to_string(),
            action: "add".to_string(),
            change: "default".to_string(),
            local_path: Some("C:\\work\\new_file.cpp".to_string()),
        };
        let (status, left_json, right_json) = pending_no_diff_filediff_payload(&file);
        assert_eq!(status, "added");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&left_json).unwrap(),
            serde_json::json!({ "type": "virtual", "text": "" })
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&right_json).unwrap(),
            serde_json::json!({ "type": "path", "path": "C:\\work\\new_file.cpp" })
        );
    }

    #[test]
    fn synthetic_git_added_file_diff_marks_file_as_added() {
        let diff = synthetic_git_added_file_diff("src/new_file.rs", "fn main() {}\n");
        assert_eq!(diff.old_path, "/dev/null");
        assert_eq!(diff.new_path, "src/new_file.rs");
        assert_eq!(diff.status, "added");
        assert_eq!(diff.hunks.len(), 1);
        assert!(diff.hunks[0].lines.iter().all(|line| line.kind == "add"));
    }

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
