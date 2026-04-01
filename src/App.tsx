import React, { useState } from "react";
import Sidebar from "./components/Sidebar";
import DiffViewer from "./components/DiffViewer";
import MergePanel from "./components/MergePanel";
import type { FileDiff } from "./types";

export default function App() {
  const [tabs, setTabs] = useState<FileDiff[]>([]);
  const [activeTab, setActiveTab] = useState<string | null>(null);
  const [mergeVisible, setMergeVisible] = useState(false);

  const openFileDiff = (fd: FileDiff) => {
    if (!tabs.find((t) => t.filediff_id === fd.filediff_id)) {
      setTabs((prev) => [...prev, fd]);
    }
    setActiveTab(fd.filediff_id);
  };

  const closeTab = (id: string) => {
    setTabs((prev) => prev.filter((t) => t.filediff_id !== id));
    if (activeTab === id) {
      setActiveTab(tabs.length > 1 ? tabs[tabs.length - 2]?.filediff_id ?? null : null);
    }
  };

  const currentFd = tabs.find((t) => t.filediff_id === activeTab) ?? null;

  return (
    <div className="app-root">
      <Sidebar onSelectFileDiff={openFileDiff} />

      <main className="main">
        {/* Tab bar */}
        <div className="tab-bar">
          {tabs.map((fd) => (
            <div
              key={fd.filediff_id}
              className={`tab ${fd.filediff_id === activeTab ? "tab-active" : ""}`}
              onClick={() => setActiveTab(fd.filediff_id)}
            >
              <span className="tab-label">{fd.display_path}</span>
              <button
                className="tab-close"
                onClick={(e) => {
                  e.stopPropagation();
                  closeTab(fd.filediff_id);
                }}
              >
                ×
              </button>
            </div>
          ))}

          {/* Toolbar actions */}
          <div className="toolbar-actions">
            {currentFd && (
              <button
                className="btn-merge-toggle"
                onClick={() => setMergeVisible((v) => !v)}
              >
                {mergeVisible ? "Hide Merge" : "Edit / Resolve"}
              </button>
            )}
          </div>
        </div>

        {/* Diff viewer */}
        <div className="editor-area">
          {currentFd ? (
            <DiffViewer fileDiff={currentFd} />
          ) : (
            <div className="empty-state">
              Open a diff from the sidebar, or import a patch file.
            </div>
          )}
        </div>

        {/* Merge panel */}
        {currentFd && (
          <MergePanel
            fileDiff={currentFd}
            visible={mergeVisible}
            onToggle={() => setMergeVisible(false)}
          />
        )}
      </main>
    </div>
  );
}
