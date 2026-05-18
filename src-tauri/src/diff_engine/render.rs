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
    // Collect hunk references by scanning rows
    let mut hunk_refs: Vec<RenderedHunkRef> = Vec::new();
    let mut current_hunk_id: Option<String> = None;
    let mut hunk_start: usize = 0;

    for (i, row) in rows.iter().enumerate() {
        match (&row.hunk_id, &current_hunk_id) {
            (Some(hid), None) => {
                current_hunk_id = Some(hid.clone());
                hunk_start = i;
            }
            (Some(hid), Some(cid)) if hid != cid => {
                hunk_refs.push(RenderedHunkRef {
                    hunk_id: cid.clone(),
                    start_row: hunk_start,
                    end_row: i - 1,
                });
                current_hunk_id = Some(hid.clone());
                hunk_start = i;
            }
            (None, Some(cid)) => {
                hunk_refs.push(RenderedHunkRef {
                    hunk_id: cid.clone(),
                    start_row: hunk_start,
                    end_row: i - 1,
                });
                current_hunk_id = None;
            }
            _ => {}
        }
    }
    if let Some(cid) = current_hunk_id {
        hunk_refs.push(RenderedHunkRef {
            hunk_id: cid,
            start_row: hunk_start,
            end_row: rows.len().saturating_sub(1),
        });
    }

    RenderedDiffModel {
        filediff_id: filediff_id.to_string(),
        rows: rows.to_vec(),
        hunks: hunk_refs,
    }
}
