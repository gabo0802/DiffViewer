import React, { useEffect, useState, useCallback } from "react";
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
        .filter((r) => r.left && r.left.kind !== "empty")
        .map((r) => r.left!.text)
        .join("\n")
    : "";

  const rightText = model
    ? model.rows
        .filter((r) => r.right && r.right.kind !== "empty")
        .map((r) => r.right!.text)
        .join("\n")
    : "";

  const leftDecorations = model
    ? buildDecorations(model.rows, "left")
    : [];

  const rightDecorations = model
    ? buildDecorations(model.rows, "right")
    : [];

  const goHunk = useCallback(
    (dir: 1 | -1) => {
      if (!model) return;
      setCurrentHunkIdx((prev) => {
        const next = prev + dir;
        if (next < 0) return model.hunks.length - 1;
        if (next >= model.hunks.length) return 0;
        return next;
      });
    },
    [model]
  );

  return (
    <div className="diff-viewer">
      <div className="diff-toolbar">
        <span className="diff-path">{fileDiff.display_path}</span>
        <span className="diff-status badge">{fileDiff.status}</span>
        <span className="diff-nav">
          <button onClick={() => goHunk(-1)} title="Previous hunk">
            ↑ Prev
          </button>
          <span>
            {model
              ? `Hunk ${currentHunkIdx + 1}/${model.hunks.length}`
              : "–"}
          </span>
          <button onClick={() => goHunk(1)} title="Next hunk">
            Next ↓
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
            height="100%"
            defaultLanguage="text"
            value={leftText}
            options={{ readOnly: true, minimap: { enabled: false }, lineNumbers: "on", scrollBeyondLastLine: false }}
          />
        </div>
        <div className="diff-editor-col">
          <div className="editor-label">{fileDiff.right_label || "Right"}</div>
          <Editor
            height="100%"
            defaultLanguage="text"
            value={rightText}
            options={{ readOnly: true, minimap: { enabled: false }, lineNumbers: "on", scrollBeyondLastLine: false }}
          />
        </div>
          </>
        )}
      </div>
    </div>
  );
}

function buildDecorations(rows: AlignmentRow[], side: "left" | "right") {
  // Placeholder for Monaco decorations — will be wired after MVP
  return [];
}
