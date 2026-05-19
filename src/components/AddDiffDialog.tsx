import React, { useEffect, useMemo, useState } from "react";
import type {
  GitCommitSummary,
  P4PendingChangeSummary,
  SavedWorkspaceLocation,
} from "../types";
import { LoadingIcon } from "./Icons";

export type AddDiffImportKind =
  | "gitWorking"
  | "gitCommit"
  | "p4Pending"
  | "p4Shelved"
  | "p4Submitted";

export interface AddDiffRequest {
  kind: AddDiffImportKind;
  repoPath?: string;
  cwd?: string;
  rev?: string;
  change?: string;
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
  loadGitCommits: (repoPath: string) => Promise<GitCommitSummary[]>;
  loadP4PendingChanges: (cwd: string) => Promise<P4PendingChangeSummary[]>;
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
}: Props) {
  const defaultProvider = useMemo<ProviderMode>(() => {
    if (selectedP4LocationId) return "p4";
    return "git";
  }, [selectedP4LocationId]);
  const [provider, setProvider] = useState<ProviderMode>(defaultProvider);
  const [gitMode, setGitMode] = useState<"gitWorking" | "gitCommit">("gitWorking");
  const [p4Mode, setP4Mode] = useState<"p4Pending" | "p4Submitted" | "p4Shelved">("p4Pending");
  const [change, setChange] = useState("default");
  const [rev, setRev] = useState("HEAD");
  const [gitCommits, setGitCommits] = useState<GitCommitSummary[]>([]);
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
    setGitCommits([]);
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
          const nextCommits = await loadGitCommits(activePath);
          if (cancelled) return;
          setGitCommits(nextCommits);
          setRev(nextCommits[0]?.rev ?? "HEAD");
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
    loadGitCommits,
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
      !!rev.trim() ||
      !!change.trim());

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
                        : `CL ${item.change} - ${item.description}`}
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
