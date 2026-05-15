use serde::{Deserialize, Serialize};

/// A single line in a hunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HunkLine {
    pub kind: String, // "context" | "add" | "del"
    pub text: String,
}

/// A hunk parsed from a unified diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hunk {
    pub id: String,
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<HunkLine>,
}

/// A single file entry parsed from a unified diff / patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchFileDiff {
    pub old_path: String,
    pub new_path: String,
    pub hunks: Vec<Hunk>,
    pub status: String, // "modified" | "added" | "deleted" | "renamed"
}

/// Parse a unified diff / patch string into a list of file diffs.
pub fn parse_unified_diff(patch: &str) -> Vec<PatchFileDiff> {
    let mut files: Vec<PatchFileDiff> = Vec::new();
    let mut current_file: Option<PatchFileDiff> = None;
    let mut current_hunk: Option<Hunk> = None;
    let mut hunk_counter: usize = 0;

    for line in patch.lines() {
        if line.starts_with("--- ") {
            // Flush previous hunk/file
            if let Some(ref mut f) = current_file {
                if let Some(h) = current_hunk.take() {
                    f.hunks.push(h);
                }
            }
            if let Some(f) = current_file.take() {
                files.push(f);
            }
            let old_path = line[4..].trim().to_string();
            let old_path = strip_prefix_ab(&old_path);
            current_file = Some(PatchFileDiff {
                old_path,
                new_path: String::new(),
                hunks: Vec::new(),
                status: "modified".to_string(),
            });
            current_hunk = None;
        } else if line.starts_with("+++ ") {
            if let Some(ref mut f) = current_file {
                let new_path = line[4..].trim().to_string();
                f.new_path = strip_prefix_ab(&new_path);
                // Derive status
                if f.old_path == "/dev/null" {
                    f.status = "added".to_string();
                } else if f.new_path == "/dev/null" {
                    f.status = "deleted".to_string();
                } else if f.old_path != f.new_path {
                    f.status = "renamed".to_string();
                }
            }
        } else if line.starts_with("@@ ") {
            // Flush previous hunk
            if let Some(ref mut f) = current_file {
                if let Some(h) = current_hunk.take() {
                    f.hunks.push(h);
                }
            }
            if let Some((os, oc, ns, nc)) = parse_hunk_header(line) {
                hunk_counter += 1;
                current_hunk = Some(Hunk {
                    id: format!("hunk-{}", hunk_counter),
                    old_start: os,
                    old_count: oc,
                    new_start: ns,
                    new_count: nc,
                    lines: Vec::new(),
                });
            }
        } else if let Some(ref mut h) = current_hunk {
            if let Some(rest) = line.strip_prefix('+') {
                h.lines.push(HunkLine {
                    kind: "add".to_string(),
                    text: rest.to_string(),
                });
            } else if let Some(rest) = line.strip_prefix('-') {
                h.lines.push(HunkLine {
                    kind: "del".to_string(),
                    text: rest.to_string(),
                });
            } else {
                // Context line (starts with ' ' or is the line itself)
                let text = line.strip_prefix(' ').unwrap_or(line);
                h.lines.push(HunkLine {
                    kind: "context".to_string(),
                    text: text.to_string(),
                });
            }
        }
    }

    // Flush last hunk/file
    if let Some(ref mut f) = current_file {
        if let Some(h) = current_hunk.take() {
            f.hunks.push(h);
        }
    }
    if let Some(f) = current_file.take() {
        files.push(f);
    }

    files
}

fn strip_prefix_ab(path: &str) -> String {
    if path.starts_with("a/") || path.starts_with("b/") {
        path[2..].to_string()
    } else {
        path.to_string()
    }
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize)> {
    // @@ -old_start,old_count +new_start,new_count @@
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }
    let old_range = parts[1].trim_start_matches('-');
    let new_range = parts[2].trim_start_matches('+');
    let (os, oc) = parse_range(old_range);
    let (ns, nc) = parse_range(new_range);
    Some((os, oc, ns, nc))
}

fn parse_range(range: &str) -> (usize, usize) {
    if let Some((start, count)) = range.split_once(',') {
        (
            start.parse().unwrap_or(1),
            count.parse().unwrap_or(0),
        )
    } else {
        (range.parse().unwrap_or(1), 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_patch() {
        let patch = "\
--- a/hello.txt
+++ b/hello.txt
@@ -1,3 +1,4 @@
 line1
-line2
+line2_modified
+line2b
 line3
";
        let files = parse_unified_diff(patch);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].old_path, "hello.txt");
        assert_eq!(files[0].new_path, "hello.txt");
        assert_eq!(files[0].hunks.len(), 1);
        assert_eq!(files[0].hunks[0].lines.len(), 5);
    }

    #[test]
    fn test_parse_added_file() {
        let patch = "\
--- /dev/null
+++ b/new_file.txt
@@ -0,0 +1,2 @@
+hello
+world
";
        let files = parse_unified_diff(patch);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, "added");
    }

    #[test]
    fn test_parse_git_unified_diff() {
        let patch = "\
commit abc123
Author: Dev <dev@example.com>

    touch git path

diff --git a/src/lib.rs b/src/lib.rs
index e69de29..4b825dc 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -0,0 +1,2 @@
+pub fn answer() -> i32 {
+    42
+}
";
        let files = parse_unified_diff(patch);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].new_path, "src/lib.rs");
        assert_eq!(files[0].status, "modified");
    }

    #[test]
    fn test_parse_p4_unified_diff() {
        let patch = "\
Change 12345 by dev@workspace on 2026/05/15 pending

Affected files ...

... //depot/main/foo.cpp#7 edit

Differences ...

==== //depot/main/foo.cpp#7 - C:\\work\\foo.cpp ====
--- //depot/main/foo.cpp#7
+++ C:\\work\\foo.cpp
@@ -1,2 +1,2 @@
 old
-line
+line changed
";
        let files = parse_unified_diff(patch);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].old_path, "//depot/main/foo.cpp#7");
        assert_eq!(files[0].new_path, "C:\\work\\foo.cpp");
        assert_eq!(files[0].hunks.len(), 1);
    }
}
