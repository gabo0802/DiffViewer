import { invoke } from "@tauri-apps/api/core";
import type {
  Workspace,
  WorkspaceSettings,
  P4PendingChangeSummary,
  GitCommitSummary,
  GitStashSummary,
  PullRequestSummary,
  DiffSet,
  FileDiff,
  RenderedDiffModel,
  MergeBuffer,
} from "./types";

// Workspace commands

export const getCurrentWorkspace = () =>
  invoke<Workspace>("get_current_workspace");

export const getCurrentWorkspaceSettings = () =>
  invoke<WorkspaceSettings>("get_current_workspace_settings");

export const listWorkspaces = () =>
  invoke<Workspace[]>("list_workspaces");

export const createWorkspace = (name: string) =>
  invoke<Workspace>("create_workspace", { name });

export const openWorkspace = (id: string) =>
  invoke<void>("open_workspace", { id });

export const saveCurrentWorkspaceLocation = (provider: "git" | "p4", path: string) =>
  invoke<WorkspaceSettings>("save_current_workspace_location", { provider, path });

export const selectCurrentWorkspaceLocation = (
  provider: "git" | "p4",
  locationId: string | null
) =>
  invoke<WorkspaceSettings>("select_current_workspace_location", {
    provider,
    locationId,
  });

export const removeCurrentWorkspaceLocation = (
  provider: "git" | "p4",
  locationId: string
) =>
  invoke<WorkspaceSettings>("remove_current_workspace_location", {
    provider,
    locationId,
  });

export const updateScmSettings = (
  githubPat?: string,
  gitlabPat?: string,
  gitlabHostUrl?: string
) =>
  invoke<WorkspaceSettings>("update_scm_settings", {
    githubPat,
    gitlabPat,
    gitlabHostUrl,
  });

export const browseForDirectory = (initialPath?: string) =>
  invoke<string | null>("browse_for_directory", { initialPath });

// Diff creation

export const importPatch = (path: string) =>
  invoke<string>("import_patch", { path });

export const compareTwoFiles = (leftPath: string, rightPath: string) =>
  invoke<string>("compare_two_files", { leftPath, rightPath });

export const importGitWorkingTree = (repoPath: string) =>
  invoke<string>("import_git_working_tree", { repoPath });

export const importGitCommit = (repoPath: string, rev: string) =>
  invoke<string>("import_git_commit", { repoPath, rev });

export const importGitStash = (repoPath: string, stashId: string) =>
  invoke<string>("import_git_stash", { repoPath, stashId });

export const listGitStashes = (repoPath: string) =>
  invoke<GitStashSummary[]>("list_git_stashes", { repoPath });

export const popGitStash = (repoPath: string, stashId: string) =>
  invoke<void>("pop_git_stash", { repoPath, stashId });

export const applyGitStash = (repoPath: string, stashId: string) =>
  invoke<void>("apply_git_stash", { repoPath, stashId });

export const importP4Pending = (change: string, cwd?: string) =>
  invoke<string>("import_p4_pending", { change, cwd });

export const importP4Shelved = (change: string, cwd?: string) =>
  invoke<string>("import_p4_shelved", { change, cwd });

export const importP4Submitted = (change: string, cwd?: string) =>
  invoke<string>("import_p4_submitted", { change, cwd });

export const listGitCommits = (repoPath: string, limit = 30, branch: string | null = null) =>
  invoke<GitCommitSummary[]>("list_git_commits", { repoPath, limit, branch });

export const listGitBranches = (repoPath: string) =>
  invoke<string[]>("list_git_branches", { repoPath });

export const getPullRequests = (repoPath: string) =>
  invoke<PullRequestSummary[]>("get_pull_requests", { repoPath });

export const importGitPullRequest = (repoPath: string, prId: string, targetBranch: string, prTitle?: string) =>
  invoke<string>("import_git_pull_request", { repoPath, prId, targetBranch, prTitle });

export const listP4PendingChanges = (cwd?: string) =>
  invoke<P4PendingChangeSummary[]>("list_p4_pending_changes", { cwd });

// Diff access

export const listDiffsets = (workspaceId: string) =>
  invoke<DiffSet[]>("list_diffsets", { workspaceId });

export const listFilediffs = (diffsetId: string) =>
  invoke<FileDiff[]>("list_filediffs", { diffsetId });

export const refreshWorkspaceDiffsets = (workspaceId: string) =>
  invoke<DiffSet[]>("refresh_workspace_diffsets", { workspaceId });

export const refreshDiffset = (diffsetId: string) =>
  invoke<boolean>("refresh_diffset", { diffsetId });

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
