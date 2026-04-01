use serde::{Deserialize, Serialize};
use super::unified_parser::{Hunk, HunkLine};

/// Compute hunks by comparing two texts line-by-line (simple LCS-based diff).
pub fn compute_hunks(left: &str, right: &str) -> Vec<Hunk> {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();

    let lcs = lcs_table(&left_lines, &right_lines);
    let edits = backtrack_edits(&lcs, &left_lines, &right_lines);

    build_hunks_from_edits(&edits, 3)
}

#[derive(Debug, Clone)]
enum Edit {
    Keep(String),
    Del(String),
    Add(String),
}

fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<usize>> {
    let m = a.len();
    let n = b.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }
    dp
}

fn backtrack_edits(dp: &[Vec<usize>], a: &[&str], b: &[&str]) -> Vec<Edit> {
    let mut i = a.len();
    let mut j = b.len();
    let mut edits = Vec::new();

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && a[i - 1] == b[j - 1] {
            edits.push(Edit::Keep(a[i - 1].to_string()));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            edits.push(Edit::Add(b[j - 1].to_string()));
            j -= 1;
        } else {
            edits.push(Edit::Del(a[i - 1].to_string()));
            i -= 1;
        }
    }

    edits.reverse();
    edits
}

fn build_hunks_from_edits(edits: &[Edit], context: usize) -> Vec<Hunk> {
    // Find ranges of changed lines, expand by context, merge overlapping ranges.
    let mut change_indices: Vec<usize> = Vec::new();
    for (i, e) in edits.iter().enumerate() {
        match e {
            Edit::Add(_) | Edit::Del(_) => change_indices.push(i),
            _ => {}
        }
    }

    if change_indices.is_empty() {
        return Vec::new();
    }

    // Group changes into ranges expanded by context lines
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut start = change_indices[0].saturating_sub(context);
    let mut end = (change_indices[0] + context).min(edits.len() - 1);

    for &ci in &change_indices[1..] {
        let new_start = ci.saturating_sub(context);
        let new_end = (ci + context).min(edits.len() - 1);
        if new_start <= end + 1 {
            end = new_end;
        } else {
            ranges.push((start, end));
            start = new_start;
            end = new_end;
        }
    }
    ranges.push((start, end));

    // Build hunks from ranges
    let mut hunks = Vec::new();
    let mut hunk_counter = 0;

    for (range_start, range_end) in ranges {
        hunk_counter += 1;
        let mut lines = Vec::new();
        let mut old_line = 0usize;
        let mut new_line = 0usize;

        // Count lines before this range to get starting line numbers
        let mut old_start = 1usize;
        let mut new_start = 1usize;
        for e in edits.iter().take(range_start) {
            match e {
                Edit::Keep(_) => { old_start += 1; new_start += 1; }
                Edit::Del(_) => { old_start += 1; }
                Edit::Add(_) => { new_start += 1; }
            }
        }

        for e in edits.iter().take(range_end + 1).skip(range_start) {
            match e {
                Edit::Keep(t) => {
                    lines.push(HunkLine { kind: "context".to_string(), text: t.clone() });
                    old_line += 1;
                    new_line += 1;
                }
                Edit::Del(t) => {
                    lines.push(HunkLine { kind: "del".to_string(), text: t.clone() });
                    old_line += 1;
                }
                Edit::Add(t) => {
                    lines.push(HunkLine { kind: "add".to_string(), text: t.clone() });
                    new_line += 1;
                }
            }
        }

        hunks.push(Hunk {
            id: format!("hunk-{}", hunk_counter),
            old_start,
            old_count: old_line,
            new_start,
            new_count: new_line,
            lines,
        });
    }

    hunks
}

/// Alignment row for side-by-side rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentRow {
    pub row_id: String,
    pub left: Option<AlignmentCell>,
    pub right: Option<AlignmentCell>,
    pub hunk_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentCell {
    pub line_no: usize,
    pub text: String,
    pub kind: String, // "context" | "del" | "add" | "empty"
}

