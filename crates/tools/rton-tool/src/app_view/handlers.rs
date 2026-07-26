use dioxus::prelude::*;

use crate::domain::{DropMarker, EditorTabState};

pub(crate) fn start_tab_drag_if_needed(
    id: usize,
    tabs: Signal<Vec<EditorTabState>>,
    mut dragged_tab_id: Signal<Option<usize>>,
    mut tab_drop_marker: Signal<Option<DropMarker<usize>>>,
) {
    if tabs.read().len() > 1 {
        dragged_tab_id.set(Some(id));
        tab_drop_marker.set(None);
    }
}

pub(crate) fn update_tab_drop_marker_for_drag(
    marker: DropMarker<usize>,
    dragged_tab_id: Signal<Option<usize>>,
    mut tab_drop_marker: Signal<Option<DropMarker<usize>>>,
) {
    if matches!(*dragged_tab_id.read(), Some(id) if id != marker.id) {
        tab_drop_marker.set(Some(marker));
    }
}
