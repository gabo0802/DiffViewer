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
  { keys: "Ctrl + Tab", action: "Switch to the next diff tab" },
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

export default function SettingsPanel({
  editorPreferences,
  onEditorPreferencesChange,
}: Props) {
  const [newExtension, setNewExtension] = useState("");
  const [newLanguage, setNewLanguage] = useState("plaintext");

  const syntaxLanguages = useMemo(
    () => LANGUAGE_OPTIONS.filter((option) => option.value !== "auto"),
    []
  );

  const overrideEntries = useMemo(
    () =>
      Object.entries(editorPreferences.extensionLanguageOverrides).sort(([left], [right]) =>
        left.localeCompare(right)
      ),
    [editorPreferences.extensionLanguageOverrides]
  );

  const addOverride = () => {
    const extension = normalizeExtensionKey(newExtension);
    if (!extension) return;

    onEditorPreferencesChange((current) => ({
      ...current,
      extensionLanguageOverrides: {
        ...current.extensionLanguageOverrides,
        [extension]: newLanguage,
      },
    }));
    setNewExtension("");
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

        <div className="settings-syntax-add">
          <label className="settings-field">
            <span>File format</span>
            <input
              type="text"
              list="diffviewer-file-formats"
              value={newExtension}
              onChange={(event) => setNewExtension(event.target.value)}
              placeholder="log"
            />
          </label>
          <label className="settings-field">
            <span>Syntax highlighter</span>
            <select value={newLanguage} onChange={(event) => setNewLanguage(event.target.value)}>
              {syntaxLanguages.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className="toolbar-button toolbar-button-with-icon"
            onClick={addOverride}
            disabled={!normalizeExtensionKey(newExtension)}
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
        </div>

        {overrideEntries.length === 0 ? (
          <div className="settings-empty">
            No custom file-format mappings yet. Add an extension like `log`, `tmpl`, or `shader`
            and assign one of the built-in syntax highlighters.
          </div>
        ) : (
          <div className="settings-override-list">
            {overrideEntries.map(([extension, language]) => (
              <div key={extension} className="settings-override-row">
                <span className="settings-extension-pill">.{extension}</span>
                <select
                  value={language}
                  onChange={(event) =>
                    onEditorPreferencesChange((current) => ({
                      ...current,
                      extensionLanguageOverrides: {
                        ...current.extensionLanguageOverrides,
                        [extension]: event.target.value,
                      },
                    }))
                  }
                >
                  {syntaxLanguages.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  className="icon-button icon-button-square"
                  title={`Remove .${extension} mapping`}
                  onClick={() =>
                    onEditorPreferencesChange((current) => {
                      const nextOverrides = { ...current.extensionLanguageOverrides };
                      delete nextOverrides[extension];
                      return {
                        ...current,
                        extensionLanguageOverrides: nextOverrides,
                      };
                    })
                  }
                >
                  <CloseIcon />
                </button>
              </div>
            ))}
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
