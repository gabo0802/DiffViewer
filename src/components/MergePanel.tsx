import React, { useEffect, useRef, useState } from "react";
import Editor from "@monaco-editor/react";
import * as api from "../api";
import {
  extractVirtualText,
  parseHunks,
  suggestedSavePath,
  type ParsedHunk,
} from "../diffDomain";
import {
  LANGUAGE_OPTIONS,
  resolveEditorLanguage,
  type EditorPreferences,
} from "../editorPreferences";
import FormDialog from "./FormDialog";
import type { FileDiff, MergeBuffer } from "../types";
import { EditIcon, LoadingIcon, SaveIcon } from "./Icons";

interface Props {
  fileDiff: FileDiff;
  visible: boolean;
  onToggle: () => void;
  onSaveComplete?: () => Promise<void> | void;
  editorPreferences: EditorPreferences;
  theme: "dark" | "light";
  onEditorPreferencesChange: React.Dispatch<React.SetStateAction<EditorPreferences>>;
  initialFocusLine: number;
  onScrollLineChange?: (topLine: number) => void;
  syncedTopLine?: number | null;
  syncToken?: number;
}

export default function MergePanel({
  fileDiff,
  visible,
  onToggle,
  onSaveComplete,
  editorPreferences,
  theme,
  onEditorPreferencesChange,
  initialFocusLine,
  onScrollLineChange,
  syncedTopLine = null,
  syncToken = 0,
}: Props) {
  const [buffer, setBuffer] = useState<MergeBuffer | null>(null);
  const [mergedText, setMergedText] = useState("");
  const [height, setHeight] = useState(320);
  const [isResizing, setIsResizing] = useState(false);
  const [showEditorSettings, setShowEditorSettings] = useState(false);
  const [saveAsVisible, setSaveAsVisible] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const editorRef = useRef<any>(null);
  const mergeDecorationIdsRef = useRef<string[]>([]);
  const suppressScrollSyncRef = useRef(false);
  const lastReportedTopLineRef = useRef(1);
  const lastFocusedFileRef = useRef<string | null>(null);
  const lastAppliedSyncTokenRef = useRef(0);
  const startYRef = useRef(0);
  const startHeightRef = useRef(320);
  const editorLanguage = resolveEditorLanguage(fileDiff, editorPreferences);

  useEffect(() => {
    if (visible) {
      api
        .initMergebuffer(fileDiff.filediff_id)
        .then((mb) => {
          setBuffer(mb);
          setMergedText(extractVirtualText(mb.merged_content_json));
          setSaveError(null);
        })
        .catch((error) => setSaveError(String(error)));
    }
  }, [visible, fileDiff.filediff_id]);

  useEffect(() => {
    if (!visible) {
      lastFocusedFileRef.current = null;
    }
  }, [visible]);

  useEffect(() => {
    if (!visible || !editorRef.current) return;
    if (lastFocusedFileRef.current === fileDiff.filediff_id) return;
    lastFocusedFileRef.current = fileDiff.filediff_id;
    withSuppressedScroll(suppressScrollSyncRef, () => {
      editorRef.current.revealLineInCenter(initialFocusLine);
    });
  }, [fileDiff.filediff_id, initialFocusLine, visible]);

  useEffect(() => {
    if (!visible || !editorRef.current || !syncedTopLine || syncToken === 0) return;
    if (lastAppliedSyncTokenRef.current === syncToken) return;
    lastAppliedSyncTokenRef.current = syncToken;
    withSuppressedScroll(suppressScrollSyncRef, () => {
      editorRef.current.revealLineNearTop(syncedTopLine);
    });
  }, [syncToken, syncedTopLine, visible]);

  useEffect(() => {
    if (!visible || !editorRef.current) return;
    mergeDecorationIdsRef.current = editorRef.current.deltaDecorations(
      mergeDecorationIdsRef.current,
      buildMergeDecorations(fileDiff.hunks_json, mergedText)
    );
  }, [fileDiff.hunks_json, mergedText, visible]);

  const handleSave = async () => {
    setSaveError(null);
    setIsSaving(true);
    try {
      await api.setMergebufferText(fileDiff.filediff_id, mergedText);
      await api.saveMergebuffer(fileDiff.filediff_id);
      setBuffer((prev) => (prev ? { ...prev, dirty: false } : prev));
      await onSaveComplete?.();
    } catch (e: any) {
      const message = e?.toString?.() ?? String(e);
      if (message.toLowerCase().includes("save as required")) {
        setSaveAsVisible(true);
      } else {
        setSaveError(`Save failed: ${message}`);
      }
    } finally {
      setIsSaving(false);
    }
  };

  const handleSaveAs = () => {
    setSaveError(null);
    setSaveAsVisible(true);
  };

  const submitSaveAs = async (values: Record<string, string>) => {
    const path = values.path.trim();
    if (!path) return;
    setIsSaving(true);
    try {
      await api.setMergebufferText(fileDiff.filediff_id, mergedText);
      await api.saveMergebufferAs(fileDiff.filediff_id, path);
      setBuffer((prev) => (prev ? { ...prev, dirty: false } : prev));
      setSaveAsVisible(false);
      await onSaveComplete?.();
    } catch (e) {
      setSaveError(`Save As failed: ${String(e)}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleFormat = async () => {
    if (!editorRef.current) return;
    try {
      await editorRef.current.getAction("editor.action.formatDocument")?.run();
    } catch (error) {
      setSaveError(`Format failed: ${String(error)}`);
    }
  };

  useEffect(() => {
    if (!isResizing) return;

    const onMove = (e: MouseEvent) => {
      const delta = startYRef.current - e.clientY;
      const parentHeight = panelRef.current?.parentElement?.clientHeight ?? window.innerHeight;
      const maxHeight = Math.max(220, parentHeight - 180);
      const next = Math.max(180, Math.min(maxHeight, startHeightRef.current + delta));
      setHeight(next);
    };

    const onUp = () => setIsResizing(false);

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [isResizing]);

  const beginResize = (e: React.MouseEvent<HTMLDivElement>) => {
    startYRef.current = e.clientY;
    startHeightRef.current = height;
    setIsResizing(true);
  };

  if (!visible) return null;

  return (
    <div ref={panelRef} className="merge-panel" style={{ height }}>
      <div className="merge-resizer" onMouseDown={beginResize} title="Drag to resize" />
      <div className="merge-toolbar">
        <span className="merge-title">Merged Output</span>
        <span className="merge-dirty">{buffer?.dirty ? "* unsaved" : ""}</span>
        <span className="merge-language">{editorLanguage}</span>
        <button type="button" onClick={handleFormat}>Format</button>
        <button
          type="button"
          onClick={() => setShowEditorSettings((current) => !current)}
        >
          <EditIcon />
          <span>Editor</span>
        </button>
        <button type="button" onClick={handleSave} disabled={isSaving}>
          {isSaving ? <LoadingIcon className="button-icon-spin" /> : <SaveIcon />}
          <span>{isSaving ? "Saving..." : "Save"}</span>
        </button>
        <button type="button" onClick={handleSaveAs} disabled={isSaving}>
          <SaveIcon />
          <span>Save As</span>
        </button>
        <button type="button" onClick={onToggle}>Close</button>
      </div>
      {saveError && <div className="merge-error">{saveError}</div>}
      {showEditorSettings && (
        <div className="editor-settings-panel">
          <label className="editor-settings-field">
            <span>Language</span>
            <select
              value={editorPreferences.languageOverride}
              onChange={(event) =>
                onEditorPreferencesChange((current) => ({
                  ...current,
                  languageOverride: event.target.value,
                }))
              }
            >
              {LANGUAGE_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <label className="editor-settings-field">
            <span>Tab size</span>
            <input
              type="number"
              min={2}
              max={8}
              value={editorPreferences.tabSize}
              onChange={(event) =>
                onEditorPreferencesChange((current) => ({
                  ...current,
                  tabSize: clampTabSize(event.target.value),
                }))
              }
            />
          </label>
          <label className="editor-settings-field editor-settings-checkbox">
            <input
              type="checkbox"
              checked={editorPreferences.insertSpaces}
              onChange={(event) =>
                onEditorPreferencesChange((current) => ({
                  ...current,
                  insertSpaces: event.target.checked,
                }))
              }
            />
            <span>Insert spaces</span>
          </label>
          <label className="editor-settings-field">
            <span>Word wrap</span>
            <select
              value={editorPreferences.wordWrap}
              onChange={(event) =>
                onEditorPreferencesChange((current) => ({
                  ...current,
                  wordWrap: event.target.value as EditorPreferences["wordWrap"],
                }))
              }
            >
              <option value="off">Off</option>
              <option value="on">On</option>
              <option value="bounded">Bounded</option>
            </select>
          </label>
        </div>
      )}
      <div className="merge-editor-wrap">
        <Editor
          height="100%"
          language={editorLanguage}
          theme={theme === "dark" ? "vs-dark" : "vs"}
          value={mergedText}
          onChange={(value) => setMergedText(value ?? "")}
          onMount={(editor) => {
            editorRef.current = editor;
            editor.onDidScrollChange(() => {
              if (suppressScrollSyncRef.current) return;
              const topLine = editor.getVisibleRanges()?.[0]?.startLineNumber ?? 1;
              if (topLine !== lastReportedTopLineRef.current) {
                lastReportedTopLineRef.current = topLine;
                onScrollLineChange?.(topLine);
              }
            });
          }}
          options={{
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            inlineSuggest: { enabled: true },
            suggest: { preview: true },
            tabSize: editorPreferences.tabSize,
            insertSpaces: editorPreferences.insertSpaces,
            wordWrap: editorPreferences.wordWrap,
          }}
        />
      </div>
      <FormDialog
        visible={saveAsVisible}
        title="Save Merged Output"
        description="Enter the target path for the merged file."
        confirmLabel="Save"
        onCancel={() => setSaveAsVisible(false)}
        onConfirm={submitSaveAs}
        fields={[
          {
            id: "path",
            label: "Path",
            defaultValue: suggestedSavePath(fileDiff),
            placeholder: "Path to save merged output",
            required: true,
          },
        ]}
      />
    </div>
  );
}

function withSuppressedScroll(flagRef: React.MutableRefObject<boolean>, fn: () => void) {
  flagRef.current = true;
  fn();
  window.requestAnimationFrame(() => {
    flagRef.current = false;
  });
}

function clampTabSize(value: string) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) return 2;
  return Math.min(8, Math.max(2, Math.round(numeric)));
}

function buildMergeDecorations(hunksJson: string, mergedText: string) {
  const lineCount = Math.max(1, mergedText.split("\n").length);
  return parseHunks(hunksJson).flatMap((hunk) => {
    const decorations: Array<{
      range: {
        startLineNumber: number;
        startColumn: number;
        endLineNumber: number;
        endColumn: number;
      };
      options: Record<string, unknown>;
    }> = [];

    let currentLine = hunk.new_start ?? 1;
    let pendingDeletionCount = 0;

    for (const line of hunk.lines ?? []) {
      if (line.kind === "context") {
        if (pendingDeletionCount > 0) {
          decorations.push(deletionMarkerDecoration(currentLine, lineCount, false));
          pendingDeletionCount = 0;
        }
        currentLine += 1;
        continue;
      }

      if (line.kind === "del") {
        pendingDeletionCount += 1;
        continue;
      }

      if (line.kind === "add") {
        decorations.push({
          range: {
            startLineNumber: clampLine(currentLine, lineCount),
            startColumn: 1,
            endLineNumber: clampLine(currentLine, lineCount),
            endColumn: 1,
          },
          options: {
            isWholeLine: true,
            className: "diff-line-add",
            linesDecorationsClassName: "diff-gutter-add",
          },
        });

        if (pendingDeletionCount > 0) {
          decorations.push(deletionMarkerDecoration(currentLine, lineCount, false));
          pendingDeletionCount = 0;
        }

        currentLine += 1;
      }
    }

    if (pendingDeletionCount > 0 || ((hunk.old_count ?? 0) > 0 && (hunk.new_count ?? 0) === 0)) {
      decorations.push(deletionMarkerDecoration(currentLine, lineCount, true));
    }

    return decorations;
  });
}

function deletionMarkerDecoration(
  lineNumber: number,
  lineCount: number,
  useAnchorBackground: boolean
) {
  const anchorLine = clampLine(lineNumber, lineCount);
  return {
    range: {
      startLineNumber: anchorLine,
      startColumn: 1,
      endLineNumber: anchorLine,
      endColumn: 1,
    },
    options: {
      isWholeLine: true,
      linesDecorationsClassName: "merge-line-del-marker",
      className: useAnchorBackground ? "diff-line-del-anchor" : undefined,
    },
  };
}

function clampLine(lineNumber: number, lineCount: number) {
  return Math.min(Math.max(lineNumber || 1, 1), lineCount);
}
