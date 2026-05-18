use rusqlite::Connection;

use crate::content_source::ContentSource;
use crate::diff_engine::{render, twoway, unified_parser};
use crate::store;

pub fn get_rendered_diff(
    conn: &Connection,
    filediff_id: &str,
) -> Result<render::RenderedDiffModel, String> {
    let fd = store::get_filediff(conn, filediff_id)?;
    let left_text = ContentSource::resolve_json_text(&fd.content_left_json)?;
    let right_text = ContentSource::resolve_json_text(&fd.content_right_json)?;
    let hunks: Vec<unified_parser::Hunk> =
        serde_json::from_str(&fd.hunks_json).map_err(|err| err.to_string())?;
    let rows = twoway::build_alignment_rows(&left_text, &right_text, &hunks);
    Ok(render::build_rendered_model(filediff_id, &rows))
}
