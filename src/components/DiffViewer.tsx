import React, { useCallback, useEffect, useRef, useState } from "react";
import Editor from "@monaco-editor/react";
import * as api from "../api";
import type { FileDiff, RenderedDiffModel, AlignmentRow } from "../types";

interface Props {
  fileDiff: FileDiff;
}

export default function DiffViewer({ fileDiff }: Props) {
  const [model, setModel] = useState<RenderedDiffModel | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [currentHunkIdx, setCurrentHunkIdx] = useState(0);
  const leftEditorRef = useRef<any>(null);
  const rightEditorRef = useRef<any>(null);

  useEffect(() => {
    setModel(null);
    setLoadError(null);
    api
      .getRenderedDiff(fileDiff.filediff_id)
      .then(setModel)
      .catch((err) => {
        console.error(err);
        setLoadError(String(err));
      });
    setCurrentHunkIdx(0);
  }, [fileDiff.filediff_id]);

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
        if (next < 0) return model.hunks.length - 1;
        if (next >= model.hunks.length) return 0;
        return next;
      });
    },
    [model]
  );

  useEffect(() => {
    if (!model || model.hunks.length === 0) return;
    const lineNumber = model.hunks[currentHunkIdx].start_row + 1;
    leftEditorRef.current?.revealLineInCenter(lineNumber);
    rightEditorRef.current?.revealLineInCenter(lineNumber);
  }, [model, currentHunkIdx]);

  const decorateOnMount = (side: "left" | "right") => (editor: any, monaco: any) => {
    if (side === "left") leftEditorRef.current = editor;
    if (side === "right") rightEditorRef.current = editor;
    if (model) {
      editor.deltaDecorations([], buildDecorations(model.rows, side, monaco));
    }
  };

  return (
    <div className="diff-viewer">
      <div className="diff-toolbar">
        <span className="diff-path">{fileDiff.display_path}</span>
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
            <div className="diff-editor-col">
              <div className="editor-label">{fileDiff.left_label || "Left"}</div>
              <Editor
                key={`left-${fileDiff.filediff_id}-${model.rows.length}`}
                height="100%"
                defaultLanguage="text"
                value={leftText}
                onMount={decorateOnMount("left")}
                options={{
                  readOnly: true,
                  minimap: { enabled: false },
                  lineNumbers: "on",
                  scrollBeyondLastLine: false,
                  renderLineHighlight: "none",
                }}
              />
            </div>
            <div className="diff-editor-col">
              <div className="editor-label">{fileDiff.right_label || "Right"}</div>
              <Editor
                key={`right-${fileDiff.filediff_id}-${model.rows.length}`}
                height="100%"
                defaultLanguage="text"
                value={rightText}
                onMount={decorateOnMount("right")}
                options={{
                  readOnly: true,
                  minimap: { enabled: false },
                  lineNumbers: "on",
                  scrollBeyondLastLine: false,
                  renderLineHighlight: "none",
                }}
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function buildDecorations(rows: AlignmentRow[], side: "left" | "right", monaco: any) {
  return rows
    .map((row, index) => {
      const cell = side === "left" ? row.left : row.right;
      const kind = cell?.kind;
      if (!kind || kind === "context") return null;
      return {
        range: new monaco.Range(index + 1, 1, index + 1, 1),
        options: {
          isWholeLine: true,
          className: `diff-line-${kind}`,
        },
      };
    })
    .filter(Boolean);
}
