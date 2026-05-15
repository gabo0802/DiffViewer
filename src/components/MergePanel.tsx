import React, { useEffect, useRef, useState } from "react";
import Editor from "@monaco-editor/react";
import * as api from "../api";
import type { FileDiff, MergeBuffer } from "../types";

interface Props {
  fileDiff: FileDiff;
  visible: boolean;
  onToggle: () => void;
}

export default function MergePanel({ fileDiff, visible, onToggle }: Props) {
  const [buffer, setBuffer] = useState<MergeBuffer | null>(null);
  const [mergedText, setMergedText] = useState("");
  const [height, setHeight] = useState(320);
  const [isResizing, setIsResizing] = useState(false);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const startYRef = useRef(0);
  const startHeightRef = useRef(320);

  useEffect(() => {
    if (visible) {
      api
        .initMergebuffer(fileDiff.filediff_id)
        .then((mb) => {
          setBuffer(mb);
          setMergedText(extractText(mb.merged_content_json));
        })
        .catch(console.error);
    }
  }, [visible, fileDiff.filediff_id]);

  const handleSave = async () => {
    try {
      await api.setMergebufferText(fileDiff.filediff_id, mergedText);
      await api.saveMergebuffer(fileDiff.filediff_id);
      setBuffer((prev) => (prev ? { ...prev, dirty: false } : prev));
    } catch (e: any) {
      const message = e?.toString?.() ?? String(e);
      if (message.toLowerCase().includes("save as required")) {
        await handleSaveAs();
      } else {
        console.error(e);
        alert(`Save failed: ${message}`);
      }
    }
  };

  const handleSaveAs = async () => {
    const suggested = suggestedSavePath(fileDiff);
    const path = window.prompt("Save merged output as path:", suggested);
    if (!path) return;
    try {
      await api.setMergebufferText(fileDiff.filediff_id, mergedText);
      await api.saveMergebufferAs(fileDiff.filediff_id, path);
      setBuffer((prev) => (prev ? { ...prev, dirty: false } : prev));
    } catch (e) {
      console.error(e);
      alert(`Save As failed: ${String(e)}`);
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
        <button type="button" onClick={handleSave}>Save</button>
        <button type="button" onClick={handleSaveAs}>Save As</button>
        <button type="button" onClick={onToggle}>Close</button>
      </div>
      <div className="merge-editor-wrap">
        <Editor
          height="100%"
          defaultLanguage="text"
          value={mergedText}
          onChange={(value) => setMergedText(value ?? "")}
          options={{
            minimap: { enabled: false },
            scrollBeyondLastLine: false,
            inlineSuggest: { enabled: true },
            suggest: { preview: true },
          }}
        />
      </div>
    </div>
  );
}

function extractText(json: string): string {
  try {
    const parsed = JSON.parse(json);
    return parsed.text ?? "";
  } catch {
    return "";
  }
}

function suggestedSavePath(fileDiff: FileDiff): string {
  try {
    const parsed = JSON.parse(fileDiff.write_target_json);
    if (parsed?.type === "path" && typeof parsed.path === "string") {
      return parsed.path;
    }
  } catch {
    // Ignore and fall back to display path.
  }
  return fileDiff.display_path || "merged-output.txt";
}
