import React, { useEffect, useState } from "react";
import * as api from "../api";
import type { Workspace, DiffSet, FileDiff } from "../types";

interface Props {
  onSelectFileDiff: (fd: FileDiff) => void;
}

export default function Sidebar({ onSelectFileDiff }: Props) {
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [diffsets, setDiffsets] = useState<DiffSet[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [filediffs, setFilediffs] = useState<Record<string, FileDiff[]>>({});

  useEffect(() => {
    api.getCurrentWorkspace().then(setWorkspace).catch(console.error);
  }, []);

  useEffect(() => {
    if (workspace) {
      api
        .listDiffsets(workspace.workspace_id)
        .then(setDiffsets)
        .catch(console.error);
    }
  }, [workspace]);

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

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <span className="sidebar-title">
          {workspace?.name ?? "Loading…"}
        </span>
      </div>

      <div className="sidebar-section-label">DiffSets</div>

      {diffsets.length === 0 && (
        <div className="sidebar-empty">
          No diffs yet — import a patch or compare two files.
        </div>
      )}

      {diffsets.map((ds) => (
        <div key={ds.diffset_id} className="sidebar-diffset">
          <button
            className="sidebar-diffset-btn"
            onClick={() => toggleDiffset(ds.diffset_id)}
          >
            <span className="chevron">
              {expanded === ds.diffset_id ? "▾" : "▸"}
            </span>
            <span className="diffset-title">{ds.title}</span>
            <span className="diffset-type badge">{ds.source_type}</span>
          </button>

          {expanded === ds.diffset_id &&
            filediffs[ds.diffset_id]?.map((fd) => (
              <button
                key={fd.filediff_id}
                className="sidebar-file-btn"
                onClick={() => onSelectFileDiff(fd)}
              >
                <span className={`status-dot status-${fd.status}`} />
                {fd.display_path}
              </button>
            ))}
        </div>
      ))}
    </aside>
  );
}
