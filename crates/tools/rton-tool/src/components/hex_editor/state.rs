pub(super) const HEX_BYTES_PER_ROW: usize = 16;
pub(super) const HEX_COMPACT_BYTES_PER_ROW: usize = 8;
pub(super) const HEX_COMPACT_LAYOUT_MAX_WIDTH: f64 = 560.0;
pub(super) const HEX_INSPECTOR_DEFAULT_WIDTH: i32 = 310;
pub(super) const HEX_INSPECTOR_MIN_WIDTH: i32 = 220;
pub(super) const HEX_INSPECTOR_MAX_WIDTH: i32 = 620;

use dioxus::prelude::*;

use crate::domain::{HEX_DEFAULT_VIEWPORT_HEIGHT, HexEdit, HexSearchMode};

use super::logic::HexCommitTargets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HexJumpTarget {
    pub(crate) id: u64,
    pub(crate) offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HexPane {
    Hex,
    Ascii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ByteSelection {
    pub(super) anchor: usize,
    pub(super) focus: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PendingHexEdit {
    pub(super) offset: usize,
    pub(super) text: String,
    pub(super) delete_length: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct HexInspectorResizeDrag {
    pub(super) start_x: f64,
    pub(super) start_width: i32,
}

#[derive(Clone, Copy)]
pub(super) struct HexEditorSignals {
    pub(super) selected_offset: Signal<usize>,
    pub(super) selection_anchor: Signal<usize>,
    pub(super) selection_range: Signal<Option<ByteSelection>>,
    pub(super) pending_hex_edit: Signal<Option<PendingHexEdit>>,
    pub(super) insert_mode: Signal<bool>,
    pub(super) active_pane: Signal<HexPane>,
    pub(super) pointer_selecting: Signal<bool>,
    pub(super) scroll_top: Signal<f64>,
    pub(super) viewport_height: Signal<usize>,
    pub(super) bytes_per_row: Signal<usize>,
    pub(super) inspector_width: Signal<i32>,
    pub(super) inspector_drag: Signal<Option<HexInspectorResizeDrag>>,
    pub(super) search_mode: Signal<HexSearchMode>,
    pub(super) search_query: Signal<String>,
    pub(super) replace_query: Signal<String>,
    pub(super) case_sensitive: Signal<bool>,
}

impl HexEditorSignals {
    pub(super) fn commit_targets(self, on_change: EventHandler<Vec<HexEdit>>) -> HexCommitTargets {
        HexCommitTargets {
            on_change,
            pending_hex_edit: self.pending_hex_edit,
            selection_range: self.selection_range,
            selected_offset: self.selected_offset,
        }
    }
}

pub(super) fn use_hex_editor_signals() -> HexEditorSignals {
    HexEditorSignals {
        selected_offset: use_signal(|| 0_usize),
        selection_anchor: use_signal(|| 0_usize),
        selection_range: use_signal(|| None::<ByteSelection>),
        pending_hex_edit: use_signal(|| None::<PendingHexEdit>),
        insert_mode: use_signal(|| false),
        active_pane: use_signal(|| HexPane::Hex),
        pointer_selecting: use_signal(|| false),
        scroll_top: use_signal(|| 0_f64),
        viewport_height: use_signal(|| HEX_DEFAULT_VIEWPORT_HEIGHT),
        bytes_per_row: use_signal(|| HEX_BYTES_PER_ROW),
        inspector_width: use_signal(|| HEX_INSPECTOR_DEFAULT_WIDTH),
        inspector_drag: use_signal(|| None::<HexInspectorResizeDrag>),
        search_mode: use_signal(|| HexSearchMode::Hex),
        search_query: use_signal(String::new),
        replace_query: use_signal(String::new),
        case_sensitive: use_signal(|| true),
    }
}
