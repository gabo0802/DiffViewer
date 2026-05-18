// Mirrors the Rust data model

export interface Workspace {
  workspace_id: string;
  name: string;
  created_at: number;
  last_opened_at: number;
  settings_json: string;
}

export interface SavedWorkspaceLocation {
  id: string;
  path: string;
  label: string;
  createdAt: number;
  lastUsedAt: number;
}

export interface WorkspaceSettings {
  savedGitDirectories: SavedWorkspaceLocation[];
  savedP4Directories: SavedWorkspaceLocation[];
  selectedGitDirectoryId: string | null;
  selectedP4DirectoryId: string | null;
}

export interface P4PendingChangeSummary {
  change: string;
  description: string;
  client: string | null;
  user: string | null;
  isDefault: boolean;
}

export interface GitCommitSummary {
  rev: string;
  shortRev: string;
  subject: string;
  relativeTime: string;
}

export interface DiffSet {
  diffset_id: string;
  workspace_id: string;
  title: string;
  source_type: string;
  provider: "git" | "p4" | "patch" | "external" | string;
  kind: string;
  source_meta_json: string;
  created_at: number;
}

export interface FileDiff {
  filediff_id: string;
  diffset_id: string;
  display_path: string;
  status: string;
  left_label: string;
  right_label: string;
  content_left_json: string;
  content_right_json: string;
  hunks_json: string;
  write_target_json: string;
  created_at: number;
}

export interface AlignmentCell {
  line_no: number;
  text: string;
  kind: "context" | "del" | "add" | "empty";
}

export interface AlignmentRow {
  row_id: string;
  left?: AlignmentCell;
  right?: AlignmentCell;
  hunk_id?: string;
}

export interface RenderedHunkRef {
  hunk_id: string;
  start_row: number;
  end_row: number;
}

export interface RenderedDiffModel {
  filediff_id: string;
  rows: AlignmentRow[];
  hunks: RenderedHunkRef[];
}

export interface MergeBuffer {
  filediff_id: string;
  merged_content_json: string;
  dirty: boolean;
  updated_at: number;
}
