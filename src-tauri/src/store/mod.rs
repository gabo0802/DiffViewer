pub mod migrations;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Get the database path in the app data directory.
pub fn db_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("DiffViewer");
    std::fs::create_dir_all(&dir).ok();
    dir.join("diffviewer.db")
}

/// Open (or create) the SQLite database and run migrations.
pub fn open_db() -> Result<Connection, String> {
    let path = db_path();
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| e.to_string())?;
    migrations::run(&conn)?;
    Ok(conn)
}

// Data structs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub workspace_id: String,
    pub name: String,
    pub created_at: i64,
    pub last_opened_at: i64,
    pub settings_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSet {
    pub diffset_id: String,
    pub workspace_id: String,
    pub title: String,
    pub source_type: String,
    pub provider: String,
    pub kind: String,
    pub source_meta_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub filediff_id: String,
    pub diffset_id: String,
    pub display_path: String,
    pub status: String,
    pub left_label: String,
    pub right_label: String,
    pub content_left_json: String,
    pub content_right_json: String,
    pub hunks_json: String,
    pub write_target_json: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewState {
    pub filediff_id: String,
    pub reviewed: bool,
    pub last_view_mode: String,
    pub last_scroll_pos: f64,
    pub last_cursor_json: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub snapshot_id: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub cache_path: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeBuffer {
    pub filediff_id: String,
    pub merged_content_json: String,
    pub dirty: bool,
    pub updated_at: i64,
}

// CRUD operations

pub fn ensure_inbox(conn: &Connection) -> Result<Workspace, String> {
    let existing: Option<Workspace> = conn
        .query_row(
            "SELECT workspace_id, name, created_at, last_opened_at, settings_json FROM workspaces WHERE name = 'Inbox'",
            [],
            |row| {
                Ok(Workspace {
                    workspace_id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    last_opened_at: row.get(3)?,
                    settings_json: row.get(4)?,
                })
            },
        )
        .ok();

    if let Some(ws) = existing {
        return Ok(ws);
    }

    let now = chrono::Utc::now().timestamp();
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO workspaces (workspace_id, name, created_at, last_opened_at, settings_json) VALUES (?1,?2,?3,?4,?5)",
        params![id, "Inbox", now, now, "{}"],
    ).map_err(|e| e.to_string())?;

    Ok(Workspace {
        workspace_id: id,
        name: "Inbox".to_string(),
        created_at: now,
        last_opened_at: now,
        settings_json: "{}".to_string(),
    })
}

pub fn get_workspace(conn: &Connection, workspace_id: &str) -> Result<Workspace, String> {
    conn.query_row(
        "SELECT workspace_id, name, created_at, last_opened_at, settings_json FROM workspaces WHERE workspace_id = ?1",
        params![workspace_id],
        |row| {
            Ok(Workspace {
                workspace_id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                last_opened_at: row.get(3)?,
                settings_json: row.get(4)?,
            })
        },
    )
    .map_err(|err| err.to_string())
}

pub fn list_workspaces(conn: &Connection) -> Result<Vec<Workspace>, String> {
    let mut stmt = conn
        .prepare("SELECT workspace_id, name, created_at, last_opened_at, settings_json FROM workspaces ORDER BY last_opened_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Workspace {
                workspace_id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                last_opened_at: row.get(3)?,
                settings_json: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn create_workspace(conn: &Connection, name: &str) -> Result<Workspace, String> {
    let now = chrono::Utc::now().timestamp();
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO workspaces (workspace_id, name, created_at, last_opened_at, settings_json) VALUES (?1,?2,?3,?4,?5)",
        params![id, name, now, now, "{}"],
    ).map_err(|e| e.to_string())?;
    Ok(Workspace {
        workspace_id: id,
        name: name.to_string(),
        created_at: now,
        last_opened_at: now,
        settings_json: "{}".to_string(),
    })
}

pub fn update_workspace_settings(
    conn: &Connection,
    workspace_id: &str,
    settings_json: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE workspaces SET settings_json = ?2 WHERE workspace_id = ?1",
        params![workspace_id, settings_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_diffset(conn: &Connection, ds: &DiffSet) -> Result<(), String> {
    conn.execute(
        "INSERT INTO diffsets (diffset_id, workspace_id, title, source_type, provider, kind, source_meta_json, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![ds.diffset_id, ds.workspace_id, ds.title, ds.source_type, ds.provider, ds.kind, ds.source_meta_json, ds.created_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_diffsets(conn: &Connection, workspace_id: &str) -> Result<Vec<DiffSet>, String> {
    let mut stmt = conn
        .prepare("SELECT diffset_id, workspace_id, title, source_type, provider, kind, source_meta_json, created_at FROM diffsets WHERE workspace_id = ?1 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![workspace_id], |row| {
            Ok(DiffSet {
                diffset_id: row.get(0)?,
                workspace_id: row.get(1)?,
                title: row.get(2)?,
                source_type: row.get(3)?,
                provider: row.get(4)?,
                kind: row.get(5)?,
                source_meta_json: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_diffset(conn: &Connection, diffset_id: &str) -> Result<DiffSet, String> {
    conn.query_row(
        "SELECT diffset_id, workspace_id, title, source_type, provider, kind, source_meta_json, created_at FROM diffsets WHERE diffset_id = ?1",
        params![diffset_id],
        |row| {
            Ok(DiffSet {
                diffset_id: row.get(0)?,
                workspace_id: row.get(1)?,
                title: row.get(2)?,
                source_type: row.get(3)?,
                provider: row.get(4)?,
                kind: row.get(5)?,
                source_meta_json: row.get(6)?,
                created_at: row.get(7)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

pub fn update_diffset(conn: &Connection, ds: &DiffSet) -> Result<(), String> {
    conn.execute(
        "UPDATE diffsets SET title = ?2, source_type = ?3, provider = ?4, kind = ?5, source_meta_json = ?6 WHERE diffset_id = ?1",
        params![ds.diffset_id, ds.title, ds.source_type, ds.provider, ds.kind, ds.source_meta_json],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_filediff(conn: &Connection, fd: &FileDiff) -> Result<(), String> {
    conn.execute(
        "INSERT INTO filediffs (filediff_id, diffset_id, display_path, status, left_label, right_label, content_left_json, content_right_json, hunks_json, write_target_json, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![fd.filediff_id, fd.diffset_id, fd.display_path, fd.status, fd.left_label, fd.right_label, fd.content_left_json, fd.content_right_json, fd.hunks_json, fd.write_target_json, fd.created_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_filediffs_for_diffset(conn: &Connection, diffset_id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM merge_buffers WHERE filediff_id IN (SELECT filediff_id FROM filediffs WHERE diffset_id = ?1)",
        params![diffset_id],
    ).map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM review_state WHERE filediff_id IN (SELECT filediff_id FROM filediffs WHERE diffset_id = ?1)",
        params![diffset_id],
    ).map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM filediffs WHERE diffset_id = ?1",
        params![diffset_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_filediffs(conn: &Connection, diffset_id: &str) -> Result<Vec<FileDiff>, String> {
    let mut stmt = conn
        .prepare("SELECT filediff_id, diffset_id, display_path, status, left_label, right_label, content_left_json, content_right_json, hunks_json, write_target_json, created_at FROM filediffs WHERE diffset_id = ?1 ORDER BY display_path")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![diffset_id], |row| {
            Ok(FileDiff {
                filediff_id: row.get(0)?,
                diffset_id: row.get(1)?,
                display_path: row.get(2)?,
                status: row.get(3)?,
                left_label: row.get(4)?,
                right_label: row.get(5)?,
                content_left_json: row.get(6)?,
                content_right_json: row.get(7)?,
                hunks_json: row.get(8)?,
                write_target_json: row.get(9)?,
                created_at: row.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_filediff(conn: &Connection, filediff_id: &str) -> Result<FileDiff, String> {
    conn.query_row(
        "SELECT filediff_id, diffset_id, display_path, status, left_label, right_label, content_left_json, content_right_json, hunks_json, write_target_json, created_at FROM filediffs WHERE filediff_id = ?1",
        params![filediff_id],
        |row| {
            Ok(FileDiff {
                filediff_id: row.get(0)?,
                diffset_id: row.get(1)?,
                display_path: row.get(2)?,
                status: row.get(3)?,
                left_label: row.get(4)?,
                right_label: row.get(5)?,
                content_left_json: row.get(6)?,
                content_right_json: row.get(7)?,
                hunks_json: row.get(8)?,
                write_target_json: row.get(9)?,
                created_at: row.get(10)?,
            })
        },
    ).map_err(|e| e.to_string())
}

pub fn upsert_review_state(conn: &Connection, rs: &ReviewState) -> Result<(), String> {
    conn.execute(
        "INSERT INTO review_state (filediff_id, reviewed, last_view_mode, last_scroll_pos, last_cursor_json, updated_at) VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(filediff_id) DO UPDATE SET reviewed=excluded.reviewed, last_view_mode=excluded.last_view_mode, last_scroll_pos=excluded.last_scroll_pos, last_cursor_json=excluded.last_cursor_json, updated_at=excluded.updated_at",
        params![rs.filediff_id, rs.reviewed, rs.last_view_mode, rs.last_scroll_pos, rs.last_cursor_json, rs.updated_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn upsert_merge_buffer(conn: &Connection, mb: &MergeBuffer) -> Result<(), String> {
    conn.execute(
        "INSERT INTO merge_buffers (filediff_id, merged_content_json, dirty, updated_at) VALUES (?1,?2,?3,?4) ON CONFLICT(filediff_id) DO UPDATE SET merged_content_json=excluded.merged_content_json, dirty=excluded.dirty, updated_at=excluded.updated_at",
        params![mb.filediff_id, mb.merged_content_json, mb.dirty, mb.updated_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_merge_buffer(conn: &Connection, filediff_id: &str) -> Result<MergeBuffer, String> {
    conn.query_row(
        "SELECT filediff_id, merged_content_json, dirty, updated_at FROM merge_buffers WHERE filediff_id = ?1",
        params![filediff_id],
        |row| {
            Ok(MergeBuffer {
                filediff_id: row.get(0)?,
                merged_content_json: row.get(1)?,
                dirty: row.get(2)?,
                updated_at: row.get(3)?,
            })
        },
    ).map_err(|e| e.to_string())
}

pub fn insert_snapshot(conn: &Connection, snap: &Snapshot) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO snapshots (snapshot_id, sha256, size_bytes, cache_path, created_at) VALUES (?1,?2,?3,?4,?5)",
        params![snap.snapshot_id, snap.sha256, snap.size_bytes, snap.cache_path, snap.created_at],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_diffset(conn: &Connection, diffset_id: &str) -> Result<(), String> {
    delete_filediffs_for_diffset(conn, diffset_id)?;
    conn.execute(
        "DELETE FROM diffsets WHERE diffset_id = ?1",
        params![diffset_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
