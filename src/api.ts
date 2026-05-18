import { invoke } from "@tauri-apps/api/core";
import type {
  Workspace,
  DiffSet,
  FileDiff,
  RenderedDiffModel,
  MergeBuffer,
} from "./types";

// Workspace commands

export const getCurrentWorkspace = () =>
  invoke<Workspace>("get_current_workspace");

export const listWorkspaces = () =>
  invoke<Workspace[]>("list_workspaces");

export const createWorkspace = (name: string) =>
  invoke<Workspace>("create_workspace", { name });

export const openWorkspace = (id: string) =>
  invoke<void>("open_workspace", { id });

// Diff creation

export const importPatch = (path: string) =>
  invoke<string>("import_patch", { path });

export const compareTwoFiles = (leftPath: string, rightPath: string) =>
  invoke<string>("compare_two_files", { leftPath, rightPath });

export const importGitWorkingTree = (repoPath: string) =>
  invoke<string>("import_git_working_tree", { repoPath });

export const importGitCommit = (repoPath: string, rev: string) =>
  invoke<string>("import_git_commit", { repoPath, rev });

export const importP4Pending = (change: string, cwd?: string) =>
  invoke<string>("import_p4_pending", { change, cwd });

export const importP4Shelved = (change: string, cwd?: string) =>
  invoke<string>("import_p4_shelved", { change, cwd });

export const importP4Submitted = (change: string, cwd?: string) =>
  invoke<string>("import_p4_submitted", { change, cwd });

// Diff access

export const listDiffsets = (workspaceId: string) =>
  invoke<DiffSet[]>("list_diffsets", { workspaceId });

export const listFilediffs = (diffsetId: string) =>
  invoke<FileDiff[]>("list_filediffs", { diffsetId });

export const refreshWorkspaceDiffsets = (workspaceId: string) =>
  invoke<DiffSet[]>("refresh_workspace_diffsets", { workspaceId });

export const deleteDiffset = (diffsetId: string) =>
  invoke<void>("delete_diffset", { diffsetId });

export const getRenderedDiff = (filediffId: string) =>
  invoke<RenderedDiffModel>("get_rendered_diff", { filediffId });

export const markReviewed = (filediffId: string, reviewed: boolean) =>
  invoke<void>("mark_reviewed", { filediffId, reviewed });

// Merge panel

export const initMergebuffer = (filediffId: string) =>
  invoke<MergeBuffer>("init_mergebuffer", { filediffId });

export const applyHunkToMergebuffer = (
  filediffId: string,
  hunkId: string,
  source: string
) =>
  invoke<MergeBuffer>("apply_hunk_to_mergebuffer", {
    filediffId,
    hunkId,
    source,
  });

export const setMergebufferText = (filediffId: string, text: string) =>
  invoke<MergeBuffer>("set_mergebuffer_text", { filediffId, text });

export const saveMergebuffer = (filediffId: string) =>
  invoke<string>("save_mergebuffer", { filediffId });

export const saveMergebufferAs = (filediffId: string, path: string) =>
  invoke<string>("save_mergebuffer_as", { filediffId, path });

export const handleOpenRequest = (requestJson: string) =>
  invoke<string>("handle_open_request", { requestJson });
