use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use crate::scm::{import_unified_diff_text, display_path_for_patch, import_parsed_diff_text, replace_parsed_diffset_contents, DiffSetDescriptor, WriteTargetMode};
use crate::scm::process::run_command;
use crate::scm::pr_api;
use crate::diff_engine::{twoway, unified_parser::{self, PatchFileDiff}};
use crate::store::DiffSet;
use crate::providers::{ScmProvider, ImportTarget};
use crate::io;


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitSummary {
    pub rev: String,
    pub short_rev: String,
    pub subject: String,
    pub relative_time: String,
}

pub struct GitProvider;

impl ScmProvider for GitProvider {
    fn name(&self) -> &'static str {
        "git"
    }

    fn import_target(&self, conn: &Connection, workspace_id: &str, target: &ImportTarget) -> Result<String, String> {
        match target {
            ImportTarget::GitWorkingTree { repo_path } => {
                import_git_working_tree(conn, workspace_id, repo_path)
            }
            ImportTarget::GitCommit { repo_path, rev } => {
                import_git_commit(conn, workspace_id, repo_path, rev)
            }
            ImportTarget::GitPullRequest { repo_path, pr_id, target_branch, pr_title } => {
                import_git_pull_request(conn, workspace_id, repo_path, pr_id, target_branch, pr_title.as_deref())
            }
            _ => Err(format!("Unsupported target for GitProvider: {:?}", target)),
        }
    }

    fn replace_target(&self, conn: &Connection, diffset: &DiffSet, target: &ImportTarget) -> Result<(), String> {
        match target {
            ImportTarget::GitWorkingTree { repo_path } => {
                replace_git_working_tree(conn, diffset, repo_path)
            }
            _ => Err(format!("Unsupported replace target for GitProvider: {:?}", target)),
        }
    }
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
    let rev_prefix: String = rev.chars().take(5).collect();
    import_unified_diff_text(
        conn,
        workspace_id,
        &output,
        DiffSetDescriptor {
            title: format!("{} - '{}'", rev_prefix, subject),
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


pub fn replace_git_working_tree(
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


pub fn git_show_file(repo_path: &str, rel_path: &str) -> Result<String, String> {
    git_show_file_at_rev(repo_path, "HEAD", rel_path)
}


pub fn git_show_file_at_rev(repo_path: &str, rev: &str, rel_path: &str) -> Result<String, String> {
    run_command(
        "git",
        &["-C", repo_path, "show", &format!("{}:{}", rev, rel_path)],
        None,
    )
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