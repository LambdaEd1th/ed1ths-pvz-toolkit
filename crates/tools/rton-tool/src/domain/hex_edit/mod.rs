mod edit;
mod history;
mod search;

#[cfg(test)]
mod test_helpers;

pub(crate) use edit::{HexEdit, edited_len_after_edits};
pub(crate) use history::{
    HexHistory, prepare_hex_edits, push_hex_past_batch, push_hex_redo_batch, push_hex_undo_batch,
};
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) use search::find_hex_search_result;
pub(crate) use search::{
    HexSearchMatch, HexSearchMode, HexSearchResult, find_containing_match, hex_search_status_text,
    parse_replace_pattern, parse_search_pattern, relative_search_match, replace_all_byte_edits,
};

#[cfg(test)]
pub(crate) use edit::edited_len;
#[cfg(test)]
pub(crate) use history::HexUndoEdit;
#[cfg(test)]
pub(crate) use search::find_hex_search_matches;
#[cfg(test)]
pub(crate) use search::{HEX_SEARCH_MATCH_DISPLAY_LIMIT, parse_ascii_pattern, parse_hex_pattern};
#[cfg(test)]
pub(crate) use test_helpers::{
    apply_hex_edit, apply_hex_edits, overwrite_byte_range, replace_byte_span,
};
