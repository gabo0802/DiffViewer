use rusqlite::Connection;

pub fn run(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS workspaces (
            workspace_id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            last_opened_at INTEGER NOT NULL,
            settings_json TEXT NOT NULL DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS diffsets (
            diffset_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            title TEXT NOT NULL,
            source_type TEXT NOT NULL,
            source_meta_json TEXT NOT NULL DEFAULT '{}',
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS filediffs (
            filediff_id TEXT PRIMARY KEY,
            diffset_id TEXT NOT NULL REFERENCES diffsets(diffset_id),
            display_path TEXT NOT NULL,
            status TEXT NOT NULL,
            left_label TEXT NOT NULL DEFAULT '',
            right_label TEXT NOT NULL DEFAULT '',
            content_left_json TEXT NOT NULL DEFAULT '{}',
            content_right_json TEXT NOT NULL DEFAULT '{}',
            hunks_json TEXT NOT NULL DEFAULT '[]',
            write_target_json TEXT NOT NULL DEFAULT '{}',
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS review_state (
            filediff_id TEXT PRIMARY KEY REFERENCES filediffs(filediff_id),
            reviewed INTEGER NOT NULL DEFAULT 0,
            last_view_mode TEXT NOT NULL DEFAULT 'sideBySide',
            last_scroll_pos REAL NOT NULL DEFAULT 0.0,
            last_cursor_json TEXT NOT NULL DEFAULT '{}',
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS snapshots (
            snapshot_id TEXT PRIMARY KEY,
            sha256 TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            cache_path TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS merge_buffers (
            filediff_id TEXT PRIMARY KEY REFERENCES filediffs(filediff_id),
            merged_content_json TEXT NOT NULL DEFAULT '{}',
            dirty INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_diffsets_workspace ON diffsets(workspace_id);
        CREATE INDEX IF NOT EXISTS idx_filediffs_diffset ON filediffs(diffset_id);
        "
    ).map_err(|e| e.to_string())
}
