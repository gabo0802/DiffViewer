import type { FileDiff } from "./types";

export type WordWrapMode = "off" | "on" | "bounded";

export type EditorPreferences = {
  languageOverride: string;
  tabSize: number;
  insertSpaces: boolean;
  wordWrap: WordWrapMode;
};

export const DEFAULT_EDITOR_PREFERENCES: EditorPreferences = {
  languageOverride: "auto",
  tabSize: 2,
  insertSpaces: true,
  wordWrap: "off",
};

export const LANGUAGE_OPTIONS = [
  { value: "auto", label: "Auto detect" },
  { value: "plaintext", label: "Plain text" },
  { value: "typescript", label: "TypeScript" },
  { value: "javascript", label: "JavaScript" },
  { value: "json", label: "JSON" },
  { value: "rust", label: "Rust" },
  { value: "python", label: "Python" },
  { value: "go", label: "Go" },
  { value: "cpp", label: "C++" },
  { value: "csharp", label: "C#" },
  { value: "java", label: "Java" },
  { value: "html", label: "HTML" },
  { value: "css", label: "CSS" },
  { value: "scss", label: "SCSS" },
  { value: "markdown", label: "Markdown" },
  { value: "yaml", label: "YAML" },
  { value: "xml", label: "XML" },
  { value: "shell", label: "Shell" },
  { value: "powershell", label: "PowerShell" },
  { value: "sql", label: "SQL" },
  { value: "toml", label: "TOML" },
];

const EXTENSION_LANGUAGE_MAP: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  mts: "typescript",
  cts: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  rs: "rust",
  py: "python",
  go: "go",
  c: "c",
  h: "c",
  cc: "cpp",
  cpp: "cpp",
  cxx: "cpp",
  hpp: "cpp",
  cs: "csharp",
  java: "java",
  html: "html",
  htm: "html",
  css: "css",
  scss: "scss",
  less: "less",
  md: "markdown",
  markdown: "markdown",
  yml: "yaml",
  yaml: "yaml",
  xml: "xml",
  svg: "xml",
  sh: "shell",
  bash: "shell",
  zsh: "shell",
  ps1: "powershell",
  sql: "sql",
  toml: "toml",
};

export function loadEditorPreferences(storageKey: string) {
  const raw = window.localStorage.getItem(storageKey);
  if (!raw) return DEFAULT_EDITOR_PREFERENCES;

  try {
    const parsed = JSON.parse(raw) as Partial<EditorPreferences>;
    return {
      languageOverride:
        typeof parsed.languageOverride === "string"
          ? parsed.languageOverride
          : DEFAULT_EDITOR_PREFERENCES.languageOverride,
      tabSize: clampTabSize(parsed.tabSize),
      insertSpaces:
        typeof parsed.insertSpaces === "boolean"
          ? parsed.insertSpaces
          : DEFAULT_EDITOR_PREFERENCES.insertSpaces,
      wordWrap: normalizeWordWrap(parsed.wordWrap),
    };
  } catch {
    return DEFAULT_EDITOR_PREFERENCES;
  }
}

export function resolveEditorLanguage(fileDiff: FileDiff, preferences: EditorPreferences) {
  if (preferences.languageOverride !== "auto") {
    return preferences.languageOverride;
  }

  const extension = extensionForPath(fileDiff.display_path);
  return EXTENSION_LANGUAGE_MAP[extension] ?? "plaintext";
}

function extensionForPath(path: string) {
  const normalized = path.split(/[\\/]/).pop() ?? path;
  const parts = normalized.toLowerCase().split(".");
  return parts.length > 1 ? parts.pop() ?? "" : "";
}

function clampTabSize(value: unknown) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return DEFAULT_EDITOR_PREFERENCES.tabSize;
  }
  return Math.min(8, Math.max(2, Math.round(numeric)));
}

function normalizeWordWrap(value: unknown): WordWrapMode {
  return value === "on" || value === "bounded" ? value : "off";
}
