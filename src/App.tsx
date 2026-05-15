import React, { useEffect, useRef, useState } from "react";
import Sidebar from "./components/Sidebar";
import DiffViewer from "./components/DiffViewer";
import MergePanel from "./components/MergePanel";
import type { FileDiff } from "./types";

const SIDEBAR_WIDTH_KEY = "diffviewer.sidebarWidth";

export default function App() {
  const [tabs, setTabs] = useState<FileDiff[]>([]);
  const [activeTab, setActiveTab] = useState<string | null>(null);
  const [mergeVisible, setMergeVisible] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState(() => readStoredSidebarWidth());
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const sidebarStartX = useRef(0);
  const sidebarStartWidth = useRef(sidebarWidth);

  const openFileDiff = (fd: FileDiff) => {
    if (!tabs.find((t) => t.filediff_id === fd.filediff_id)) {
      setTabs((prev) => [...prev, fd]);
    }
    setActiveTab(fd.filediff_id);
  };

  const closeTab = (id: string) => {
    setTabs((prev) => {
      const closedIndex = prev.findIndex((t) => t.filediff_id === id);
      const nextTabs = prev.filter((t) => t.filediff_id !== id);

      if (activeTab === id) {
        const fallbackTab =
          nextTabs[closedIndex] ?? nextTabs[Math.max(0, closedIndex - 1)] ?? null;
        setActiveTab(fallbackTab?.filediff_id ?? null);
        if (!fallbackTab) {
          setMergeVisible(false);
        }
      }

      return nextTabs;
    });
  };

  const currentFd = tabs.find((t) => t.filediff_id === activeTab) ?? null;

  useEffect(() => {
    window.localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    if (!isResizingSidebar) return;

    const onMouseMove = (event: MouseEvent) => {
      const delta = event.clientX - sidebarStartX.current;
      const nextWidth = clamp(sidebarStartWidth.current + delta, 220, 520);
      setSidebarWidth(nextWidth);
    };

    const onMouseUp = () => setIsResizingSidebar(false);

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [isResizingSidebar]);

  const beginSidebarResize = (event: React.MouseEvent<HTMLDivElement>) => {
    sidebarStartX.current = event.clientX;
    sidebarStartWidth.current = sidebarWidth;
    setIsResizingSidebar(true);
  };

  return (
    <div className="app-root">
      <div className="sidebar-shell" style={{ width: sidebarWidth }}>
        <Sidebar onSelectFileDiff={openFileDiff} />
      </div>
      <div
        className={`sidebar-resizer ${isResizingSidebar ? "sidebar-resizer-active" : ""}`}
        onMouseDown={beginSidebarResize}
        title="Drag to resize sidebar"
      />

      <main className="main">
        <div className="tab-bar">
          {tabs.map((fd) => (
            <div
              key={fd.filediff_id}
              className={`tab ${fd.filediff_id === activeTab ? "tab-active" : ""}`}
              onClick={() => setActiveTab(fd.filediff_id)}
            >
              <span className="tab-label">{fd.display_path}</span>
              <button
                type="button"
                className="tab-close"
                title={`Close ${fd.display_path}`}
                onClick={(e) => {
                  e.stopPropagation();
                  closeTab(fd.filediff_id);
                }}
              >
                x
              </button>
            </div>
          ))}

          <div className="toolbar-actions">
            {currentFd && (
              <button
                type="button"
                className="btn-merge-toggle"
                onClick={() => setMergeVisible((v) => !v)}
              >
                {mergeVisible ? "Hide Merge" : "Edit / Resolve"}
              </button>
            )}
          </div>
        </div>

        <div className={`workspace-shell ${mergeVisible && currentFd ? "workspace-shell-merge" : ""}`}>
          <div className="editor-area">
            {currentFd ? (
              <DiffViewer fileDiff={currentFd} />
            ) : (
              <div className="empty-state">
                Open a diff from the sidebar, or import a patch file.
              </div>
            )}
          </div>

          {currentFd && (
            <MergePanel
              fileDiff={currentFd}
              visible={mergeVisible}
              onToggle={() => setMergeVisible(false)}
            />
          )}
        </div>
      </main>
    </div>
  );
}

function readStoredSidebarWidth() {
  const raw = window.localStorage.getItem(SIDEBAR_WIDTH_KEY);
  if (!raw) return 260;
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? clamp(parsed, 220, 520) : 260;
}

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
