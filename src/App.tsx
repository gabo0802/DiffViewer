import React, { useCallback, useEffect, useState } from "react";
import * as api from "./api";
import Sidebar from "./components/Sidebar";
import DiffViewer from "./components/DiffViewer";
import MergePanel from "./components/MergePanel";
import SettingsPanel from "./components/SettingsPanel";
import { CloseIcon, EditIcon, ThemeIcon } from "./components/Icons";
import {
  loadEditorPreferences,
  type EditorPreferences,
} from "./editorPreferences";
import { useDiffMergeScrollSync } from "./hooks/useDiffMergeScrollSync";
import { useDiffTabs } from "./hooks/useDiffTabs";
import { usePersistentState } from "./hooks/usePersistentState";
import { useResizablePane } from "./hooks/useResizablePane";
import type { FileDiff, RenderedDiffModel } from "./types";

const SIDEBAR_WIDTH_KEY = "diffviewer.sidebarWidth";
const EDITOR_PREFERENCES_KEY = "diffviewer.editorPreferences";
const THEME_KEY = "diffviewer.theme";

export default function App() {
  const [mergeVisible, setMergeVisible] = useState(false);
  const [sidebarRefreshToken, setSidebarRefreshToken] = useState(0);
  const [sidebarRefreshCommandToken, setSidebarRefreshCommandToken] = useState(0);
  const [activeWorkspaceView, setActiveWorkspaceView] = useState<"diff" | "settings">("diff");
  const [selectedTabIds, setSelectedTabIds] = useState<string[]>([]);
  const [previousHunkToken, setPreviousHunkToken] = useState(0);
  const [nextHunkToken, setNextHunkToken] = useState(0);
  const [editorPreferences, setEditorPreferences] = useState<EditorPreferences>(() =>
    loadEditorPreferences(EDITOR_PREFERENCES_KEY)
  );
  const [theme, setTheme] = usePersistentState<ThemeMode>(THEME_KEY, readStoredTheme);
  const {
    tabs,
    setTabs,
    activeTab,
    setActiveTab,
    currentFileDiff: currentFd,
    tabLabels,
    openFileDiff,
    closeTabs,
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

  const handleSelectFileDiff = useCallback((fileDiff: FileDiff) => {
    setActiveWorkspaceView("diff");
    setSelectedTabIds([]);
    openFileDiff(fileDiff);
  }, [openFileDiff]);

  const requestSidebarRefresh = useCallback(() => {
    setSidebarRefreshCommandToken((current) => current + 1);
  }, []);

  const closeRequestedTabs = useCallback((ids: string[]) => {
    if (ids.length === 0) return;
    closeTabs(ids);
    setSelectedTabIds((current) => current.filter((id) => !ids.includes(id)));
  }, [closeTabs]);

  const closeCurrentSelection = useCallback(() => {
    if (selectedTabIds.length > 0) {
      closeRequestedTabs(selectedTabIds);
      return;
    }

    if (currentFd) {
      closeRequestedTabs([currentFd.filediff_id]);
    }
  }, [closeRequestedTabs, currentFd, selectedTabIds]);

  const handleMergeSaveComplete = useCallback(async () => {
    if (!currentFd) return;

    const workspace = await api.getCurrentWorkspace();
    await api.refreshWorkspaceDiffsets(workspace.workspace_id);
    const refreshedFilediffs = await api.listFilediffs(currentFd.diffset_id);
    const replacement =
      refreshedFilediffs.find((fd) => fd.display_path === currentFd.display_path) ?? null;

    setSidebarRefreshToken(Date.now());

    if (!replacement) {
      setTabs((prev) => prev.filter((tab) => tab.filediff_id !== currentFd.filediff_id));
      setActiveTab(null);
      setSelectedTabIds((prev) => prev.filter((tabId) => tabId !== currentFd.filediff_id));
      setMergeVisible(false);
      return;
    }

    setTabs((prev) =>
      prev.map((tab) => (tab.filediff_id === currentFd.filediff_id ? replacement : tab))
    );
    setActiveTab(replacement.filediff_id);
    setSelectedTabIds((prev) =>
      prev.map((tabId) => (tabId === currentFd.filediff_id ? replacement.filediff_id : tabId))
    );
  }, [currentFd, setActiveTab, setTabs]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.ctrlKey) return;
      const target = event.target as HTMLElement | null;
      const tagName = target?.tagName?.toLowerCase();
      const insideMonaco = Boolean(target?.closest?.(".monaco-editor"));
      const isTextField =
        !insideMonaco &&
        (tagName === "input" ||
          tagName === "textarea" ||
          tagName === "select" ||
          target?.isContentEditable);

      if (event.key === "Tab") {
        if (tabs.length === 0) return;
        event.preventDefault();
        const currentIndex = tabs.findIndex((tab) => tab.filediff_id === activeTab);
        const nextIndex = currentIndex >= 0 ? (currentIndex + 1) % tabs.length : 0;
        setActiveTab(tabs[nextIndex].filediff_id);
        setActiveWorkspaceView("diff");
        setSelectedTabIds([]);
        return;
      }

      if (isTextField) return;

      const key = event.key.toLowerCase();

      if (key === "d") {
        event.preventDefault();
        closeCurrentSelection();
        return;
      }

      if (key === "r") {
        event.preventDefault();
        requestSidebarRefresh();
        return;
      }

      if (key === "e") {
        if (!currentFd || !canEditCurrent) return;
        event.preventDefault();
        setActiveWorkspaceView("diff");
        setSelectedTabIds([]);
        setMergeVisible((current) => !current);
        return;
      }

      if (key === "1") {
        if (!currentFd || activeWorkspaceView !== "diff") return;
        event.preventDefault();
        setPreviousHunkToken((current) => current + 1);
        return;
      }

      if (key === "2") {
        if (!currentFd || activeWorkspaceView !== "diff") return;
        event.preventDefault();
        setNextHunkToken((current) => current + 1);
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [
    activeTab,
    activeWorkspaceView,
    canEditCurrent,
    closeCurrentSelection,
    currentFd,
    requestSidebarRefresh,
    tabs,
  ]);

  return (
    <div className="app-root">
      <div className="sidebar-shell" style={{ width: sidebarWidth }}>
        <Sidebar
          onSelectFileDiff={handleSelectFileDiff}
          onOpenSettings={() => {
            setActiveWorkspaceView("settings");
            setSelectedTabIds([]);
          }}
          settingsActive={activeWorkspaceView === "settings"}
          refreshToken={sidebarRefreshToken}
          refreshCommandToken={sidebarRefreshCommandToken}
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
              className={`tab ${
                activeWorkspaceView === "diff" && fd.filediff_id === activeTab ? "tab-active" : ""
              } ${selectedTabIds.includes(fd.filediff_id) ? "tab-selected" : ""}`}
              onClick={(event) => {
                if (event.ctrlKey || event.metaKey) {
                  event.preventDefault();
                  setSelectedTabIds((current) =>
                    current.includes(fd.filediff_id)
                      ? current.filter((id) => id !== fd.filediff_id)
                      : [...current, fd.filediff_id]
                  );
                  return;
                }
                setActiveWorkspaceView("diff");
                setSelectedTabIds([]);
                setActiveTab(fd.filediff_id);
              }}
              title={fd.display_path}
            >
              <span className="tab-label">{tabLabels[fd.filediff_id] ?? fd.display_path}</span>
              <button
                type="button"
                className="tab-close"
                title={
                  selectedTabIds.length > 1 && selectedTabIds.includes(fd.filediff_id)
                    ? `Close ${selectedTabIds.length} selected tabs`
                    : `Close ${fd.display_path}`
                }
                onClick={(e) => {
                  e.stopPropagation();
                  closeRequestedTabs(
                    selectedTabIds.length > 1 && selectedTabIds.includes(fd.filediff_id)
                      ? selectedTabIds
                      : [fd.filediff_id]
                  );
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
                onClick={() => {
                  setActiveWorkspaceView("diff");
                  setSelectedTabIds([]);
                  setMergeVisible((v) => !v);
                }}
              >
                <EditIcon />
                <span>{mergeVisible ? "Hide Merge" : "Edit / Resolve"}</span>
              </button>
            )}
          </div>
        </div>

        <div
          className={`workspace-shell ${
            activeWorkspaceView === "diff" && mergeVisible && currentFd
              ? "workspace-shell-merge"
              : ""
          }`}
        >
          <div className="editor-area">
            {activeWorkspaceView === "settings" ? (
              <SettingsPanel
                editorPreferences={editorPreferences}
                onEditorPreferencesChange={setEditorPreferences}
              />
            ) : currentFd ? (
              <DiffViewer
                fileDiff={currentFd}
                displayLabel={tabLabels[currentFd.filediff_id] ?? currentFd.display_path}
                editorPreferences={editorPreferences}
                theme={theme}
                onModelChange={handleRenderedModelChange}
                onScrollRowChange={handleDiffScroll}
                syncedTopRow={syncSignal?.source === "merge" ? syncSignal.diffTopRow : null}
                syncToken={syncSignal?.source === "merge" ? syncSignal.token : 0}
                previousHunkToken={previousHunkToken}
                nextHunkToken={nextHunkToken}
              />
            ) : (
              <div className="empty-state">
                Open a diff from the sidebar, or import a patch file.
              </div>
            )}
          </div>

          {activeWorkspaceView === "diff" && currentFd && canEditCurrent && (
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
