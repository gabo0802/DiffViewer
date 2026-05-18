import type { FileDiff } from "./types";

export type ParsedHunk = {
  lines?: Array<{ kind: "context" | "add" | "del"; text: string }>;
  old_count?: number;
  new_start?: number;
  new_count?: number;
};

export type ContentSource =
  | { type: "virtual"; text: string }
  | { type: "path"; path: string }
  | { type: "snapshot"; snapshot_id: string };

export type WriteTarget =
  | { type: "path"; path: string }
  | { type: "save_as_required" }
  | { type: "read_only" };

export function parseHunks(hunksJson: string): ParsedHunk[] {
  return parseJsonArray<ParsedHunk>(hunksJson);
}

export function parseContentSource(json: string): ContentSource | null {
  const parsed = parseJsonObject(json);
  if (!parsed || typeof parsed.type !== "string") return null;
  if (parsed.type === "virtual" && typeof parsed.text === "string") {
    return { type: "virtual", text: parsed.text };
  }
  if (parsed.type === "path" && typeof parsed.path === "string") {
    return { type: "path", path: parsed.path };
  }
  if (parsed.type === "snapshot" && typeof parsed.snapshot_id === "string") {
    return { type: "snapshot", snapshot_id: parsed.snapshot_id };
  }
  return null;
}

export function parseWriteTarget(json: string): WriteTarget | null {
  const parsed = parseJsonObject(json);
  if (!parsed || typeof parsed.type !== "string") return null;
  if (parsed.type === "path" && typeof parsed.path === "string") {
    return { type: "path", path: parsed.path };
  }
  if (parsed.type === "save_as_required") return { type: "save_as_required" };
  if (parsed.type === "read_only") return { type: "read_only" };
  return null;
}

export function extractVirtualText(json: string): string {
  const source = parseContentSource(json);
  return source?.type === "virtual" ? source.text : "";
}

export function isEditableFileDiff(fileDiff: FileDiff) {
  return parseWriteTarget(fileDiff.write_target_json)?.type !== "read_only";
}

export function getFirstChangedMergeLine(fileDiff: FileDiff) {
  const hunks = parseHunks(fileDiff.hunks_json);
  return Math.max(1, hunks[0]?.new_start ?? 1);
}

export function suggestedSavePath(fileDiff: FileDiff): string {
  const target = parseWriteTarget(fileDiff.write_target_json);
  if (target?.type === "path") return target.path;
  return fileDiff.display_path || "merged-output.txt";
}

function parseJsonObject(json: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(json);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function parseJsonArray<T>(json: string): T[] {
  try {
    const parsed = JSON.parse(json);
    return Array.isArray(parsed) ? (parsed as T[]) : [];
  } catch {
    return [];
  }
}
