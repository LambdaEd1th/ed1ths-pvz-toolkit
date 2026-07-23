use dioxus::prelude::*;
use dioxus_html::input_data::MouseButton;

use crate::domain::{ByteDocument, HexSearchMatch};

use super::HexContextMenu;
use super::logic::{
    byte_to_ascii, display_hex_cell, hex_ascii_class, hex_byte_class, to_offset_hex,
};
use super::state::{ByteSelection, HexPane, PendingHexEdit};

#[component]
pub(super) fn HexRow(
    bytes: ByteDocument,
    row_index: usize,
    row_top: i64,
    bytes_per_row: usize,
    offset_width: usize,
    selected_offset: usize,
    normalized_selection: Option<ByteSelection>,
    pending_hex_edit: Option<PendingHexEdit>,
    search_matches: Vec<HexSearchMatch>,
    current_match: Option<HexSearchMatch>,
    on_select: EventHandler<(usize, HexPane, bool)>,
    on_enter: EventHandler<(usize, HexPane)>,
    on_context_menu: EventHandler<(usize, HexPane, HexContextMenu)>,
) -> Element {
    let row_start = row_index * bytes_per_row;
    let row_offsets = (row_start..row_start + bytes_per_row).collect::<Vec<_>>();

    rsx! {
        div {
            class: "rton-hex-row",
            style: "transform: translateY({row_top}px)",
            button {
                r#type: "button",
                class: "rton-hex-offset rton-hex-offset-button",
                onclick: move |_| on_select.call((row_start.min(bytes.len().saturating_sub(1)), HexPane::Hex, false)),
                "{to_offset_hex(row_start, offset_width - 2)}"
            }
            div { class: "rton-hex-grid",
                for offset in row_offsets.clone() {
                    if offset >= bytes.len() {
                        span { class: "rton-hex-byte-placeholder" }
                    } else {
                        button {
                            r#type: "button",
                            class: hex_byte_class(offset, selected_offset, normalized_selection, &search_matches, current_match),
                            aria_label: "Byte {to_offset_hex(offset, offset_width - 2)}",
                            onmousedown: move |event| {
                                if !is_primary_mouse_button(&event) {
                                    return;
                                }
                                event.prevent_default();
                                on_select.call((offset, HexPane::Hex, event.modifiers().shift()));
                            },
                            oncontextmenu: move |event| {
                                event.prevent_default();
                                event.stop_propagation();
                                let coordinates = event.client_coordinates();
                                on_context_menu.call((
                                    offset,
                                    HexPane::Hex,
                                    HexContextMenu {
                                        x: coordinates.x.round() as i32,
                                        y: coordinates.y.round() as i32,
                                    },
                                ));
                            },
                            onmouseenter: move |_| on_enter.call((offset, HexPane::Hex)),
                            "{display_hex_cell(&bytes, offset, pending_hex_edit.as_ref())}"
                        }
                    }
                }
            }
            div { class: "rton-hex-ascii",
                for offset in row_offsets {
                    if offset >= bytes.len() {
                        span { class: "rton-hex-ascii-placeholder" }
                    } else {
                        button {
                            r#type: "button",
                            class: hex_ascii_class(offset, selected_offset, normalized_selection, &search_matches, current_match),
                            aria_label: "ASCII byte {to_offset_hex(offset, offset_width - 2)}",
                            onmousedown: move |event| {
                                if !is_primary_mouse_button(&event) {
                                    return;
                                }
                                event.prevent_default();
                                on_select.call((offset, HexPane::Ascii, event.modifiers().shift()));
                            },
                            oncontextmenu: move |event| {
                                event.prevent_default();
                                event.stop_propagation();
                                let coordinates = event.client_coordinates();
                                on_context_menu.call((
                                    offset,
                                    HexPane::Ascii,
                                    HexContextMenu {
                                        x: coordinates.x.round() as i32,
                                        y: coordinates.y.round() as i32,
                                    },
                                ));
                            },
                            onmouseenter: move |_| on_enter.call((offset, HexPane::Ascii)),
                            "{byte_to_ascii(bytes.byte_at(offset).unwrap_or_default())}"
                        }
                    }
                }
            }
        }
    }
}

fn is_primary_mouse_button(event: &MouseEvent) -> bool {
    matches!(event.trigger_button(), None | Some(MouseButton::Primary))
}
