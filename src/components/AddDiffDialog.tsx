import React, { useEffect, useMemo, useState } from "react";
import type {
  GitCommitSummary,
  GitStashSummary,
  P4PendingChangeSummary,
  SavedWorkspaceLocation,
} from "../types";
import { LoadingIcon } from "./Icons";

export type AddDiffImportKind =
  | "gitWorking"
  | "gitCommit"
  | "gitPullRequest"
  | "gitStash"
  | "p4Pending"
  | "p4Shelved"
  | "p4Submitted";

export interface AddDiffRequest {
  kind: AddDiffImportKind;
  repoPath?: string;
  cwd?: string;
  rev?: string;
  change?: string;
  prId?: string;
  targetBranch?: string;
  prTitle?: string;
  stashId?: string;
}

interface Props {
  visible: boolean;
  importing: boolean;
  gitLocations: SavedWorkspaceLocation[];
  p4Locations: SavedWorkspaceLocation[];
  selectedGitLocationId: string | null;
  selectedP4LocationId: string | null;
  onCancel: () => void;
  onSubmit: (request: AddDiffRequest) => Promise<void> | void;
  onSelectLocation: (provider: ProviderMode, locationId: string | null) => Promise<void> | void;
  loadGitCommits: (repoPath: string, branch: string | null) => Promise<GitCommitSummary[]>;
  loadP4PendingChanges: (cwd: string) => Promise<P4PendingChangeSummary[]>;
  loadGitBranches: (repoPath: string) => Promise<string[]>;
  loadPullRequests: (repoPath: string) => Promise<import("../types").PullRequestSummary[]>;
  loadGitStashes: (repoPath: string) => Promise<GitStashSummary[]>;
}

type ProviderMode = "git" | "p4";
type LookupState = "idle" | "loading" | "ready" | "error";

