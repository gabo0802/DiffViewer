use super::twoway::AlignmentRow;
use serde::{Deserialize, Serialize};

/// The rendered diff model sent to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedDiffModel {
    pub filediff_id: String,
    pub rows: Vec<AlignmentRow>,
    pub hunks: Vec<RenderedHunkRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedHunkRef {
    pub hunk_id: String,
    pub start_row: usize,
    pub end_row: usize,
}

/// Build a RenderedDiffModel from alignment rows.
pub fn build_rendered_model(filediff_id: &str, rows: &[AlignmentRow]) -> RenderedDiffModel {
    // Collect visible changed regions for hunk navigation.
    let mut hunk_refs: Vec<RenderedHunkRef> = Vec::new();
    let mut current_hunk_id: Option<&str> = None;
    let mut hunk_start: Option<usize> = None;
    let mut block_index = 0usize;

    for (i, row) in rows.iter().enumerate() {
        let row_hunk_id = row.hunk_id.as_deref();
        let row_is_changed = is_changed_row(row);

        match (row_hunk_id, current_hunk_id, row_is_changed) {
            (Some(hid), None, true) => {
                current_hunk_id = Some(hid);
                hunk_start = Some(i);
            }
            (Some(hid), Some(cid), true) if hid != cid => {
                block_index += 1;
                hunk_refs.push(RenderedHunkRef {
                    hunk_id: format!("{}-block-{}", cid, block_index),
                    start_row: hunk_start.unwrap_or(i),
                    end_row: i - 1,
                });
                current_hunk_id = Some(hid);
                hunk_start = Some(i);
            }
            (_, Some(cid), false) => {
                block_index += 1;
                hunk_refs.push(RenderedHunkRef {
                    hunk_id: format!("{}-block-{}", cid, block_index),
                    start_row: hunk_start.unwrap_or(i),
                    end_row: i - 1,
                });
                current_hunk_id = None;
                hunk_start = None;
            }
            _ => {}
        }
    }
    if let Some(cid) = current_hunk_id {
        block_index += 1;
        hunk_refs.push(RenderedHunkRef {
            hunk_id: format!("{}-block-{}", cid, block_index),
            start_row: hunk_start.unwrap_or(0),
            end_row: rows.len().saturating_sub(1),
        });
    }

    RenderedDiffModel {
        filediff_id: filediff_id.to_string(),
        rows: rows.to_vec(),
        hunks: hunk_refs,
    }
}

fn is_changed_row(row: &AlignmentRow) -> bool {
    fn changed_kind(kind: &str) -> bool {
        matches!(kind, "add" | "del" | "empty")
    }

    row.left
        .as_ref()
        .map(|cell| changed_kind(&cell.kind))
        .unwrap_or(false)
        || row
            .right
            .as_ref()
            .map(|cell| changed_kind(&cell.kind))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::super::twoway::{AlignmentCell, AlignmentRow};
    use super::*;

    #[test]
    fn splits_navigation_hunks_across_context_inside_same_patch_hunk() {
        let rows = vec![
            AlignmentRow {
                row_id: "row-1".to_string(),
                left: Some(AlignmentCell {
                    line_no: 1,
                    text: "old one".to_string(),
                    kind: "del".to_string(),
                }),
                right: Some(AlignmentCell {
                    line_no: 1,
                    text: "new one".to_string(),
                    kind: "add".to_string(),
                }),
                hunk_id: Some("hunk-1".to_string()),
            },
            AlignmentRow {
                row_id: "row-2".to_string(),
                left: Some(AlignmentCell {
                    line_no: 2,
                    text: "context".to_string(),
                    kind: "context".to_string(),
                }),
                right: Some(AlignmentCell {
                    line_no: 2,
                    text: "context".to_string(),
                    kind: "context".to_string(),
                }),
                hunk_id: Some("hunk-1".to_string()),
            },
            AlignmentRow {
                row_id: "row-3".to_string(),
                left: Some(AlignmentCell {
                    line_no: 3,
                    text: "old two".to_string(),
                    kind: "del".to_string(),
                }),
                right: Some(AlignmentCell {
                    line_no: 3,
                    text: "new two".to_string(),
                    kind: "add".to_string(),
                }),
                hunk_id: Some("hunk-1".to_string()),
            },
        ];

        let model = build_rendered_model("fd-1", &rows);
        assert_eq!(model.hunks.len(), 2);
        assert_eq!(model.hunks[0].start_row, 0);
        assert_eq!(model.hunks[0].end_row, 0);
        assert_eq!(model.hunks[1].start_row, 2);
        assert_eq!(model.hunks[1].end_row, 2);
    }
}
