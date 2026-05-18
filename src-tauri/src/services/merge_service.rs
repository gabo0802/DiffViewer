use rusqlite::{params, Connection};
use std::path::Path;

use crate::content_source::{ContentSource, WriteTarget};
use crate::debugging::DebugLogger;
use crate::diff_engine::{merge, unified_parser};
use crate::{io, scm, store};

pub fn init_mergebuffer(
    conn: &Connection,
    filediff_id: &str,
) -> Result<store::MergeBuffer, String> {
    let fd = store::get_filediff(conn, filediff_id)?;
    let right_text = ContentSource::resolve_json_text(&fd.content_right_json)?;
    let merged = merge::init_merge_buffer(&right_text);
    let debug = DebugLogger::new("merge");
    debug.log_diff_line_counts(
        &fd.display_path,
        &fd.content_left_json,
        &fd.content_right_json,
    );
    debug.log_merge_line_count(&fd.display_path, &merged, "init");

    let mb = store::MergeBuffer {
        filediff_id: filediff_id.to_string(),
        merged_content_json: ContentSource::virtual_text(merged).to_json_string(),
        dirty: false,
        updated_at: chrono::Utc::now().timestamp(),
    };
    store::upsert_merge_buffer(conn, &mb)?;
    Ok(mb)
}

pub fn apply_hunk_to_mergebuffer(
    conn: &Connection,
    filediff_id: &str,
    hunk_id: &str,
    source: &str,
) -> Result<store::MergeBuffer, String> {
    let fd = store::get_filediff(conn, filediff_id)?;
    let mb = store::get_merge_buffer(conn, filediff_id)?;
    let merged_text = ContentSource::resolve_json_text(&mb.merged_content_json)?;
    let hunks: Vec<unified_parser::Hunk> =
        serde_json::from_str(&fd.hunks_json).map_err(|err| err.to_string())?;
    let hunk = hunks
        .iter()
        .find(|hunk| hunk.id == hunk_id)
        .ok_or_else(|| format!("Hunk {} not found", hunk_id))?;
    let new_merged = merge::apply_hunk_to_buffer(&merged_text, hunk, source)?;
    DebugLogger::new("merge").log_merge_line_count(&fd.display_path, &new_merged, "apply_hunk");

    let updated = store::MergeBuffer {
        filediff_id: filediff_id.to_string(),
        merged_content_json: ContentSource::virtual_text(new_merged).to_json_string(),
        dirty: true,
        updated_at: chrono::Utc::now().timestamp(),
    };
    store::upsert_merge_buffer(conn, &updated)?;
    Ok(updated)
}

pub fn set_mergebuffer_text(
    conn: &Connection,
    filediff_id: &str,
    text: String,
) -> Result<store::MergeBuffer, String> {
    let fd = store::get_filediff(conn, filediff_id)?;
    DebugLogger::new("merge").log_merge_line_count(&fd.display_path, &text, "set_text");
    let mb = store::MergeBuffer {
        filediff_id: filediff_id.to_string(),
        merged_content_json: ContentSource::virtual_text(text).to_json_string(),
        dirty: true,
        updated_at: chrono::Utc::now().timestamp(),
    };
    store::upsert_merge_buffer(conn, &mb)?;
    Ok(mb)
}

pub fn save_mergebuffer(conn: &Connection, filediff_id: &str) -> Result<String, String> {
    let fd = store::get_filediff(conn, filediff_id)?;
    let mb = store::get_merge_buffer(conn, filediff_id)?;
    match WriteTarget::from_json(&fd.write_target_json)? {
        WriteTarget::Path { path } => save_mergebuffer_to_path(conn, fd, mb, path),
        WriteTarget::SaveAsRequired => {
            Err("Save As required - use save_mergebuffer_as with a target path".to_string())
        }
        WriteTarget::ReadOnly => Err("This diff is read-only".to_string()),
    }
}

pub fn save_mergebuffer_as(
    conn: &Connection,
    filediff_id: &str,
    path: &str,
) -> Result<String, String> {
    let fd = store::get_filediff(conn, filediff_id)?;
    let mb = store::get_merge_buffer(conn, filediff_id)?;
    let merged_text = ContentSource::resolve_json_text(&mb.merged_content_json)?;
    let resolved_path = WriteTarget::resolve_save_as_target_json(&fd.write_target_json, path)?;
    let debug = DebugLogger::new("merge");
    debug.log_merge_line_count(&fd.display_path, &merged_text, "save_as");
    debug.log(format!("save_as_target path={}", resolved_path));
    let diffset = store::get_diffset(conn, &fd.diffset_id)?;
    let backup_path = io::atomic_write(Path::new(&resolved_path), merged_text.as_bytes())?;
    if let Some(backup_path) = backup_path {
        maybe_track_pending_p4_backup(&diffset, &backup_path);
    }

    conn.execute(
        "UPDATE filediffs SET write_target_json = ?1 WHERE filediff_id = ?2",
        params![
            WriteTarget::path(&resolved_path).to_json_string(),
            filediff_id
        ],
    )
    .map_err(|err| err.to_string())?;
    mark_buffer_clean(conn, filediff_id, mb.merged_content_json)?;
    Ok("saved".to_string())
}

fn save_mergebuffer_to_path(
    conn: &Connection,
    fd: store::FileDiff,
    mb: store::MergeBuffer,
    target_path: String,
) -> Result<String, String> {
    let merged_text = ContentSource::resolve_json_text(&mb.merged_content_json)?;
    let debug = DebugLogger::new("merge");
    debug.log_merge_line_count(&fd.display_path, &merged_text, "save");
    debug.log(format!("save_target path={}", target_path));
    let diffset = store::get_diffset(conn, &fd.diffset_id)?;
    let target = Path::new(&target_path);
    let backup_path = if let Ok(existing) = io::read_file_text(target) {
        let eol = io::detect_eol(&existing);
        let normalized = merged_text.replace("\r\n", "\n").replace('\n', eol);
        io::atomic_write(target, normalized.as_bytes())?
    } else {
        io::atomic_write(target, merged_text.as_bytes())?
    };
    if let Some(backup_path) = backup_path {
        maybe_track_pending_p4_backup(&diffset, &backup_path);
    }

    mark_buffer_clean(conn, &fd.filediff_id, mb.merged_content_json)?;
    Ok("saved".to_string())
}

fn mark_buffer_clean(
    conn: &Connection,
    filediff_id: &str,
    merged_content_json: String,
) -> Result<(), String> {
    store::upsert_merge_buffer(
        conn,
        &store::MergeBuffer {
            filediff_id: filediff_id.to_string(),
            merged_content_json,
            dirty: false,
            updated_at: chrono::Utc::now().timestamp(),
        },
    )
}

fn maybe_track_pending_p4_backup(diffset: &store::DiffSet, backup_path: &Path) {
    if diffset.kind != "p4Pending" && diffset.kind != "p4PendingDefault" {
        return;
    }

    let cwd = parse_diffset_cwd(&diffset.source_meta_json);
    let debug = DebugLogger::new("merge");
    debug.log(format!(
        "tracking_pending_p4_backup path={} cwd={:?}",
        backup_path.display(),
        cwd
    ));

    if let Err(err) = scm::track_generated_p4_backup(backup_path, cwd.as_deref()) {
        debug.log(format!(
            "tracking_pending_p4_backup_failed path={} error={}",
            backup_path.display(),
            err
        ));
    }
}

fn parse_diffset_cwd(source_meta_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(source_meta_json)
        .ok()
        .and_then(|meta| {
            meta.get("cwd")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
}
