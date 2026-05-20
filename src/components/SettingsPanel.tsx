import React, { useMemo, useState } from "react";

import {
  FILE_FORMAT_OPTIONS,
  LANGUAGE_OPTIONS,
  normalizeExtensionKey,
  type EditorPreferences,
} from "../editorPreferences";
import { CloseIcon, PlusIcon } from "./Icons";

const SHORTCUTS = [
  { keys: "Ctrl + Click", action: "Select multiple diff tabs without switching focus" },
  { keys: "Ctrl + Tab / T", action: "Switch to the next diff tab" },
  { keys: "Ctrl + D", action: "Close the current diff tab" },
  { keys: "Ctrl + R", action: "Refresh the sidebar diff list" },
  { keys: "Ctrl + 1", action: "Jump to the previous visible change block" },
  { keys: "Ctrl + 2", action: "Jump to the next visible change block" },
  { keys: "Ctrl + E", action: "Toggle the merge editor for the active diff" },
];

interface Props {
  editorPreferences: EditorPreferences;
  onEditorPreferencesChange: React.Dispatch<React.SetStateAction<EditorPreferences>>;
}

type SyntaxOverrideRow = {
  id: string;
  extension: string;
  language: string;
};

export default function SettingsPanel({
  editorPreferences,
  onEditorPreferencesChange,
}: Props) {
  const [overrideRows, setOverrideRows] = useState(() =>
    rowsFromOverrides(editorPreferences.extensionLanguageOverrides)
  );

  const syntaxLanguages = useMemo(
    () => LANGUAGE_OPTIONS.filter((option) => option.value !== "auto"),
    []
  );

  const syncOverrides = (nextRows: SyntaxOverrideRow[]) => {
    setOverrideRows(nextRows);
    onEditorPreferencesChange((current) => ({
      ...current,
      extensionLanguageOverrides: rowsToOverrides(nextRows),
    }));
  };

  const addOverrideRow = () => {
    syncOverrides([
      ...overrideRows,
      {
        id: createRowId(),
        extension: "",
        language: "plaintext",
      },
    ]);
  };

  const updateOverrideRow = (
    rowId: string,
    field: keyof Omit<SyntaxOverrideRow, "id">,
    value: string
  ) => {
    syncOverrides(
      overrideRows.map((row) =>
        row.id === rowId
          ? {
              ...row,
              [field]: value,
            }
          : row
      )
    );
  };

  const removeOverrideRow = (rowId: string) => {
    syncOverrides(overrideRows.filter((row) => row.id !== rowId));
  };

  return (
    <div className="settings-panel">
      <div className="settings-header">
        <div>
          <h2 className="settings-title">Settings</h2>
          <p className="settings-description">
            Keyboard shortcuts are fixed for now. Syntax overrides apply when the editor language
            is set to auto detect.
          </p>
        </div>
      </div>

      <section className="settings-section">
        <div className="settings-section-header">
          <h3>Shortcuts</h3>
          <span className="settings-section-helper">Display only</span>
        </div>
        <div className="settings-shortcut-list">
          {SHORTCUTS.map((shortcut) => (
            <div key={shortcut.keys} className="settings-shortcut-row">
              <span className="settings-shortcut-keys">{shortcut.keys}</span>
              <span className="settings-shortcut-action">{shortcut.action}</span>
            </div>
          ))}
        </div>
      </section>

      <section className="settings-section">
        <div className="settings-section-header">
          <h3>Custom Syntax Highlighting</h3>
          <span className="settings-section-helper">Pre-set Monaco languages</span>
        </div>

        {overrideRows.length === 0 ? (
          <div className="settings-empty">
            No custom file-format mappings yet. Use the plus button to add an extension like
            `log`, `tmpl`, or `shader` and assign one of the built-in syntax highlighters.
          </div>
        ) : (
          <div className="settings-override-list">
            {overrideRows.map((row) => (
              <div key={row.id} className="settings-override-row">
                <label className="settings-field">
                  <span>File format</span>
                  <input
                    type="text"
                    list="diffviewer-file-formats"
                    value={row.extension}
                    onChange={(event) => updateOverrideRow(row.id, "extension", event.target.value)}
                    placeholder="log"
                  />
                </label>
                <label className="settings-field">
                  <span>Syntax highlighter</span>
                  <select
                    value={row.language}
                    onChange={(event) => updateOverrideRow(row.id, "language", event.target.value)}
                  >
                    {syntaxLanguages.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  type="button"
                  className="icon-button icon-button-square"
                  title={`Remove ${row.extension || "empty"} mapping`}
                  onClick={() => removeOverrideRow(row.id)}
                >
                  <CloseIcon />
                </button>
              </div>
            ))}
          </div>
        )}

        <button
          type="button"
          className="toolbar-button toolbar-button-with-icon settings-add-button"
          onClick={addOverrideRow}
        >
          <PlusIcon />
          <span>Add Mapping</span>
        </button>

        <datalist id="diffviewer-file-formats">
          {FILE_FORMAT_OPTIONS.map((option) => (
            <option key={option.extension} value={option.extension}>
              {option.extension}
            </option>
          ))}
        </datalist>

        {overrideRows.length > 0 && (
          <div className="settings-helper-block">
            Empty file-format rows are ignored until you type an extension. If two rows use the
            same extension, the last one in the list wins.
          </div>
        )}

        <div className="settings-helper-block">
          Stretch goal: custom tokenizer uploads are not wired yet, but this tab now gives us a
          stable place to add them later.
        </div>
      </section>
    </div>
  );
}

function rowsFromOverrides(overrides: Record<string, string>) {
  return Object.entries(overrides)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([extension, language]) => ({
      id: createRowId(),
      extension,
      language,
    }));
}

function rowsToOverrides(rows: SyntaxOverrideRow[]) {
  return Object.fromEntries(
    rows.flatMap((row) => {
      const extension = normalizeExtensionKey(row.extension);
      if (!extension) {
        return [];
      }
      return [[extension, row.language]];
    })
  );
}

function createRowId() {
  return `syntax-row-${Math.random().toString(36).slice(2, 10)}`;
}
