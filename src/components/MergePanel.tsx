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
      if (e?.toString?.().includes("save_as_required")) {
        await handleSaveAs();
      } else {
        console.error(e);
      }
    }
  };

  const handleSaveAs = async () => {
    const suggested = fileDiff.display_path || "merged-output.txt";
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
      const next = Math.max(180, Math.min(700, startHeightRef.current + delta));
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
    <div className="merge-panel" style={{ height }}>
      <div className="merge-resizer" onMouseDown={beginResize} title="Drag to resize" />
      <div className="merge-toolbar">
        <span className="merge-title">Merged Output</span>
        <span className="merge-dirty">{buffer?.dirty ? "● unsaved" : ""}</span>
        <button onClick={handleSave}>Save</button>
        <button onClick={handleSaveAs}>Save As</button>
        <button onClick={onToggle}>Close</button>
      </div>
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
