use dioxus::prelude::*;

use crate::domain::{DropMarker, ToolbarGroupId, normalize_toolbar_rows, toolbar_rows_to_json};
use crate::platform;

pub(crate) fn initial_toolbar_rows() -> Vec<Vec<ToolbarGroupId>> {
    normalize_toolbar_rows(platform::read_toolbar_layout_preference().as_deref())
}

pub(crate) fn save_toolbar_rows(rows: &[Vec<ToolbarGroupId>]) {
    let _ = platform::save_toolbar_layout_preference(&toolbar_rows_to_json(rows));
}

pub(crate) fn finish_toolbar_drag_state(
    toolbar_rows: Signal<Vec<Vec<ToolbarGroupId>>>,
    mut dragged_toolbar_group_id: Signal<Option<ToolbarGroupId>>,
    mut toolbar_drop_marker: Signal<Option<DropMarker<ToolbarGroupId>>>,
) {
    if dragged_toolbar_group_id.read().is_some() {
        save_toolbar_rows(&toolbar_rows.read());
    }
    dragged_toolbar_group_id.set(None);
    toolbar_drop_marker.set(None);
}
