import React, { useCallback, useEffect, useMemo, useState } from "react";
import * as api from "../api";
import { buildDisambiguatedPathLabels } from "../pathLabels";
import type {
  DiffSet,
  FileDiff,
  SavedWorkspaceLocation,
  Workspace,
  WorkspaceSettings,
} from "../types";
import AddDiffDialog, { type AddDiffRequest } from "./AddDiffDialog";
import FormDialog from "./FormDialog";
import { CloseIcon, LoadingIcon, PlusIcon, SettingsIcon } from "./Icons";

interface Props {
  onSelectFileDiff: (fd: FileDiff) => void;
  onOpenSettings?: () => void;
  settingsActive?: boolean;
  refreshToken?: number;
  refreshCommandToken?: number;
}

type DiffSetMeta = {
  change?: string;
  status?: string;
  user?: string;
  client?: string;
  file_count?: number;
  repo_path?: string;
  rev?: string;
};

type DirectoryProvider = "git" | "p4";

const PROVIDER_ORDER = ["p4", "git", "patch", "external"];
const EMPTY_SETTINGS: WorkspaceSettings = {
  savedGitDirectories: [],
  savedP4Directories: [],
  selectedGitDirectoryId: null,
  selectedP4DirectoryId: null,
};

export default function Sidebar({
  onSelectFileDiff,
  onOpenSettings,
  settingsActive = false,
  refreshToken = 0,
  refreshCommandToken = 0,
}: Props) {
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [settings, setSettings] = useState<WorkspaceSettings>(EMPTY_SETTINGS);
  const [diffsets, setDiffsets] = useState<DiffSet[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [filediffs, setFilediffs] = useState<Record<string, FileDiff[]>>({});
  const [isImporting, setIsImporting] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [addDiffVisible, setAddDiffVisible] = useState(false);
  const [saveDirectoryProvider, setSaveDirectoryProvider] = useState<DirectoryProvider | null>(null);

  const loadWorkspaceState = useCallback(async () => {
    const [ws, nextSettings] = await Promise.all([
      api.getCurrentWorkspace(),
      api.getCurrentWorkspaceSettings(),
    ]);
    setWorkspace(ws);
    setSettings(nextSettings);
    return ws;
  }, []);

  const loadDiffsets = useCallback(async (ws: Workspace, refreshLive = false) => {
    const next = refreshLive
      ? await api.refreshWorkspaceDiffsets(ws.workspace_id)
      : await api.listDiffsets(ws.workspace_id);
    setDiffsets(next);
    return next;
  }, []);

  useEffect(() => {
    loadWorkspaceState().catch((err) => setError(String(err)));
  }, [loadWorkspaceState]);

  useEffect(() => {
    if (!workspace) return;
    loadDiffsets(workspace).catch((err) => setError(String(err)));
  }, [workspace, loadDiffsets]);

  useEffect(() => {
    if (!workspace || refreshToken === 0) return;
    loadDiffsets(workspace)
      .then(async () => {
        if (!expanded) return;
        const fds = await api.listFilediffs(expanded);
        setFilediffs((prev) => ({ ...prev, [expanded]: fds }));
      })
      .catch((err) => setError(String(err)));
  }, [expanded, loadDiffsets, refreshToken, workspace]);

  useEffect(() => {
    if (!workspace || refreshCommandToken === 0) return;
    refreshFromSidebar().catch((err) => setError(String(err)));
  }, [refreshCommandToken, workspace]);

  const grouped = useMemo(() => {
    const groups = new Map<string, DiffSet[]>();
    for (const ds of diffsets) {
      const provider = ds.provider || providerFromSource(ds.source_type);
      groups.set(provider, [...(groups.get(provider) ?? []), ds]);
    }
    return [...groups.entries()].sort(([a], [b]) => {
      const ai = PROVIDER_ORDER.indexOf(a);
      const bi = PROVIDER_ORDER.indexOf(b);
      return (ai === -1 ? 99 : ai) - (bi === -1 ? 99 : bi);
    });
  }, [diffsets]);

  const selectedGitLocation = findSelectedLocation(
    settings.savedGitDirectories,
    settings.selectedGitDirectoryId
  );
  const selectedP4Location = findSelectedLocation(
    settings.savedP4Directories,
    settings.selectedP4DirectoryId
  );

  const toggleDiffset = async (dsId: string) => {
    if (expanded === dsId) {
      setExpanded(null);
      return;
    }
    setExpanded(dsId);
    if (!filediffs[dsId]) {
      const fds = await api.listFilediffs(dsId);
      setFilediffs((prev) => ({ ...prev, [dsId]: fds }));
    }
  };

  const handleAddDiff = async (request: AddDiffRequest) => {
    setError(null);
    setIsImporting(true);
    try {
      let diffsetId: string;
      switch (request.kind) {
        case "gitWorking":
          diffsetId = await api.importGitWorkingTree(request.repoPath ?? "");
          break;
        case "gitCommit":
          diffsetId = await api.importGitCommit(request.repoPath ?? "", request.rev ?? "HEAD");
          break;
        case "p4Pending":
          diffsetId = await api.importP4Pending(request.change ?? "default", request.cwd);
          break;
        case "p4Shelved":
          diffsetId = await api.importP4Shelved(request.change ?? "", request.cwd);
          break;
        case "p4Submitted":
          diffsetId = await api.importP4Submitted(request.change ?? "", request.cwd);
          break;
      }

      if (workspace) {
        await loadDiffsets(workspace);
      }
      setExpanded(diffsetId);
      const fds = await api.listFilediffs(diffsetId);
      setFilediffs((prev) => ({ ...prev, [diffsetId]: fds }));
      setAddDiffVisible(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsImporting(false);
    }
  };

  const handleSaveDirectory = async (values: Record<string, string>) => {
    if (!saveDirectoryProvider) return;
    const path = values.path.trim();
    if (!path) return;
    try {
      const nextSettings = await api.saveCurrentWorkspaceLocation(saveDirectoryProvider, path);
      setSettings(nextSettings);
      setSaveDirectoryProvider(null);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleSelectDirectory = async (
    provider: DirectoryProvider,
    locationId: string | null
  ) => {
    setError(null);
    try {
      const nextSettings = await api.selectCurrentWorkspaceLocation(provider, locationId);
      setSettings(nextSettings);
    } catch (err) {
      setError(String(err));
    }
  };

  const handleRemoveDirectory = async (
    provider: DirectoryProvider,
    location: SavedWorkspaceLocation | null
  ) => {
    if (!location) return;
    setError(null);
    try {
      const nextSettings = await api.removeCurrentWorkspaceLocation(provider, location.id);
      setSettings(nextSettings);
    } catch (err) {
      setError(String(err));
    }
  };

  const removeDiffset = async (diffsetId: string) => {
    setError(null);
    try {
      await api.deleteDiffset(diffsetId);
      setFilediffs((prev) => {
        const next = { ...prev };
        delete next[diffsetId];
        return next;
      });
      if (expanded === diffsetId) {
        setExpanded(null);
      }
      if (workspace) {
        await loadDiffsets(workspace);
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const refreshFromSidebar = async () => {
    if (!workspace) return;
    setError(null);
    setIsRefreshing(true);
    try {
      await loadDiffsets(workspace, true);
      setFilediffs({});
      if (expanded) {
        const fds = await api.listFilediffs(expanded);
        setFilediffs((prev) => ({ ...prev, [expanded]: fds }));
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsRefreshing(false);
    }
  };

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <span className="sidebar-title">{workspace?.name ?? "Loading..."}</span>
        <button
          className="icon-button icon-button-compact"
          onClick={refreshFromSidebar}
          title="Refresh diffsets"
          disabled={!workspace || isRefreshing}
        >
          {isRefreshing ? <LoadingIcon className="button-icon-spin" /> : null}
          <span>Refresh</span>
        </button>
      </div>

      <div className="sidebar-content">
        <div className="sidebar-controls">
          <SavedDirectoryField
            label="P4 Depot"
            provider="p4"
            locations={settings.savedP4Directories}
            selectedLocation={selectedP4Location}
            onSelect={handleSelectDirectory}
            onSave={() => setSaveDirectoryProvider("p4")}
            onRemove={() => handleRemoveDirectory("p4", selectedP4Location)}
          />
          <SavedDirectoryField
            label="Git Directory"
            provider="git"
            locations={settings.savedGitDirectories}
            selectedLocation={selectedGitLocation}
            onSelect={handleSelectDirectory}
            onSave={() => setSaveDirectoryProvider("git")}
            onRemove={() => handleRemoveDirectory("git", selectedGitLocation)}
          />
          <button
            type="button"
            className="btn-merge-toggle sidebar-add-diff"
            onClick={() => setAddDiffVisible(true)}
          >
            Add Diff
          </button>
        </div>

        {error && <div className="sidebar-error">{error}</div>}

        {diffsets.length === 0 && (
          <div className="sidebar-empty">
            No diffs yet. Save a Git or P4 directory above, then open a working tree, commit, or
            changelist.
          </div>
        )}

        {grouped.map(([provider, items]) => (
          <section key={provider} className="sidebar-provider-group">
            <div className="sidebar-section-label">{providerLabel(provider)}</div>
            {items.map((ds) => (
              <DiffSetRow
                key={ds.diffset_id}
                diffset={ds}
                expanded={expanded === ds.diffset_id}
                filediffs={filediffs[ds.diffset_id]}
                onToggle={() => toggleDiffset(ds.diffset_id)}
                onSelectFileDiff={onSelectFileDiff}
                onRemove={() => removeDiffset(ds.diffset_id)}
              />
            ))}
          </section>
        ))}
      </div>

      <div className="sidebar-footer">
        <button
          type="button"
          className={`toolbar-button toolbar-button-with-icon sidebar-settings-button ${
            settingsActive ? "sidebar-settings-button-active" : ""
          }`}
          onClick={onOpenSettings}
          title="Open settings"
        >
          <SettingsIcon />
          <span>Settings</span>
        </button>
      </div>

      <AddDiffDialog
        visible={addDiffVisible}
        importing={isImporting}
        gitLocations={settings.savedGitDirectories}
        p4Locations={settings.savedP4Directories}
        selectedGitLocationId={settings.selectedGitDirectoryId}
        selectedP4LocationId={settings.selectedP4DirectoryId}
        onCancel={() => setAddDiffVisible(false)}
        onSubmit={handleAddDiff}
        onSelectLocation={handleSelectDirectory}
        loadGitCommits={(repoPath) => api.listGitCommits(repoPath)}
        loadP4PendingChanges={(cwd) => api.listP4PendingChanges(cwd)}
      />

      <FormDialog
        visible={saveDirectoryProvider !== null}
        title={saveDirectoryProvider === "git" ? "Save Git Directory" : "Save P4 Workspace"}
        description={
          saveDirectoryProvider === "git"
            ? "Enter the Git repository path to save and reuse."
            : "Enter the Perforce workspace path to save and reuse."
        }
        confirmLabel="Save"
        onCancel={() => setSaveDirectoryProvider(null)}
        onConfirm={handleSaveDirectory}
        onFieldAction={async (fieldId, currentValue) => {
          if (fieldId !== "path") return;
          const selected = await api.browseForDirectory(currentValue || undefined);
          return selected ?? undefined;
        }}
        fields={[
          {
            id: "path",
            label: "Path",
            defaultValue:
              saveDirectoryProvider === "git"
                ? selectedGitLocation?.path ?? ""
                : selectedP4Location?.path ?? "",
            placeholder:
              saveDirectoryProvider === "git"
                ? "Path to the Git repository"
                : "Path inside the Perforce workspace",
            required: true,
            actionTitle: "Browse for directory",
            actionIcon: "folder",
          },
        ]}
      />
    </aside>
  );
}

function SavedDirectoryField({
  label,
  provider,
  locations,
  selectedLocation,
  onSelect,
  onSave,
  onRemove,
}: {
  label: string;
  provider: DirectoryProvider;
  locations: SavedWorkspaceLocation[];
  selectedLocation: SavedWorkspaceLocation | null;
  onSelect: (provider: DirectoryProvider, locationId: string | null) => Promise<void>;
  onSave: () => void;
  onRemove: () => void;
}) {
  return (
    <div className="sidebar-directory-card">
      <div className="sidebar-directory-label">Current {label}</div>
      <div className="sidebar-directory-row">
        <select
          className="sidebar-select"
          value={selectedLocation?.id ?? ""}
          onChange={(event) => onSelect(provider, event.target.value || null)}
        >
          <option value="">Directory...</option>
          {locations.map((location) => (
            <option key={location.id} value={location.id}>
              {location.label}
            </option>
          ))}
        </select>
        <button
          type="button"
          className="icon-button icon-button-square"
          title={`Add ${label}`}
          onClick={onSave}
        >
          <PlusIcon />
        </button>
        <button
          type="button"
          className="icon-button icon-button-square"
          title={`Remove selected ${label}`}
          onClick={onRemove}
          disabled={!selectedLocation}
        >
          <CloseIcon />
        </button>
      </div>
      <div className="sidebar-directory-path" title={selectedLocation?.path ?? ""}>
        {selectedLocation?.path ?? "No saved directory selected"}
      </div>
    </div>
  );
}

function findSelectedLocation(
  locations: SavedWorkspaceLocation[],
  selectedId: string | null
) {
  if (!selectedId) return null;
  return locations.find((location) => location.id === selectedId) ?? null;
}

function DiffSetRow({
  diffset,
  expanded,
  filediffs,
  onToggle,
  onSelectFileDiff,
  onRemove,
}: {
  diffset: DiffSet;
  expanded: boolean;
  filediffs?: FileDiff[];
  onToggle: () => void;
  onSelectFileDiff: (fd: FileDiff) => void;
  onRemove: () => void;
}) {
  const meta = parseMeta(diffset.source_meta_json);
  const isP4 = diffset.provider === "p4";
  const count = meta.file_count ?? filediffs?.length;
  const fileLabels = useMemo(
    () =>
      buildDisambiguatedPathLabels(
        (filediffs ?? []).map((fd) => ({ id: fd.filediff_id, path: fd.display_path }))
      ),
    [filediffs]
  );

  return (
    <div className="sidebar-diffset">
      <div className="sidebar-diffset-row">
        <button className="sidebar-diffset-btn" onClick={onToggle}>
          <span className="chevron">{expanded ? "v" : ">"}</span>
          <span className="diffset-title">
            {isP4 && meta.change ? `CL ${meta.change}` : diffset.title}
            {isP4 && <small>{diffset.title}</small>}
          </span>
          <span className={`diffset-type badge badge-${diffset.provider}`}>
            {statusLabel(diffset, meta)}
          </span>
        </button>
        <button
          type="button"
          className="sidebar-diffset-close"
          title={`Remove ${diffset.title}`}
          onClick={(event) => {
            event.stopPropagation();
            onRemove();
          }}
        >
          <CloseIcon />
        </button>
      </div>
      <div className="diffset-meta">
        {meta.user || meta.client ? <span>{[meta.user, meta.client].filter(Boolean).join("@")}</span> : null}
        {meta.repo_path ? <span>{meta.repo_path}</span> : null}
        {count !== undefined ? <span>{count} files</span> : null}
      </div>

      {expanded &&
        filediffs?.map((fd) => (
          <button
            key={fd.filediff_id}
            className="sidebar-file-btn"
            onClick={() => onSelectFileDiff({ ...fd, diffset_kind: diffset.kind })}
            title={fd.display_path}
          >
            <span className={`status-dot status-${statusClass(fd.status)}`} />
            <span className="file-path">{fileLabels[fd.filediff_id] ?? fd.display_path}</span>
            <span className="file-action">{fd.status}</span>
          </button>
        ))}
    </div>
  );
}

function parseMeta(json: string): DiffSetMeta {
  try {
    return JSON.parse(json) as DiffSetMeta;
  } catch {
    return {};
  }
}

function providerFromSource(sourceType: string) {
  const normalized = sourceType.toLowerCase();
  if (normalized.includes("git")) return "git";
  if (normalized.includes("perforce") || normalized.includes("p4")) return "p4";
  if (normalized.includes("patch")) return "patch";
  return "external";
}

function providerLabel(provider: string) {
  if (provider === "p4") return "Perforce Changelists";
  if (provider === "git") return "Git Diffs";
  if (provider === "patch") return "Patch Imports";
  return "External Compares";
}

function statusLabel(diffset: DiffSet, meta: DiffSetMeta) {
  if (diffset.provider === "p4") return meta.status ?? p4KindLabel(diffset.kind);
  if (diffset.provider === "git") return diffset.kind === "gitCommit" ? "Commit" : "Working";
  return diffset.source_type;
}

function p4KindLabel(kind: string) {
  if (kind === "p4Shelved") return "Shelved";
  if (kind === "p4Submitted") return "Submitted";
  if (kind === "p4PendingDefault") return "Default";
  return "Pending";
}

function statusClass(status: string) {
  return status.toLowerCase().replace(/[^a-z0-9]+/g, "-");
}
