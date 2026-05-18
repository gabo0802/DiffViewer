import { useMemo, useState } from "react";

import { getFirstChangedMergeLine, isEditableFileDiff } from "../diffDomain";
import { buildDisambiguatedPathLabels } from "../pathLabels";
import type { FileDiff } from "../types";

export function useDiffTabs(onLastTabClosed?: () => void) {
  const [tabs, setTabs] = useState<FileDiff[]>([]);
  const [activeTab, setActiveTab] = useState<string | null>(null);

  const openFileDiff = (fileDiff: FileDiff) => {
    setTabs((current) =>
      current.some((tab) => tab.filediff_id === fileDiff.filediff_id)
        ? current
        : [...current, fileDiff]
    );
    setActiveTab(fileDiff.filediff_id);
  };

  const closeTab = (id: string) => {
    setTabs((current) => {
      const closedIndex = current.findIndex((tab) => tab.filediff_id === id);
      const nextTabs = current.filter((tab) => tab.filediff_id !== id);
      if (activeTab === id) {
        const fallbackTab =
          nextTabs[closedIndex] ?? nextTabs[Math.max(0, closedIndex - 1)] ?? null;
        setActiveTab(fallbackTab?.filediff_id ?? null);
        if (!fallbackTab) onLastTabClosed?.();
      }
      return nextTabs;
    });
  };

  const currentFileDiff = tabs.find((tab) => tab.filediff_id === activeTab) ?? null;
  const tabLabels = useMemo(
    () =>
      buildDisambiguatedPathLabels(
        tabs.map((tab) => ({ id: tab.filediff_id, path: tab.display_path }))
      ),
    [tabs]
  );

  return {
    tabs,
    setTabs,
    activeTab,
    setActiveTab,
    currentFileDiff,
    tabLabels,
    openFileDiff,
    closeTab,
    firstChangedMergeLine: currentFileDiff ? getFirstChangedMergeLine(currentFileDiff) : 1,
    canEditCurrent: currentFileDiff ? isEditableFileDiff(currentFileDiff) : false,
  };
}
