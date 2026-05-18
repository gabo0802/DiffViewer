import React, { useCallback, useEffect, useMemo, useState } from "react";
import * as api from "../api";
import { buildDisambiguatedPathLabels } from "../pathLabels";
import FormDialog, { type FormDialogField } from "./FormDialog";
import type { Workspace, DiffSet, FileDiff } from "../types";

interface Props {
  onSelectFileDiff: (fd: FileDiff) => void;
  refreshToken?: number;
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

const PROVIDER_ORDER = ["p4", "git", "patch", "external"];

export default function Sidebar({ onSelectFileDiff, refreshToken = 0 }: Props) {
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [diffsets, setDiffsets] = useState<DiffSet[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [filediffs, setFilediffs] = useState<Record<string, FileDiff[]>>({});
  const [isImporting, setIsImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingImportKind, setPendingImportKind] = useState<
    "p4Pending" | "p4Shelved" | "p4Submitted" | null
  >(null);

  const loadDiffsets = useCallback(async (ws: Workspace, refreshLive = false) => {
    const next = refreshLive
      ? await api.refreshWorkspaceDiffsets(ws.workspace_id)
      : await api.listDiffsets(ws.workspace_id);
    setDiffsets(next);
  }, []);

  useEffect(() => {
    api
      .getCurrentWorkspace()
      .then((ws) => {
        setWorkspace(ws);
      })
      .catch((err) => setError(String(err)));
  }, [loadDiffsets]);

  useEffect(() => {
    if (workspace) {
      loadDiffsets(workspace).catch((err) => setError(String(err)));
    }
  }, [workspace, loadDiffsets]);

  useEffect(() => {
    if (!workspace || refreshToken === 0) return;
    loadDiffsets(workspace, true)
      .then(async () => {
        if (!expanded) return;
        const fds = await api.listFilediffs(expanded);
        setFilediffs((prev) => ({ ...prev, [expanded]: fds }));
      })
      .catch((err) => setError(String(err)));
  }, [expanded, loadDiffsets, refreshToken, workspace]);

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

  const runImport = async (kind: "gitWorking" | "gitCommit" | "p4Pending" | "p4Shelved" | "p4Submitted") => {
    setError(null);
    if (kind === "p4Pending" || kind === "p4Shelved" || kind === "p4Submitted") {
      setPendingImportKind(kind);
      return;
    }
    try {
      setIsImporting(true);
      let diffsetId: string | null = null;
      if (kind === "gitWorking") {
        const repoPath = window.prompt("Git repository path:", ".");
        if (!repoPath) return;
        diffsetId = await api.importGitWorkingTree(repoPath);
      } else if (kind === "gitCommit") {
        const repoPath = window.prompt("Git repository path:", ".");
        if (!repoPath) return;
        const rev = window.prompt("Git commit or revision:", "HEAD");
        if (!rev) return;
        diffsetId = await api.importGitCommit(repoPath, rev);
      }
      if (workspace) await loadDiffsets(workspace);
      if (diffsetId) {
        setExpanded(diffsetId);
        const fds = await api.listFilediffs(diffsetId);
        setFilediffs((prev) => ({ ...prev, [diffsetId!]: fds }));
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsImporting(false);
    }
  };

  const submitP4Import = async (values: Record<string, string>) => {
    if (!pendingImportKind) return;
    setError(null);
    try {
      setIsImporting(true);
      const change = values.change.trim();
      const cwd = values.cwd.trim() || undefined;
      let diffsetId: string | null = null;
      if (pendingImportKind === "p4Pending") {
        diffsetId = await api.importP4Pending(change, cwd);
      } else if (pendingImportKind === "p4Shelved") {
        diffsetId = await api.importP4Shelved(change, cwd);
      } else {
        diffsetId = await api.importP4Submitted(change, cwd);
      }
      if (workspace) await loadDiffsets(workspace);
      if (diffsetId) {
        setExpanded(diffsetId);
        const fds = await api.listFilediffs(diffsetId);
        setFilediffs((prev) => ({ ...prev, [diffsetId]: fds }));
      }
      setPendingImportKind(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsImporting(false);
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
    try {
      await loadDiffsets(workspace, true);
      setFilediffs({});
      if (expanded) {
        const fds = await api.listFilediffs(expanded);
        setFilediffs((prev) => ({ ...prev, [expanded]: fds }));
      }
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <span className="sidebar-title">{workspace?.name ?? "Loading..."}</span>
        <button
          className="icon-button"
          onClick={refreshFromSidebar}
          title="Refresh diffsets"
        >
          Refresh
        </button>
      </div>

      <div className="sidebar-actions">
        <button onClick={() => runImport("p4Pending")} disabled={isImporting}>P4 Pending</button>
        <button onClick={() => runImport("p4Shelved")} disabled={isImporting}>P4 Shelved</button>
        <button onClick={() => runImport("p4Submitted")} disabled={isImporting}>P4 Submitted</button>
        <button onClick={() => runImport("gitWorking")} disabled={isImporting}>Git Working</button>
        <button onClick={() => runImport("gitCommit")} disabled={isImporting}>Git Commit</button>
      </div>

      {error && <div className="sidebar-error">{error}</div>}

      {diffsets.length === 0 && (
        <div className="sidebar-empty">
          No diffs yet. Import P4, Git, patch, or two-file diffs to begin.
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
      <FormDialog
        visible={pendingImportKind !== null}
        title={dialogTitle(pendingImportKind)}
        description="Enter the changelist number and optional workspace path."
        confirmLabel="Open"
        onCancel={() => setPendingImportKind(null)}
        onConfirm={submitP4Import}
        fields={p4DialogFields(pendingImportKind)}
      />
    </aside>
  );
}

function dialogTitle(kind: "p4Pending" | "p4Shelved" | "p4Submitted" | null) {
  if (kind === "p4Pending") return "Open P4 Pending Changelist";
  if (kind === "p4Shelved") return "Open P4 Shelved Changelist";
  if (kind === "p4Submitted") return "Open P4 Submitted Changelist";
  return "";
}

function p4DialogFields(kind: "p4Pending" | "p4Shelved" | "p4Submitted" | null): FormDialogField[] {
  return [
    {
      id: "change",
      label: "Changelist",
      defaultValue: kind === "p4Pending" ? "default" : "",
      placeholder: kind === "p4Pending" ? "default or 12345" : "12345",
      required: true,
    },
    {
      id: "cwd",
      label: "Workspace",
      defaultValue: "",
      placeholder: "Optional workspace path",
    },
  ];
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
          x
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
            onClick={() => onSelectFileDiff(fd)}
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
