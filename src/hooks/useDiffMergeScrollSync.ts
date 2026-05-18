import { useCallback, useEffect, useState } from "react";

import type { RenderedDiffModel } from "../types";

type ScrollSyncSignal = {
  source: "diff" | "merge";
  diffTopRow: number;
  mergeTopLine: number;
  token: number;
};

export function useDiffMergeScrollSync(activeFileDiffId?: string | null) {
  const [renderedModel, setRenderedModel] = useState<RenderedDiffModel | null>(null);
  const [syncSignal, setSyncSignal] = useState<ScrollSyncSignal | null>(null);

  useEffect(() => {
    setRenderedModel(null);
    setSyncSignal(null);
  }, [activeFileDiffId]);

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

  return {
    renderedModel,
    setRenderedModel,
    syncSignal,
    handleDiffScroll,
    handleMergeScroll,
  };
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

function clamp(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}
