use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use crate::scm::{import_unified_diff_text, replace_diffset_contents, import_parsed_diff_text, DiffSetDescriptor, WriteTargetMode, display_path_for_patch};
use crate::scm::p4_config::{load_p4_config, P4Config};
use crate::scm::process::{run_p4, run_p4_owned};
use crate::diff_engine::unified_parser::{self, PatchFileDiff};
use crate::debugging::DebugLogger;
use crate::store::{self, DiffSet, FileDiff};
use crate::content_source::{ContentSource, WriteTarget};
use crate::providers::{ScmProvider, ImportTarget};


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

pub struct P4Provider;

impl ScmProvider for P4Provider {
    fn name(&self) -> &'static str {
        "p4"
    }

    fn import_target(&self, conn: &Connection, workspace_id: &str, target: &ImportTarget) -> Result<String, String> {
        match target {
            ImportTarget::P4Pending { change, cwd } => {
                import_p4_pending(conn, workspace_id, change, cwd.as_deref())
            }
            ImportTarget::P4Shelved { change, cwd } => {
                import_p4_shelved(conn, workspace_id, change, cwd.as_deref())
            }
            ImportTarget::P4Submitted { change, cwd } => {
                import_p4_submitted(conn, workspace_id, change, cwd.as_deref())
            }
            _ => Err(format!("Unsupported target for P4Provider: {:?}", target)),
        }
    }

    fn replace_target(&self, conn: &Connection, diffset: &DiffSet, target: &ImportTarget) -> Result<(), String> {
        match target {
            ImportTarget::P4Pending { change, cwd } => {
                replace_p4_pending(conn, diffset, change, cwd.as_deref())
            }
            _ => Err(format!("Unsupported replace target for P4Provider: {:?}", target)),
        }
    }
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
        "default - 'Default pending changelist'".to_string()
    } else {
        let desc = get_p4_pending_change_description(change, cwd, &p4_config)
            .unwrap_or_else(|| "Pending changelist".to_string());
        format!("{} - '{}'", change, desc)
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
        "Shelved changelist",
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
        "Submitted changelist",
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
    let title = if let Some(line) = desc {
        format!("{} - '{}'", change, line)
    } else {
        format!("{} - '{}'", change, fallback_title)
    };
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


pub fn replace_p4_pending(
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
        "default - 'Default pending changelist'".to_string()
    } else {
        let desc = get_p4_pending_change_description(change, cwd, &p4_config)
            .unwrap_or_else(|| "Pending changelist".to_string());
        format!("{} - '{}'", change, desc)
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


pub fn pending_local_path(pf: &PatchFileDiff, cwd: Option<&str>) -> Option<String> {
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


pub fn p4_print_file(path: &str, cwd: Option<&str>, p4_config: &P4Config) -> Result<String, String> {
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
            if trimmed == "Affected files ..." || trimmed == "Files:" {
                return None;
            }
            return Some(trimmed.to_string());
        }
    }
    None
}


fn get_p4_pending_change_description(change: &str, cwd: Option<&str>, p4_config: &P4Config) -> Option<String> {
    let output = run_p4(&["change", "-o", change], cwd, p4_config).ok()?;
    first_p4_description_line(&output)
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


pub fn strip_p4_rev(path: &str) -> String {
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