export default function AddDiffDialog({
  visible,
  importing,
  gitLocations,
  p4Locations,
  selectedGitLocationId,
  selectedP4LocationId,
  onCancel,
  onSubmit,
  onSelectLocation,
  loadGitCommits,
  loadP4PendingChanges,
  loadGitBranches,
  loadPullRequests,
  loadGitStashes,
}: Props) {
  const defaultProvider = useMemo<ProviderMode>(() => {
    if (selectedP4LocationId) return "p4";
    return "git";
  }, [selectedP4LocationId]);
  const [provider, setProvider] = useState<ProviderMode>(defaultProvider);
  const [gitMode, setGitMode] = useState<"gitWorking" | "gitCommit" | "gitPullRequest" | "gitStash">("gitWorking");
  const [p4Mode, setP4Mode] = useState<"p4Pending" | "p4Submitted" | "p4Shelved">("p4Pending");
  const [change, setChange] = useState("default");
  const [rev, setRev] = useState("HEAD");
  const [gitBranch, setGitBranch] = useState<string | null>(null);
  const [gitBranches, setGitBranches] = useState<string[]>([]);
  const [gitCommits, setGitCommits] = useState<GitCommitSummary[]>([]);
  const [prId, setPrId] = useState("");
  const [targetBranch, setTargetBranch] = useState("main");
  const [pullRequests, setPullRequests] = useState<import("../types").PullRequestSummary[]>([]);
  const [stashId, setStashId] = useState("");
  const [gitStashes, setGitStashes] = useState<GitStashSummary[]>([]);
  const [pendingChanges, setPendingChanges] = useState<P4PendingChangeSummary[]>([]);
  const [lookupState, setLookupState] = useState<LookupState>("idle");
  const [lookupError, setLookupError] = useState<string | null>(null);

  useEffect(() => {
    if (!visible) return;
    setProvider(defaultProvider);
    setGitMode("gitWorking");
    setP4Mode("p4Pending");
    setChange("default");
    setRev("HEAD");
    setGitBranch(null);
    setGitBranches([]);
    setGitCommits([]);
    setPrId("");
    setTargetBranch("main");
    setPullRequests([]);
    setStashId("");
    setGitStashes([]);
    setPendingChanges([]);
    setLookupState("idle");
    setLookupError(null);
  }, [defaultProvider, visible]);

  const selectedGitLocation = useMemo(
    () => gitLocations.find((location) => location.id === selectedGitLocationId) ?? null,
    [gitLocations, selectedGitLocationId]
  );
  const selectedP4Location = useMemo(
    () => p4Locations.find((location) => location.id === selectedP4LocationId) ?? null,
    [p4Locations, selectedP4LocationId]
  );
  const activeLocation = provider === "git" ? selectedGitLocation : selectedP4Location;
  const activePath = activeLocation?.path ?? null;

  useEffect(() => {
    if (!visible) return;
    let cancelled = false;

    const run = async () => {
      try {
        setLookupError(null);
        if (provider === "p4" && p4Mode === "p4Pending" && activePath) {
          setLookupState("loading");
          const nextChanges = await loadP4PendingChanges(activePath);
          if (cancelled) return;
          setPendingChanges(nextChanges);
          setChange(nextChanges[0]?.change ?? "default");
          setLookupState("ready");
          return;
        }

        if (provider === "git" && gitMode === "gitCommit" && activePath) {
          setLookupState("loading");
          let branches = gitBranches;
          if (branches.length === 0) {
            branches = await loadGitBranches(activePath);
            if (cancelled) return;
            setGitBranches(branches);
          }
          const nextCommits = await loadGitCommits(activePath, gitBranch);
          if (cancelled) return;
          setGitCommits(nextCommits);
          if (!nextCommits.find(c => c.rev === rev)) {
            setRev(nextCommits[0]?.rev ?? "HEAD");
          }
          setLookupState("ready");
          return;
        }

        if (provider === "git" && gitMode === "gitPullRequest" && activePath) {
          setLookupState("loading");
          let branches = gitBranches;
          if (branches.length === 0) {
            branches = await loadGitBranches(activePath);
            if (cancelled) return;
            setGitBranches(branches);
          }
          const prs = await loadPullRequests(activePath);
          if (cancelled) return;
          setPullRequests(prs);
          if (prs.length > 0) {
            setPrId(prs[0].id);
            setTargetBranch(prs[0].targetBranch);
          }
          setLookupState("ready");
          return;
        }

        if (provider === "git" && gitMode === "gitStash" && activePath) {
          setLookupState("loading");
          const stashes = await loadGitStashes(activePath);
          if (cancelled) return;
          setGitStashes(stashes);
          if (stashes.length > 0) {
            setStashId(stashes[0].stashId);
          }
          setLookupState("ready");
          return;
        }

        setLookupState("idle");
      } catch (error) {
        if (cancelled) return;
        setLookupError(String(error));
        setLookupState("error");
      }
    };

    run();
    return () => {
      cancelled = true;
    };
  }, [
    activePath,
    gitMode,
    gitBranch,
    loadGitBranches,
    loadGitCommits,
    loadPullRequests,
    loadGitStashes,
    loadP4PendingChanges,
    p4Mode,
    provider,
    visible,
  ]);

  if (!visible) return null;

  const activeKind = provider === "git" ? gitMode : p4Mode;
  const pathLabel = provider === "git" ? "Current Git Directory" : "Current P4 Depot";
  const canSubmit =
    !!activePath &&
    !importing &&
    (activeKind === "gitWorking" ||
      (activeKind === "gitCommit" && !!rev.trim()) ||
      (activeKind === "gitPullRequest" && !!prId.trim() && !!targetBranch.trim()) ||
      (activeKind === "gitStash" && !!stashId.trim()) ||
      ((activeKind === "p4Pending" || activeKind === "p4Submitted" || activeKind === "p4Shelved") && !!change.trim()));

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!activePath || !canSubmit) return;

    if (activeKind === "gitWorking") {
      await onSubmit({ kind: activeKind, repoPath: activePath });
      return;
    }
    if (activeKind === "gitCommit") {
      await onSubmit({ kind: activeKind, repoPath: activePath, rev: rev.trim() });
      return;
    }
    if (activeKind === "gitPullRequest") {
      const prTitle = pullRequests.find((pr) => pr.id === prId.trim())?.title;
      await onSubmit({ kind: activeKind, repoPath: activePath, prId: prId.trim(), targetBranch: targetBranch.trim(), prTitle });
      return;
    }
    if (activeKind === "gitStash") {
      await onSubmit({ kind: activeKind, repoPath: activePath, stashId: stashId.trim() });
      return;
    }

    await onSubmit({ kind: activeKind, cwd: activePath, change: change.trim() });
  };

  return (
    <div className="dialog-backdrop" onClick={importing ? undefined : onCancel}>
      <form
        className="dialog-card add-diff-dialog"
        onClick={(event) => event.stopPropagation()}
        onSubmit={handleSubmit}
      >
        <div className="dialog-header">
          <div className="dialog-title">Add Diff</div>
          <div className="dialog-description">
            Pick the source, then open a pending changelist, submitted change, shelved change,
            working tree, or commit.
          </div>
        </div>

        <div className="dialog-segment">
          <button
            type="button"
            className={`dialog-segment-button ${provider === "p4" ? "dialog-segment-button-active" : ""}`}
            onClick={() => setProvider("p4")}
          >
            Perforce
          </button>
          <button
            type="button"
            className={`dialog-segment-button ${provider === "git" ? "dialog-segment-button-active" : ""}`}
            onClick={() => setProvider("git")}
          >
            Git
          </button>
        </div>

        {provider === "p4" ? (
          <div className="dialog-stack">
            <div className="dialog-mode-list">
              <ModeButton
                active={p4Mode === "p4Pending"}
                label="Pending"
                onClick={() => setP4Mode("p4Pending")}
              />
              <ModeButton
                active={p4Mode === "p4Submitted"}
                label="Submitted"
                onClick={() => setP4Mode("p4Submitted")}
              />
              <ModeButton
                active={p4Mode === "p4Shelved"}
                label="Shelved"
                onClick={() => setP4Mode("p4Shelved")}
              />
            </div>
            <SavedLocationPicker
              label={pathLabel}
              locations={p4Locations}
              selectedLocationId={selectedP4LocationId}
              onChange={(locationId) => onSelectLocation("p4", locationId)}
            />
            <PathSummary label={pathLabel} path={activePath} />
            {p4Mode === "p4Pending" ? (
              <label className="dialog-field">
                <span>Pending changelist</span>
                <select
                  value={change}
                  onChange={(event) => setChange(event.target.value)}
                  disabled={!activePath || lookupState === "loading"}
                >
                  {pendingChanges.map((item) => (
                    <option key={item.change} value={item.change}>
                      {item.isDefault
                        ? "default"
                        : `${item.description}`}
                    </option>
                  ))}
                  {pendingChanges.length === 0 && <option value="default">default</option>}
                </select>
                <LookupStatus
                  activePath={activePath}
                  lookupError={lookupError}
                  lookupState={lookupState}
                  loadingLabel="Loading pending changelists..."
                  emptyLabel="No pending changelists found for this workspace."
                  hasResults={pendingChanges.length > 0}
                />
              </label>
            ) : (
              <label className="dialog-field">
                <span>CL number</span>
                <input
                  value={change}
                  placeholder="12345"
                  onChange={(event) => setChange(event.target.value)}
                />
              </label>
            )}
          </div>
        ) : (
          <div className="dialog-stack">
            <div className="dialog-mode-list">
              <ModeButton
                active={gitMode === "gitWorking"}
                label="Working Tree"
                onClick={() => setGitMode("gitWorking")}
              />
              <ModeButton
                active={gitMode === "gitCommit"}
                label="Commit"
                onClick={() => setGitMode("gitCommit")}
              />
              <ModeButton
                active={gitMode === "gitPullRequest"}
                label="Pull Request"
                onClick={() => setGitMode("gitPullRequest")}
              />
              <ModeButton
                active={gitMode === "gitStash"}
                label="Stash"
                onClick={() => setGitMode("gitStash")}
              />
            </div>
            <SavedLocationPicker
              label={pathLabel}
              locations={gitLocations}
              selectedLocationId={selectedGitLocationId}
              onChange={(locationId) => onSelectLocation("git", locationId)}
            />
            <PathSummary label={pathLabel} path={activePath} />
            {gitMode === "gitCommit" && (
              <>
                <label className="dialog-field">
                  <span>Branch (Optional)</span>
                  <select
                    value={gitBranch ?? ""}
                    onChange={(event) => setGitBranch(event.target.value || null)}
                    disabled={!activePath || lookupState === "loading"}
                  >
                    <option value="">All branches</option>
                    {gitBranches.map((branch) => (
                      <option key={branch} value={branch}>
                        {branch}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="dialog-field">
                  <span>Recent commits</span>
                  <select
                    value={rev}
                    onChange={(event) => setRev(event.target.value)}
                    disabled={!activePath || lookupState === "loading"}
                  >
                    {gitCommits.map((commit) => (
                      <option key={commit.rev} value={commit.rev}>
                        {commit.shortRev} - {commit.subject}
                      </option>
                    ))}
                    {gitCommits.length === 0 && <option value="HEAD">HEAD</option>}
                  </select>
                  <LookupStatus
                    activePath={activePath}
                    lookupError={lookupError}
                    lookupState={lookupState}
                    loadingLabel="Loading recent commits..."
                    emptyLabel="No commits were returned for this repository."
                    hasResults={gitCommits.length > 0}
                  />
                </label>
                <label className="dialog-field">
                  <span>Selected commit</span>
                  <div className="dialog-static-value" title={rev}>
                    {gitCommits.find((commit) => commit.rev === rev)?.shortRev ?? rev}
                  </div>
                </label>
              </>
            )}
            {gitMode === "gitPullRequest" && (
              <>
                <label className="dialog-field">
                  <span>Pull Requests</span>
                  <select
                    value={prId}
                    onChange={(event) => {
                      const id = event.target.value;
                      setPrId(id);
                      const pr = pullRequests.find(p => p.id === id);
                      if (pr) setTargetBranch(pr.targetBranch);
                    }}
                    disabled={!activePath || lookupState === "loading"}
                  >
                    {pullRequests.map((pr) => (
                      <option key={pr.id} value={pr.id}>
                        #{pr.id} - {pr.title} ({pr.state}) by {pr.author}
                      </option>
                    ))}
                  </select>
                  <LookupStatus
                    activePath={activePath}
                    lookupError={lookupError}
                    lookupState={lookupState}
                    loadingLabel="Fetching PRs from GitHub/GitLab..."
                    emptyLabel="No PRs found. Did you set your PAT in Settings?"
                    hasResults={pullRequests.length > 0}
                  />
                </label>
                <label className="dialog-field">
                  <span>Target branch</span>
                  <select
                    value={targetBranch}
                    onChange={(event) => setTargetBranch(event.target.value)}
                    disabled={!activePath || lookupState === "loading"}
                  >
                    {gitBranches.map((branch) => (
                      <option key={branch} value={branch}>
                        {branch}
                      </option>
                    ))}
                    {/* Fallback if targetBranch is not in the list but was set by PR */}
                    {!gitBranches.includes(targetBranch) && targetBranch && (
                      <option value={targetBranch}>{targetBranch}</option>
                    )}
                  </select>
                </label>
              </>
            )}
            {gitMode === "gitStash" && (
              <>
                <label className="dialog-field">
                  <span>Git Stashes</span>
                  <select
                    value={stashId}
                    onChange={(event) => setStashId(event.target.value)}
                    disabled={!activePath || lookupState === "loading"}
                  >
                    {gitStashes.map((stash) => (
                      <option key={stash.stashId} value={stash.stashId}>
                        {stash.message}
                      </option>
                    ))}
                  </select>
                  <LookupStatus
                    activePath={activePath}
                    lookupError={lookupError}
                    lookupState={lookupState}
                    loadingLabel="Loading git stashes..."
                    emptyLabel="No stashes found."
                    hasResults={gitStashes.length > 0}
                  />
                </label>
              </>
            )}
          </div>
        )}

        {!activePath && (
          <div className="sidebar-error">
            Save and select a {provider === "git" ? "Git directory" : "Perforce workspace"} in
            the sidebar before opening this diff.
          </div>
        )}

        <div className="dialog-actions">
          <button type="button" className="toolbar-button" onClick={onCancel} disabled={importing}>
            Cancel
          </button>
          <button type="submit" className="btn-merge-toggle" disabled={!canSubmit}>
            {importing ? <LoadingIcon className="button-icon-spin" /> : null}
            <span>{importing ? "Opening..." : "Open Diff"}</span>
          </button>
        </div>
      </form>
    </div>
  );
}

function ModeButton({
  active,
  label,
  onClick,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={`dialog-mode-button ${active ? "dialog-mode-button-active" : ""}`}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

function PathSummary({ label, path }: { label: string; path: string | null }) {
  return (
    <div className="dialog-path-summary">
      <span className="dialog-path-label">{label}</span>
      <div className="dialog-path-value" title={path ?? ""}>
        {path ?? "No saved directory selected"}
      </div>
    </div>
  );
}

function SavedLocationPicker({
  label,
  locations,
  selectedLocationId,
  onChange,
}: {
  label: string;
  locations: SavedWorkspaceLocation[];
  selectedLocationId: string | null;
  onChange: (locationId: string | null) => Promise<void> | void;
}) {
  return (
    <label className="dialog-field">
      <span>{label}</span>
      <select
        value={selectedLocationId ?? ""}
        onChange={(event) => onChange(event.target.value || null)}
      >
        <option value="">Directory...</option>
        {locations.map((location) => (
          <option key={location.id} value={location.id}>
            {location.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function LookupStatus({
  activePath,
  lookupState,
  lookupError,
  loadingLabel,
  emptyLabel,
  hasResults,
}: {
  activePath: string | null;
  lookupState: LookupState;
  lookupError: string | null;
  loadingLabel: string;
  emptyLabel: string;
  hasResults: boolean;
}) {
  if (!activePath) {
    return <span className="dialog-helper">Choose a saved directory first.</span>;
  }
  if (lookupState === "loading") {
    return (
      <span className="dialog-helper dialog-helper-inline">
        <LoadingIcon className="button-icon-spin" />
        <span>{loadingLabel}</span>
      </span>
    );
  }
  if (lookupState === "error") {
    return <span className="dialog-helper dialog-helper-error">{lookupError}</span>;
  }
  if (lookupState === "ready" && !hasResults) {
    return <span className="dialog-helper">{emptyLabel}</span>;
  }
  return null;
}
