import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Sidebar from "./components/Sidebar";
import DiffViewer from "./components/DiffViewer";
import MergePanel from "./components/MergePanel";
import {
  loadEditorPreferences,
  type EditorPreferences,
} from "./editorPreferences";
import type { FileDiff, RenderedDiffModel } from "./types";

const SIDEBAR_WIDTH_KEY = "diffviewer.sidebarWidth";
const EDITOR_PREFERENCES_KEY = "diffviewer.editorPreferences";
const THEME_KEY = "diffviewer.theme";

export default function App() {
  const [tabs, setTabs] = useState<FileDiff[]>([]);
  const [activeTab, setActiveTab] = useState<string | null>(null);
  const [mergeVisible, setMergeVisible] = useState(false);
  const [sidebarRefreshToken, setSidebarRefreshToken] = useState(0);
  const [sidebarWidth, setSidebarWidth] = useState(() => readStoredSidebarWidth());
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [renderedModel, setRenderedModel] = useState<RenderedDiffModel | null>(null);
  const [syncSignal, setSyncSignal] = useState<ScrollSyncSignal | null>(null);
  const [editorPreferences, setEditorPreferences] = useState<EditorPreferences>(() =>
    loadEditorPreferences(EDITOR_PREFERENCES_KEY)
  );
  const [theme, setTheme] = useState<ThemeMode>(() => readStoredTheme());
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
  const firstChangedMergeLine = useMemo(
    () => (currentFd ? getFirstChangedMergeLine(currentFd) : 1),
    [currentFd]
  );
  const canEditCurrent = useMemo(
    () => (currentFd ? isEditableFileDiff(currentFd) : false),
    [currentFd]
  );

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
    setRenderedModel(null);
    setSyncSignal(null);
  }, [currentFd?.filediff_id]);

  useEffect(() => {
    if (!canEditCurrent && mergeVisible) {
      setMergeVisible(false);
    }
  }, [canEditCurrent, mergeVisible]);

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

  const handleRenderedModelChange = useCallback((nextModel: RenderedDiffModel | null) => {
    setRenderedModel(nextModel);
  }, []);

  const handleDiffScroll = useCallback(
    (diffTopRow: number) => {
      if (!renderedModel) return;
      setSyncSignal({
        source: "diff",
        diffTopRow,
        mergeTopLine: mapDiffRowToMergeLine(renderedModel, diffTopRow),
        token: Date.now(),
      });
    },
    [renderedModel]
  );

  const handleMergeScroll = useCallback(
    (mergeTopLine: number) => {
      if (!renderedModel) return;
      setSyncSignal({
        source: "merge",
        diffTopRow: mapMergeLineToDiffRow(renderedModel, mergeTopLine),
        mergeTopLine,
        token: Date.now(),
      });
    },
    [renderedModel]
  );

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
      setMergeVisible(false);
      return;
    }

    setTabs((prev) =>
      prev.map((tab) => (tab.filediff_id === currentFd.filediff_id ? replacement : tab))
    );
    setActiveTab(replacement.filediff_id);
  }, [currentFd]);

  return (
    <div className="app-root">
      <div className="sidebar-shell" style={{ width: sidebarWidth }}>
        <Sidebar onSelectFileDiff={openFileDiff} refreshToken={sidebarRefreshToken} />
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
            <button
              type="button"
              className="toolbar-button"
              onClick={() => setTheme((current) => (current === "dark" ? "light" : "dark"))}
            >
              {theme === "dark" ? "Light Mode" : "Dark Mode"}
            </button>
            {currentFd && canEditCurrent && (
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
              <DiffViewer
                fileDiff={currentFd}
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

type ScrollSyncSignal = {
  source: "diff" | "merge";
  diffTopRow: number;
  mergeTopLine: number;
  token: number;
};

type ParsedHunk = {
  new_start?: number;
};

function getFirstChangedMergeLine(fileDiff: FileDiff) {
  try {
    const hunks = JSON.parse(fileDiff.hunks_json) as ParsedHunk[];
    return Math.max(1, hunks[0]?.new_start ?? 1);
  } catch {
    return 1;
  }
}

function mapDiffRowToMergeLine(model: RenderedDiffModel, diffTopRow: number) {
  const rows = model.rows;
  if (rows.length === 0) return 1;
  const index = clamp(diffTopRow - 1, 0, rows.length - 1);

  for (let offset = 0; offset < rows.length; offset += 1) {
    const right = rows[index + offset]?.right;
    if (right && right.line_no > 0) return right.line_no;
    const previous = rows[index - offset]?.right;
    if (previous && previous.line_no > 0) return previous.line_no;
  }

  return 1;
}

function mapMergeLineToDiffRow(model: RenderedDiffModel, mergeTopLine: number) {
  const targetLine = Math.max(1, mergeTopLine);
  let fallbackRow = 1;

  for (let index = 0; index < model.rows.length; index += 1) {
    const right = model.rows[index].right;
    if (!right || right.line_no <= 0) continue;
    fallbackRow = index + 1;
    if (right.line_no >= targetLine) {
      return index + 1;
    }
  }

  return fallbackRow;
}

function readStoredTheme(): ThemeMode {
  const stored = window.localStorage.getItem(THEME_KEY);
  return stored === "light" ? "light" : "dark";
}

function isEditableFileDiff(fileDiff: FileDiff) {
  try {
    const parsed = JSON.parse(fileDiff.write_target_json);
    return parsed?.type !== "read_only";
  } catch {
    return true;
  }
}
