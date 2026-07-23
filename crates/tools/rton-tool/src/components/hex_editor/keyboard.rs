use dioxus::prelude::*;

use crate::domain::{ByteDocument, HexEdit};

use super::logic::{
    HexCommitTargets, HexSelectionTarget, commit_hex_clear_target, commit_hex_write_target,
    hex_selection_target, is_hex_key, key_to_latin1_byte, set_hex_focus,
};
use super::state::{ByteSelection, HexPane, PendingHexEdit};

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_hex_key(
    event: KeyboardEvent,
    bytes: &ByteDocument,
    bytes_len: usize,
    safe_selected_offset: usize,
    bytes_per_row: usize,
    normalized_selection: Option<ByteSelection>,
    mut insert_mode: Signal<bool>,
    active_pane: Signal<HexPane>,
    mut pending_hex_edit: Signal<Option<PendingHexEdit>>,
    mut selection_range: Signal<Option<ByteSelection>>,
    selection_anchor: Signal<usize>,
    mut selected_offset: Signal<usize>,
    on_change: EventHandler<Vec<HexEdit>>,
    on_undo: EventHandler<()>,
    on_redo: EventHandler<()>,
    on_search_visible_change: EventHandler<bool>,
) {
    if bytes_len == 0 {
        return;
    }
    let bytes_per_row = bytes_per_row.max(1);
    let commit_targets = HexCommitTargets {
        on_change,
        pending_hex_edit,
        selection_range,
        selected_offset,
    };
    let Some(active_target) =
        hex_selection_target(normalized_selection, bytes_len, safe_selected_offset)
    else {
        return;
    };
    let key = event.key().to_string();
    let modifiers = event.modifiers();
    let ctrl_or_meta = modifiers.ctrl() || modifiers.meta();
    let shift = modifiers.shift();

    if ctrl_or_meta && key.eq_ignore_ascii_case("f") {
        event.prevent_default();
        on_search_visible_change.call(true);
        return;
    }
    if ctrl_or_meta && (key.eq_ignore_ascii_case("y") || (shift && key.eq_ignore_ascii_case("z"))) {
        event.prevent_default();
        on_redo.call(());
        return;
    }
    if ctrl_or_meta && key.eq_ignore_ascii_case("z") {
        event.prevent_default();
        on_undo.call(());
        return;
    }

    match key.as_str() {
        "Insert" => {
            event.prevent_default();
            let next_insert_mode = !*insert_mode.read();
            insert_mode.set(next_insert_mode);
            pending_hex_edit.set(None);
        }
        "ArrowLeft" => {
            event.prevent_default();
            set_hex_focus(
                safe_selected_offset as isize - 1,
                shift,
                bytes_len,
                selection_range,
                selection_anchor,
                selected_offset,
            );
        }
        "ArrowRight" => {
            event.prevent_default();
            set_hex_focus(
                safe_selected_offset as isize + 1,
                shift,
                bytes_len,
                selection_range,
                selection_anchor,
                selected_offset,
            );
        }
        "ArrowUp" => {
            event.prevent_default();
            set_hex_focus(
                safe_selected_offset as isize - bytes_per_row as isize,
                shift,
                bytes_len,
                selection_range,
                selection_anchor,
                selected_offset,
            );
        }
        "ArrowDown" => {
            event.prevent_default();
            set_hex_focus(
                safe_selected_offset as isize + bytes_per_row as isize,
                shift,
                bytes_len,
                selection_range,
                selection_anchor,
                selected_offset,
            );
        }
        "Home" => {
            event.prevent_default();
            set_hex_focus(
                (safe_selected_offset / bytes_per_row * bytes_per_row) as isize,
                shift,
                bytes_len,
                selection_range,
                selection_anchor,
                selected_offset,
            );
        }
        "End" => {
            event.prevent_default();
            set_hex_focus(
                ((safe_selected_offset / bytes_per_row * bytes_per_row) + bytes_per_row - 1)
                    as isize,
                shift,
                bytes_len,
                selection_range,
                selection_anchor,
                selected_offset,
            );
        }
        "Backspace" => {
            event.prevent_default();
            pending_hex_edit.set(None);
            let target = if *insert_mode.read() && !active_target.explicit_selection {
                if safe_selected_offset == 0 {
                    return;
                }
                HexSelectionTarget::new(safe_selected_offset - 1, 1, false)
            } else {
                active_target
            };
            commit_hex_clear_target(bytes, target, *insert_mode.read(), commit_targets);
        }
        "Delete" => {
            event.prevent_default();
            pending_hex_edit.set(None);
            commit_hex_clear_target(bytes, active_target, *insert_mode.read(), commit_targets);
        }
        "Escape" => {
            pending_hex_edit.set(None);
            selection_range.set(None);
        }
        _ => {
            if ctrl_or_meta {
                return;
            }
            if *active_pane.read() == HexPane::Ascii {
                if let Some(value) = key_to_latin1_byte(&key) {
                    event.prevent_default();
                    pending_hex_edit.set(None);
                    commit_hex_write_target(
                        bytes,
                        active_target,
                        vec![value],
                        *insert_mode.read(),
                        commit_targets,
                    );
                }
                return;
            }

            if is_hex_key(&key) {
                event.prevent_default();
                let edit_offset = active_target.offset;
                let current_text = pending_hex_edit
                    .read()
                    .as_ref()
                    .filter(|pending| pending.offset == edit_offset)
                    .map(|pending| pending.text.clone())
                    .unwrap_or_default();
                let next_text = format!("{current_text}{}", key.to_ascii_uppercase());
                if next_text.len() < 2 {
                    pending_hex_edit.set(Some(PendingHexEdit {
                        offset: edit_offset,
                        text: next_text,
                        delete_length: active_target
                            .explicit_selection
                            .then_some(active_target.length),
                    }));
                    selected_offset.set(edit_offset);
                    return;
                }
                let value = u8::from_str_radix(&next_text[..2], 16).unwrap_or_default();
                commit_hex_write_target(
                    bytes,
                    active_target,
                    vec![value],
                    *insert_mode.read(),
                    commit_targets,
                );
                pending_hex_edit.set(None);
            }
        }
    }
}
