use super::unified_parser::Hunk;

/// Initialize a merged buffer from the right-side text (baseline).
pub fn init_merge_buffer(right_text: &str) -> String {
    right_text.to_string()
}

/// Apply a hunk from a given source ("left" or "right") onto the merged buffer.
/// For V1, this does line-level replacement based on hunk positions.
pub fn apply_hunk_to_buffer(
    merged: &str,
    hunk: &Hunk,
    source: &str, // "left" | "right"
) -> Result<String, String> {
    let merged_lines: Vec<String> = merged.lines().map(|l| l.to_string()).collect();

    // Determine which lines to insert based on source
    let insert_lines: Vec<&str> = hunk
        .lines
        .iter()
        .filter(|l| match source {
            "left" => l.kind == "del" || l.kind == "context",
            "right" => l.kind == "add" || l.kind == "context",
            _ => l.kind == "context",
        })
        .map(|l| l.text.as_str())
        .collect();

    // Replace at the hunk position in merged buffer (using new_start since merged starts as right-side)
    let start = if hunk.new_start > 0 {
        hunk.new_start - 1
    } else {
        0
    };
    let end = (start + hunk.new_count).min(merged_lines.len());

    let mut result = Vec::new();
    result.extend_from_slice(&merged_lines[..start]);
    result.extend(insert_lines.iter().map(|s| s.to_string()));
    if end <= merged_lines.len() {
        result.extend_from_slice(&merged_lines[end..]);
    }

    Ok(result.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_merge() {
        let right = "line1\nline2\nline3";
        let merged = init_merge_buffer(right);
        assert_eq!(merged, right);
    }
}
