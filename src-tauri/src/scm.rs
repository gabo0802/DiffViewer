use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::debugging::DebugLogger;
use crate::diff_engine::unified_parser::{self, PatchFileDiff};
use crate::store::{self, DiffSet, FileDiff};

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

#[derive(Debug, Clone, Default)]
struct P4Config {
    client: Option<String>,
    port: Option<String>,
    user: Option<String>,
    charset: Option<String>,
    source_path: Option<PathBuf>,
}

pub fn import_git_working_tree(
    conn: &Connection,
    workspace_id: &str,
    repo_path: &str,
) -> Result<String, String> {
    let output = run_command(
        "git",
        &[
            "-C",
            repo_path,
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--unified=3",
        ],
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
            write_target_mode: WriteTargetMode::SaveAsRequired,
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
    let output = run_command(
        "git",
        &[
            "-C",
            repo_path,
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--unified=3",
        ],
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
    SaveAsRequired,
    GitWorkingTree {
        repo_path: String,
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

    for pf in parsed {
        insert_patch_file_diff(
            conn,
            &diffset_id,
            pf,
            &descriptor.left_label,
            &descriptor.right_label,
            &descriptor.write_target_mode,
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
    DebugLogger::new("scm").log(format!(
        "replace_diffset_contents diffset_id={} provider={} kind={} parsed_files={} diff_len={}",
        diffset.diffset_id,
        descriptor.provider,
        descriptor.kind,
        parsed.len(),
        diff_text.len()
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
    store::update_diffset(conn, &updated)?;
    store::delete_filediffs_for_diffset(conn, &diffset.diffset_id)?;

    for pf in &parsed {
        insert_patch_file_diff(
            conn,
            &diffset.diffset_id,
            pf,
            &left_label,
            &right_label,
            &write_target_mode,
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
            write_target_json: write_target.to_string(),
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
        WriteTargetMode::SaveAsRequired => Ok((
            serde_json::json!({ "type": "virtual", "text": reconstruct_old_text(pf) }).to_string(),
            serde_json::json!({ "type": "virtual", "text": reconstruct_new_text(pf) }).to_string(),
        )),
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
                serde_json::json!({ "type": "virtual", "text": reconstruct_new_text(pf) })
                    .to_string()
            } else {
                serde_json::json!({ "type": "path", "path": right_abs.to_string_lossy() })
                    .to_string()
            };
            Ok((
                serde_json::json!({ "type": "virtual", "text": left_text }).to_string(),
                right_json,
            ))
        }
        WriteTargetMode::P4Pending { cwd, config } => {
            let left_json = if pf.old_path == "/dev/null" {
                serde_json::json!({ "type": "virtual", "text": "" }).to_string()
            } else if pf.old_path.starts_with("//") {
                serde_json::json!({ "type": "virtual", "text": p4_print_file(&pf.old_path, cwd.as_deref(), config)? }).to_string()
            } else {
                serde_json::json!({ "type": "virtual", "text": reconstruct_old_text(pf) })
                    .to_string()
            };
            let right_json = if let Some(local_path) = pending_local_path(pf, cwd.as_deref()) {
                if Path::new(&local_path).exists() {
                    serde_json::json!({ "type": "path", "path": local_path }).to_string()
                } else {
                    serde_json::json!({ "type": "virtual", "text": reconstruct_new_text(pf) })
                        .to_string()
                }
            } else {
                serde_json::json!({ "type": "virtual", "text": reconstruct_new_text(pf) })
                    .to_string()
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
                serde_json::json!({ "type": "virtual", "text": left_text }).to_string(),
                serde_json::json!({ "type": "virtual", "text": right_text }).to_string(),
            ))
        }
    }
}

fn derive_write_target(
    mode: &WriteTargetMode,
    pf: &PatchFileDiff,
    display_path: &str,
) -> serde_json::Value {
    match mode {
        WriteTargetMode::SaveAsRequired => serde_json::json!({ "type": "save_as_required" }),
        WriteTargetMode::GitWorkingTree { repo_path } => {
            let resolved = Path::new(repo_path).join(display_path);
            serde_json::json!({ "type": "path", "path": resolved.to_string_lossy() })
        }
        WriteTargetMode::P4Pending { cwd, .. } => {
            if let Some(local_path) = pending_local_path(pf, cwd.as_deref()) {
                serde_json::json!({ "type": "path", "path": local_path })
            } else {
                serde_json::json!({ "type": "save_as_required" })
            }
        }
        WriteTargetMode::P4ReadOnly { .. } => serde_json::json!({ "type": "read_only" }),
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
                    .map(|path| serde_json::json!({ "type": "path", "path": path }).to_string())
                    .unwrap_or_else(|| {
                        serde_json::json!({ "type": "save_as_required" }).to_string()
                    }),
                created_at: now,
            },
        )?;
    }

    Ok(())
}

fn pending_no_diff_filediff_payload(file: &P4OpenedFile) -> (String, String, String) {
    let empty_json = serde_json::json!({ "type": "virtual", "text": "" }).to_string();
    let right_json = file
        .local_path
        .as_ref()
        .map(|path| serde_json::json!({ "type": "path", "path": path }).to_string())
        .unwrap_or_else(|| empty_json.clone());

    match file.action.as_str() {
        "add" | "branch" | "move/add" => ("added".to_string(), empty_json, right_json),
        _ => (
            format!("{} no-diff", file.action),
            empty_json,
            serde_json::json!({ "type": "virtual", "text": "" }).to_string(),
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
    run_command(
        "git",
        &["-C", repo_path, "show", &format!("HEAD:{}", rel_path)],
        None,
    )
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

pub fn parse_p4_describe_actions(output: &str) -> HashMap<String, String> {
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
    files.iter()
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

fn run_command(program: &str, args: &[&str], cwd: Option<&str>) -> Result<String, String> {
    let args = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    run_command_owned(program, &args, cwd, None)
}

fn run_p4(args: &[&str], cwd: Option<&str>, p4_config: &P4Config) -> Result<String, String> {
    let owned = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    run_p4_owned(&owned, cwd, p4_config)
}

fn run_p4_owned(
    args: &[String],
    cwd: Option<&str>,
    p4_config: &P4Config,
) -> Result<String, String> {
    let mut full_args = Vec::new();
    full_args.extend(args.iter().cloned());
    run_command_owned("p4", &full_args, cwd, Some(p4_config))
}

fn opened_args<'a>(change: &'a str, p4_config: &P4Config) -> Vec<&'a str> {
    if change == "default" || p4_config.client.is_some() {
        vec!["opened", "-c", change]
    } else {
        vec!["opened", "-a", "-c", change]
    }
}

fn load_p4_config(cwd: Option<&str>) -> P4Config {
    let Some(cwd) = cwd.filter(|value| !value.trim().is_empty()) else {
        return P4Config::default();
    };

    let mut current = PathBuf::from(cwd);
    loop {
        let candidate = current.join(".p4config");
        if candidate.is_file() {
            let mut config = P4Config {
                source_path: Some(candidate.clone()),
                ..P4Config::default()
            };
            if let Ok(contents) = std::fs::read_to_string(&candidate) {
                for raw_line in contents.lines() {
                    let line = raw_line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    let Some((key, value)) = line.split_once('=') else {
                        continue;
                    };
                    let value = value.trim().to_string();
                    match key.trim() {
                        "P4CLIENT" => config.client = Some(value),
                        "P4PORT" => config.port = Some(value),
                        "P4USER" => config.user = Some(value),
                        "P4CHARSET" => config.charset = Some(value),
                        _ => {}
                    }
                }
            }
            return config;
        }
        if !current.pop() {
            break;
        }
    }

    P4Config::default()
}

fn apply_p4_config_env(cmd: &mut Command, p4_config: &P4Config) {
    if let Some(client) = p4_config
        .client
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        cmd.env("P4CLIENT", client);
    }
    if let Some(port) = p4_config
        .port
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        cmd.env("P4PORT", port);
    }
    if let Some(user) = p4_config
        .user
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        cmd.env("P4USER", user);
    }
    let charset = p4_config
        .charset
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("utf8");
    cmd.env("P4CHARSET", charset);
}

fn run_command_owned(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    p4_config: Option<&P4Config>,
) -> Result<String, String> {
    let debug = DebugLogger::new("scm");
    if program == "p4" {
        debug.log(format!(
            "command={} cwd={:?} args={} client={:?} port={:?} user={:?} config_path={:?}",
            program,
            cwd,
            args.join(" "),
            p4_config.and_then(|config| config.client.as_deref()),
            p4_config.and_then(|config| config.port.as_deref()),
            p4_config.and_then(|config| config.user.as_deref()),
            p4_config.and_then(|config| config.source_path.as_ref())
        ));
    }
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(cwd) = cwd.filter(|value| !value.trim().is_empty()) {
        cmd.current_dir(cwd);
    }
    if let Some(p4_config) = p4_config {
        apply_p4_config_env(&mut cmd, p4_config);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run {}: {}", program, e))?;
    if program == "p4" {
        debug.log(format!("status={}", output.status));
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug.log_multiline("stdout", &stdout);
        debug.log_multiline("stderr", &stderr);
    }
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
            left_json,
            serde_json::json!({ "type": "virtual", "text": "" }).to_string()
        );
        assert_eq!(
            right_json,
            serde_json::json!({ "type": "path", "path": "C:\\work\\new_file.cpp" }).to_string()
        );
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
