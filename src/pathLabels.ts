type PathLabelEntry = {
  id: string;
  path: string;
};

type ParsedPath = {
  original: string;
  separator: string;
  segments: string[];
};

export function buildDisambiguatedPathLabels(
  entries: PathLabelEntry[]
): Record<string, string> {
  const parsedEntries = entries.map((entry) => ({
    ...entry,
    parsed: parsePath(entry.path),
  }));
  const labels = new Map<string, string>();

  for (const entry of parsedEntries) {
    labels.set(entry.id, entry.parsed.segments.at(-1) || entry.path);
  }

  let changed = true;
  while (changed) {
    changed = false;
    const groups = groupByLabel(parsedEntries, labels);

    for (const group of groups) {
      if (group.length < 2) continue;

      for (const entry of group) {
        const currentLabel = labels.get(entry.id) || entry.path;
        const nextLabel = expandLabel(entry.parsed, currentLabel);
        if (nextLabel !== currentLabel) {
          labels.set(entry.id, nextLabel);
          changed = true;
        }
      }
    }
  }

  for (const group of groupByLabel(parsedEntries, labels)) {
    if (group.length < 2) continue;
    for (const entry of group) {
      labels.set(entry.id, entry.path);
    }
  }

  return Object.fromEntries(labels);
}

function groupByLabel(
  entries: Array<PathLabelEntry & { parsed: ParsedPath }>,
  labels: Map<string, string>
) {
  const grouped = new Map<string, Array<PathLabelEntry & { parsed: ParsedPath }>>();

  for (const entry of entries) {
    const label = labels.get(entry.id) || entry.path;
    grouped.set(label, [...(grouped.get(label) ?? []), entry]);
  }

  return [...grouped.values()];
}

function expandLabel(parsed: ParsedPath, currentLabel: string) {
  const segmentCount = currentLabel
    .split(parsed.separator)
    .filter(Boolean).length;

  if (segmentCount >= parsed.segments.length) {
    return parsed.original;
  }

  return parsed.segments.slice(-segmentCount - 1).join(parsed.separator);
}

function parsePath(path: string): ParsedPath {
  const separator = path.includes("\\") && !path.includes("/") ? "\\" : "/";
  const segments = path.split(/[\\/]+/).filter(Boolean);
  return {
    original: path,
    separator,
    segments: segments.length > 0 ? segments : [path],
  };
}