/// Build alignment rows from hunks (for side-by-side display).
pub fn build_alignment_rows(left_text: &str, right_text: &str, hunks: &[Hunk]) -> Vec<AlignmentRow> {
    let left_lines: Vec<&str> = left_text.lines().collect();
    let right_lines: Vec<&str> = right_text.lines().collect();

    let mut rows = Vec::new();
    let mut left_idx = 0usize;
    let mut right_idx = 0usize;
    let mut row_counter = 0usize;

    for hunk in hunks {
        let hunk_old_start = if hunk.old_start > 0 { hunk.old_start - 1 } else { 0 };
        let hunk_new_start = if hunk.new_start > 0 { hunk.new_start - 1 } else { 0 };

        // Context lines before hunk
        while left_idx < hunk_old_start && right_idx < hunk_new_start {
            row_counter += 1;
            rows.push(AlignmentRow {
                row_id: format!("row-{}", row_counter),
                left: Some(AlignmentCell {
                    line_no: left_idx + 1,
                    text: left_lines.get(left_idx).unwrap_or(&"").to_string(),
                    kind: "context".to_string(),
                }),
                right: Some(AlignmentCell {
                    line_no: right_idx + 1,
                    text: right_lines.get(right_idx).unwrap_or(&"").to_string(),
                    kind: "context".to_string(),
                }),
                hunk_id: None,
            });
            left_idx += 1;
            right_idx += 1;
        }

        // Hunk lines — group del/add for side-by-side alignment
        let mut del_lines: Vec<&HunkLine> = Vec::new();
        let mut add_lines: Vec<&HunkLine> = Vec::new();

        for hl in &hunk.lines {
            match hl.kind.as_str() {
                "del" => {
                    if !add_lines.is_empty() {
                        emit_paired_rows(&mut rows, &mut row_counter, &del_lines, &add_lines,
                            &mut left_idx, &mut right_idx, &hunk.id);
                        del_lines.clear();
                        add_lines.clear();
                    }
                    del_lines.push(hl);
                }
                "add" => {
                    add_lines.push(hl);
                }
                _ => {
                    // Flush del/add
                    if !del_lines.is_empty() || !add_lines.is_empty() {
                        emit_paired_rows(&mut rows, &mut row_counter, &del_lines, &add_lines,
                            &mut left_idx, &mut right_idx, &hunk.id);
                        del_lines.clear();
                        add_lines.clear();
                    }
                    // Context line
                    row_counter += 1;
                    rows.push(AlignmentRow {
                        row_id: format!("row-{}", row_counter),
                        left: Some(AlignmentCell {
                            line_no: left_idx + 1,
                            text: hl.text.clone(),
                            kind: "context".to_string(),
                        }),
                        right: Some(AlignmentCell {
                            line_no: right_idx + 1,
                            text: hl.text.clone(),
                            kind: "context".to_string(),
                        }),
                        hunk_id: Some(hunk.id.clone()),
                    });
                    left_idx += 1;
                    right_idx += 1;
                }
            }
        }
        // Flush remaining
        if !del_lines.is_empty() || !add_lines.is_empty() {
            emit_paired_rows(&mut rows, &mut row_counter, &del_lines, &add_lines,
                &mut left_idx, &mut right_idx, &hunk.id);
        }
    }

    // Remaining context after all hunks
    while left_idx < left_lines.len() || right_idx < right_lines.len() {
        row_counter += 1;
        rows.push(AlignmentRow {
            row_id: format!("row-{}", row_counter),
            left: if left_idx < left_lines.len() {
                let cell = AlignmentCell {
                    line_no: left_idx + 1,
                    text: left_lines[left_idx].to_string(),
                    kind: "context".to_string(),
                };
                left_idx += 1;
                Some(cell)
            } else { None },
            right: if right_idx < right_lines.len() {
                let cell = AlignmentCell {
                    line_no: right_idx + 1,
                    text: right_lines[right_idx].to_string(),
                    kind: "context".to_string(),
                };
                right_idx += 1;
                Some(cell)
            } else { None },
            hunk_id: None,
        });
    }

    rows
}

fn emit_paired_rows(
    rows: &mut Vec<AlignmentRow>,
    row_counter: &mut usize,
    del_lines: &[&HunkLine],
    add_lines: &[&HunkLine],
    left_idx: &mut usize,
    right_idx: &mut usize,
    hunk_id: &str,
) {
    let max_len = del_lines.len().max(add_lines.len());
    for i in 0..max_len {
        *row_counter += 1;
        let left = if i < del_lines.len() {
            *left_idx += 1;
            Some(AlignmentCell {
                line_no: *left_idx,
                text: del_lines[i].text.clone(),
                kind: "del".to_string(),
            })
        } else {
            Some(AlignmentCell { line_no: 0, text: String::new(), kind: "empty".to_string() })
        };
        let right = if i < add_lines.len() {
            *right_idx += 1;
            Some(AlignmentCell {
                line_no: *right_idx,
                text: add_lines[i].text.clone(),
                kind: "add".to_string(),
            })
        } else {
            Some(AlignmentCell { line_no: 0, text: String::new(), kind: "empty".to_string() })
        };
        rows.push(AlignmentRow {
            row_id: format!("row-{}", row_counter),
            left,
            right,
            hunk_id: Some(hunk_id.to_string()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hunks_simple() {
        let left = "line1\nline2\nline3\n";
        let right = "line1\nline2_mod\nline3\n";
        let hunks = compute_hunks(left, right);
        assert!(!hunks.is_empty());
        assert!(hunks[0].lines.iter().any(|l| l.kind == "del"));
        assert!(hunks[0].lines.iter().any(|l| l.kind == "add"));
    }

    #[test]
    fn test_alignment_rows() {
        let left = "a\nb\nc\n";
        let right = "a\nB\nc\n";
        let hunks = compute_hunks(left, right);
        let rows = build_alignment_rows(left, right, &hunks);
        assert!(rows.len() >= 3);
    }
}
