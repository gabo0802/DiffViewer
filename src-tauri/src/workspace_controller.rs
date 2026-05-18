use rusqlite::Connection;
use std::path::PathBuf;

use crate::debugging::DebugLogger;
use crate::diff_engine::{twoway, unified_parser};
use crate::io;
use crate::store::{self, DiffSet, FileDiff, Snapshot};

/// Import a unified diff / patch file into the current workspace.
pub fn import_patch(
    conn: &Connection,
    workspace_id: &str,
    patch_path: &str,
) -> Result<String, String> {
    let patch_resolved = resolve_input_path(patch_path);
    let content = io::read_file_text(&patch_resolved)?;
    let parsed = unified_parser::parse_unified_diff(&content);

    let now = chrono::Utc::now().timestamp();
    let diffset_id = uuid::Uuid::new_v4().to_string();
    let title = patch_resolved
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Imported Patch".to_string());

    store::insert_diffset(
        conn,
        &DiffSet {
            diffset_id: diffset_id.clone(),
            workspace_id: workspace_id.to_string(),
            title,
            source_type: "Patch".to_string(),
            provider: "patch".to_string(),
            kind: "patchImport".to_string(),
            source_meta_json: serde_json::json!({ "path": patch_resolved.to_string_lossy() })
                .to_string(),
            created_at: now,
        },
    )?;

    for pf in &parsed {
        let filediff_id = uuid::Uuid::new_v4().to_string();
        let hunks_json = serde_json::to_string(&pf.hunks).unwrap_or_else(|_| "[]".to_string());

        // For patch imports: content sources are virtual (from patch text)
        let left_text = reconstruct_old_text(pf);
        let right_text = reconstruct_new_text(pf);
        let content_left_json =
            serde_json::json!({ "type": "virtual", "text": left_text }).to_string();
        let content_right_json =
            serde_json::json!({ "type": "virtual", "text": right_text }).to_string();
        let display_path = pf.new_path.clone();
        DebugLogger::new("workspace").log_diff_line_counts(
            &display_path,
            &content_left_json,
            &content_right_json,
        );

        store::insert_filediff(
            conn,
            &FileDiff {
                filediff_id,
                diffset_id: diffset_id.clone(),
                display_path,
                status: pf.status.clone(),
                left_label: pf.old_path.clone(),
                right_label: pf.new_path.clone(),
                content_left_json,
                content_right_json,
                hunks_json,
                write_target_json: serde_json::json!({ "type": "save_as_required" }).to_string(),
                created_at: now,
            },
        )?;
    }

    Ok(diffset_id)
}

/// Compare two files and create a DiffSet in the current workspace.
pub fn compare_two_files(
    conn: &Connection,
    workspace_id: &str,
    left_path: &str,
    right_path: &str,
    title: Option<&str>,
) -> Result<String, String> {
    let left_resolved = resolve_input_path(left_path);
    let right_resolved = resolve_input_path(right_path);
    let left_text = io::read_file_text(&left_resolved)?;
    let right_text = io::read_file_text(&right_resolved)?;

    // Snapshot both sides
    let (ls_id, ls_hash, ls_size, ls_cache) = io::snapshot_file(&left_resolved)?;
    let (rs_id, rs_hash, rs_size, rs_cache) = io::snapshot_file(&right_resolved)?;

    let now = chrono::Utc::now().timestamp();
    store::insert_snapshot(
        conn,
        &Snapshot {
            snapshot_id: ls_id.clone(),
            sha256: ls_hash,
            size_bytes: ls_size,
            cache_path: ls_cache,
            created_at: now,
        },
    )?;
    store::insert_snapshot(
        conn,
        &Snapshot {
            snapshot_id: rs_id.clone(),
            sha256: rs_hash,
            size_bytes: rs_size,
            cache_path: rs_cache,
            created_at: now,
        },
    )?;

    let hunks = twoway::compute_hunks(&left_text, &right_text);
    let hunks_json = serde_json::to_string(&hunks).unwrap_or_else(|_| "[]".to_string());

    let diffset_id = uuid::Uuid::new_v4().to_string();
    let display_title = title.unwrap_or("Two-way compare");

    store::insert_diffset(
        conn,
        &DiffSet {
            diffset_id: diffset_id.clone(),
            workspace_id: workspace_id.to_string(),
            title: display_title.to_string(),
            source_type: "External".to_string(),
            provider: "external".to_string(),
            kind: "twoWayCompare".to_string(),
            source_meta_json: serde_json::json!({
                "left": left_resolved.to_string_lossy(),
                "right": right_resolved.to_string_lossy()
            })
            .to_string(),
            created_at: now,
        },
    )?;

    let filediff_id = uuid::Uuid::new_v4().to_string();
    let left_name = left_resolved
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let right_name = right_resolved
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let content_left_json =
        serde_json::json!({ "type": "snapshot", "snapshot_id": ls_id }).to_string();
    let content_right_json =
        serde_json::json!({ "type": "snapshot", "snapshot_id": rs_id }).to_string();
    DebugLogger::new("workspace").log_diff_line_counts(
        &right_name,
        &content_left_json,
        &content_right_json,
    );

    store::insert_filediff(
        conn,
        &FileDiff {
            filediff_id,
            diffset_id: diffset_id.clone(),
            display_path: right_name.clone(),
            status: "modified".to_string(),
            left_label: left_name,
            right_label: right_name,
            content_left_json,
            content_right_json,
            hunks_json,
            write_target_json:
                serde_json::json!({ "type": "path", "path": right_resolved.to_string_lossy() })
                    .to_string(),
            created_at: now,
        },
    )?;

    Ok(diffset_id)
}

fn resolve_input_path(input: &str) -> PathBuf {
    let p = PathBuf::from(input);
    if p.is_absolute() {
        return p;
    }

    if let Ok(cwd) = std::env::current_dir() {
        let in_cwd = cwd.join(&p);
        if in_cwd.exists() {
            return in_cwd;
        }
        if let Some(parent) = cwd.parent() {
            let in_parent = parent.join(&p);
            if in_parent.exists() {
                return in_parent;
            }
        }
        return in_cwd;
    }

    p
}

// ── Helpers ──

fn reconstruct_old_text(pf: &unified_parser::PatchFileDiff) -> String {
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

fn reconstruct_new_text(pf: &unified_parser::PatchFileDiff) -> String {
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
