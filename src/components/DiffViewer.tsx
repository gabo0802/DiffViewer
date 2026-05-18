import React, { useCallback, useEffect, useRef, useState } from "react";
import Editor from "@monaco-editor/react";
import * as api from "../api";
import { resolveEditorLanguage, type EditorPreferences } from "../editorPreferences";
import type { FileDiff, RenderedDiffModel, AlignmentRow } from "../types";

interface Props {
  fileDiff: FileDiff;
  displayLabel?: string;
  editorPreferences: EditorPreferences;
  theme: "dark" | "light";
  onModelChange?: (model: RenderedDiffModel | null) => void;
  onScrollRowChange?: (topRow: number) => void;
  syncedTopRow?: number | null;
  syncToken?: number;
}

export default function DiffViewer({
  fileDiff,
  displayLabel,
  editorPreferences,
  theme,
  onModelChange,
  onScrollRowChange,
  syncedTopRow = null,
  syncToken = 0,
}: Props) {
  const [model, setModel] = useState<RenderedDiffModel | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [currentHunkIdx, setCurrentHunkIdx] = useState(0);
  const leftEditorRef = useRef<any>(null);
  const rightEditorRef = useRef<any>(null);
  const monacoRef = useRef<any>(null);
  const leftDecorationIdsRef = useRef<string[]>([]);
  const rightDecorationIdsRef = useRef<string[]>([]);
  const suppressScrollSyncRef = useRef(false);
  const lastReportedTopRowRef = useRef(1);
  const lastAppliedSyncTokenRef = useRef(0);
  const editorLanguage = resolveEditorLanguage(fileDiff, editorPreferences);
  const showSinglePane = isAddedFileDiff(fileDiff);

  useEffect(() => {
    setModel(null);
    onModelChange?.(null);
    setLoadError(null);
    api
      .getRenderedDiff(fileDiff.filediff_id)
      .then((nextModel) => {
        setModel(nextModel);
        onModelChange?.(nextModel);
      })
      .catch((err) => {
        console.error(err);
        setLoadError(String(err));
      });
    setCurrentHunkIdx(0);
  }, [fileDiff.filediff_id, onModelChange]);

  const leftText = model
    ? model.rows
        .map((r) => (r.left && r.left.kind !== "empty" ? r.left.text : ""))
        .join("\n")
    : "";

  const rightText = model
    ? model.rows
        .map((r) => (r.right && r.right.kind !== "empty" ? r.right.text : ""))
        .join("\n")
    : "";

  const goHunk = useCallback(
    (dir: 1 | -1) => {
      if (!model || model.hunks.length === 0) return;
      setCurrentHunkIdx((prev) => {
        const next = prev + dir;
        const wrapped =
          next < 0 ? model.hunks.length - 1 : next >= model.hunks.length ? 0 : next;
        if (wrapped === prev) {
          revealAlignedLine(model.hunks[wrapped].start_row + 1);
        }
        return wrapped;
      });
    },
    [model]
  );

  useEffect(() => {
    if (!model || model.hunks.length === 0) return;
    const lineNumber = model.hunks[currentHunkIdx].start_row + 1;
    revealAlignedLine(lineNumber);
  }, [model, currentHunkIdx]);

  useEffect(() => {
    if (!model || !monacoRef.current) return;
    applyDecorations(model);
  }, [model]);

  useEffect(() => {
    if (!syncedTopRow || !model || syncToken === 0) return;
    if (lastAppliedSyncTokenRef.current === syncToken) return;
    lastAppliedSyncTokenRef.current = syncToken;
    withSuppressedScroll(suppressScrollSyncRef, () => {
      revealAlignedLine(syncedTopRow, "nearTop");
    });
  }, [model, syncToken, syncedTopRow]);

  const decorateOnMount = (side: "left" | "right") => (editor: any, monaco: any) => {
    monacoRef.current = monaco;
    if (side === "left") leftEditorRef.current = editor;
    if (side === "right") rightEditorRef.current = editor;
    if (model) {
      applyDecorations(model);
    }

    editor.onDidScrollChange(() => {
      if (suppressScrollSyncRef.current) return;
      const otherEditor = side === "left" ? rightEditorRef.current : leftEditorRef.current;
      if (otherEditor) {
        withSuppressedScroll(suppressScrollSyncRef, () => {
          otherEditor.setScrollTop(editor.getScrollTop());
          otherEditor.setScrollLeft(editor.getScrollLeft());
        });
      }

      const topRow = editor.getVisibleRanges()?.[0]?.startLineNumber ?? 1;
      if (topRow !== lastReportedTopRowRef.current) {
        lastReportedTopRowRef.current = topRow;
        onScrollRowChange?.(topRow);
      }
    });
  };

  const revealAlignedLine = (lineNumber: number, position: "center" | "nearTop" = "center") => {
    const editors = [leftEditorRef.current, rightEditorRef.current].filter(Boolean);
    for (const editor of editors) {
      if (position === "nearTop") {
        editor.revealLineNearTop(lineNumber);
      } else {
        editor.revealLineInCenter(lineNumber);
      }
    }
  };

  const applyDecorations = (nextModel: RenderedDiffModel) => {
    if (!monacoRef.current) return;
    if (leftEditorRef.current) {
      leftDecorationIdsRef.current = leftEditorRef.current.deltaDecorations(
        leftDecorationIdsRef.current,
        buildDecorations(nextModel.rows, "left", monacoRef.current)
      );
    }
    if (rightEditorRef.current) {
      rightDecorationIdsRef.current = rightEditorRef.current.deltaDecorations(
        rightDecorationIdsRef.current,
        buildDecorations(nextModel.rows, "right", monacoRef.current)
      );
    }
  };

  return (
    <div className="diff-viewer">
      <div className="diff-toolbar">
        <span className="diff-path" title={fileDiff.display_path}>
          {displayLabel ?? fileDiff.display_path}
        </span>
        <span className="diff-status badge">{fileDiff.status}</span>
        <span className="diff-nav">
          <button onClick={() => goHunk(-1)} title="Previous hunk">
            Prev
          </button>
          <span>
            {model
              ? `Hunk ${model.hunks.length === 0 ? 0 : currentHunkIdx + 1}/${model.hunks.length}`
              : "-"}
          </span>
          <button onClick={() => goHunk(1)} title="Next hunk">
            Next
          </button>
        </span>
      </div>

      <div className="diff-editors">
        {loadError && (
          <div className="empty-state" style={{ width: "100%" }}>
            Failed to render diff: {loadError}
          </div>
        )}
        {!loadError && model && model.rows.length === 0 && (
          <div className="empty-state" style={{ width: "100%" }}>
            No renderable lines for this file diff (possible metadata-only or binary change).
          </div>
        )}
        {!loadError && !model && (
          <div className="empty-state" style={{ width: "100%" }}>
            Loading diff...
          </div>
        )}

        {!loadError && model && model.rows.length > 0 && (
          <>
            {!showSinglePane && (
              <div className="diff-editor-col">
                <div className="editor-label">{fileDiff.left_label || "Left"}</div>
                <Editor
                  key={`left-${fileDiff.filediff_id}-${model.rows.length}`}
                  height="100%"
                  language={editorLanguage}
                  theme={theme === "dark" ? "vs-dark" : "vs"}
                  value={leftText}
                  onMount={decorateOnMount("left")}
                  options={{
                    readOnly: true,
                    minimap: { enabled: false },
                    lineNumbers: "on",
                    scrollBeyondLastLine: false,
                    renderLineHighlight: "none",
                    tabSize: editorPreferences.tabSize,
                    insertSpaces: editorPreferences.insertSpaces,
                    wordWrap: editorPreferences.wordWrap,
                  }}
                />
              </div>
            )}
            <div className={`diff-editor-col ${showSinglePane ? "diff-editor-col-full" : ""}`}>
              <div className="editor-label">{fileDiff.right_label || "Right"}</div>
              <Editor
                key={`right-${fileDiff.filediff_id}-${model.rows.length}`}
                height="100%"
                language={editorLanguage}
                theme={theme === "dark" ? "vs-dark" : "vs"}
                value={rightText}
                onMount={decorateOnMount("right")}
                options={{
                  readOnly: true,
                  minimap: { enabled: false },
                  lineNumbers: "on",
                  scrollBeyondLastLine: false,
                  renderLineHighlight: "none",
                  tabSize: editorPreferences.tabSize,
                  insertSpaces: editorPreferences.insertSpaces,
                  wordWrap: editorPreferences.wordWrap,
                }}
              />
            </div>
          </>
        )}
      </div>
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

function buildDecorations(rows: AlignmentRow[], side: "left" | "right", monaco: any) {
  return rows
    .map((row, index) => {
      const className = decorationClassForRow(row, side);
      if (!className) return null;
      return {
        range: new monaco.Range(index + 1, 1, index + 1, 1),
        options: {
          isWholeLine: true,
          className,
          linesDecorationsClassName: gutterClassForRow(row, side),
        },
      };
    })
    .filter(Boolean);
}

function decorationClassForRow(row: AlignmentRow, side: "left" | "right") {
  const cell = side === "left" ? row.left : row.right;
  const kind = cell?.kind;
  if (!kind || kind === "context") return null;

  if (side === "right" && kind === "empty" && row.left?.kind === "del") {
    return "diff-line-del-empty";
  }

  return `diff-line-${kind}`;
}

function gutterClassForRow(row: AlignmentRow, side: "left" | "right") {
  const cell = side === "left" ? row.left : row.right;
  const kind = cell?.kind;

  if (side === "right" && kind === "empty" && row.left?.kind === "del") {
    return "diff-gutter-del";
  }
  if (kind === "add") return "diff-gutter-add";
  if (kind === "del") return "diff-gutter-del";
  return undefined;
}

function isAddedFileDiff(fileDiff: FileDiff) {
  const status = fileDiff.status.toLowerCase();
  return status === "add" || status === "added";
}
