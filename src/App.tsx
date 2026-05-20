import React, { useCallback, useEffect, useState } from "react";
import * as api from "./api";
import Sidebar from "./components/Sidebar";
import DiffViewer from "./components/DiffViewer";
import MergePanel from "./components/MergePanel";
import { CloseIcon, EditIcon, ThemeIcon } from "./components/Icons";
import {
  loadEditorPreferences,
  type EditorPreferences,
} from "./editorPreferences";
import { useDiffMergeScrollSync } from "./hooks/useDiffMergeScrollSync";
import { useDiffTabs } from "./hooks/useDiffTabs";
import { usePersistentState } from "./hooks/usePersistentState";
import { useResizablePane } from "./hooks/useResizablePane";
import type { RenderedDiffModel } from "./types";

const SIDEBAR_WIDTH_KEY = "diffviewer.sidebarWidth";
const EDITOR_PREFERENCES_KEY = "diffviewer.editorPreferences";
const THEME_KEY = "diffviewer.theme";

export default function App() {
  const [mergeVisible, setMergeVisible] = useState(false);
  const [sidebarRefreshToken, setSidebarRefreshToken] = useState(0);
  const [editorPreferences, setEditorPreferences] = useState<EditorPreferences>(() =>
    loadEditorPreferences(EDITOR_PREFERENCES_KEY)
  );
  const [theme, setTheme] = usePersistentState<ThemeMode>(THEME_KEY, readStoredTheme);
  const {
    tabs,
    activeTab,
    setActiveTab,
    currentFileDiff: currentFd,
    tabLabels,
    openFileDiff,
    closeTab,
    reconcileTabs,
    firstChangedMergeLine,
    canEditCurrent,
  } = useDiffTabs(() => setMergeVisible(false));
  const {
    width: sidebarWidth,
    isResizing: isResizingSidebar,
    beginResize: beginSidebarResize,
  } = useResizablePane(readStoredSidebarWidth(), 220, 520);
  const {
    setRenderedModel,
    syncSignal,
    handleDiffScroll,
    handleMergeScroll,
  } = useDiffMergeScrollSync(currentFd?.filediff_id);

  useEffect(() => {
    window.localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    window.localStorage.setItem(EDITOR_PREFERENCES_KEY, JSON.stringify(editorPreferences));
  }, [editorPreferences]);

  useEffect(() => {
    window.localStorage.setItem(THEME_KEY, theme);
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useEffect(() => {
    if (!canEditCurrent && mergeVisible) {
      setMergeVisible(false);
    }
  }, [canEditCurrent, mergeVisible]);

  const handleRenderedModelChange = useCallback((nextModel: RenderedDiffModel | null) => {
    setRenderedModel(nextModel);
  }, [setRenderedModel]);

  const refreshWorkspaceAndTabs = useCallback(async () => {
    const workspace = await api.getCurrentWorkspace();
    const refreshedDiffsets = await api.refreshWorkspaceDiffsets(workspace.workspace_id);
    const diffsetKindById = Object.fromEntries(
      refreshedDiffsets.map((diffset) => [diffset.diffset_id, diffset.kind])
    );
    const openDiffsetIds = [...new Set(tabs.map((tab) => tab.diffset_id))];
    if (openDiffsetIds.length > 0) {
      const refreshedEntries = await Promise.all(
        openDiffsetIds.map(async (diffsetId) => {
          const filediffs = await api.listFilediffs(diffsetId);
          const diffsetKind = diffsetKindById[diffsetId];
          return [
            diffsetId,
            filediffs.map((filediff) => ({
              ...filediff,
              diffset_kind: diffsetKind,
            })),
          ] as const;
        })
      );
      reconcileTabs(Object.fromEntries(refreshedEntries));
    }

    setSidebarRefreshToken(Date.now());
  }, [reconcileTabs, tabs]);

  const handleMergeSaveComplete = useCallback(async () => {
    if (!currentFd) return;
    await refreshWorkspaceAndTabs();
  }, [currentFd, refreshWorkspaceAndTabs]);

  return (
    <div className="app-root">
      <div className="sidebar-shell" style={{ width: sidebarWidth }}>
        <Sidebar
          onSelectFileDiff={openFileDiff}
          refreshToken={sidebarRefreshToken}
          onRefreshWorkspace={refreshWorkspaceAndTabs}
        />
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
              title={fd.display_path}
            >
              <span className="tab-label">{tabLabels[fd.filediff_id] ?? fd.display_path}</span>
              <button
                type="button"
                className="tab-close"
                title={`Close ${fd.display_path}`}
                onClick={(e) => {
                  e.stopPropagation();
                  closeTab(fd.filediff_id);
                }}
              >
                <CloseIcon />
              </button>
            </div>
          ))}

          <div className="toolbar-actions">
            <button
              type="button"
              className="toolbar-button toolbar-button-with-icon"
              onClick={() => setTheme((current) => (current === "dark" ? "light" : "dark"))}
              title={theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
            >
              <ThemeIcon mode={theme} />
              <span>{theme === "dark" ? "Light Mode" : "Dark Mode"}</span>
            </button>
            {currentFd && canEditCurrent && (
              <button
                type="button"
                className="btn-merge-toggle button-with-icon"
                onClick={() => setMergeVisible((v) => !v)}
              >
                <EditIcon />
                <span>{mergeVisible ? "Hide Merge" : "Edit / Resolve"}</span>
              </button>
            )}
          </div>
        </div>

        <div className={`workspace-shell ${mergeVisible && currentFd ? "workspace-shell-merge" : ""}`}>
          <div className="editor-area">
            {currentFd ? (
              <DiffViewer
                fileDiff={currentFd}
                displayLabel={tabLabels[currentFd.filediff_id] ?? currentFd.display_path}
                editorPreferences={editorPreferences}
                theme={theme}
                onModelChange={handleRenderedModelChange}
                onScrollRowChange={handleDiffScroll}
                syncedTopRow={syncSignal?.source === "merge" ? syncSignal.diffTopRow : null}
                syncToken={syncSignal?.source === "merge" ? syncSignal.token : 0}
              />
            ) : (
              <div className="empty-state">
                Open a diff from the sidebar, or import a patch file.
              </div>
            )}
          </div>

          {currentFd && canEditCurrent && (
            <MergePanel
              fileDiff={currentFd}
              visible={mergeVisible}
              onToggle={() => setMergeVisible(false)}
              onSaveComplete={handleMergeSaveComplete}
              editorPreferences={editorPreferences}
              theme={theme}
              onEditorPreferencesChange={setEditorPreferences}
              initialFocusLine={firstChangedMergeLine}
              onScrollLineChange={handleMergeScroll}
              syncedTopLine={syncSignal?.source === "diff" ? syncSignal.mergeTopLine : null}
              syncToken={syncSignal?.source === "diff" ? syncSignal.token : 0}
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

type ThemeMode = "dark" | "light";

function readStoredTheme(): ThemeMode {
  const stored = window.localStorage.getItem(THEME_KEY);
  return stored === "light" ? "light" : "dark";
}
