use dioxus::prelude::*;

use crate::domain::{
    ByteDocument, format_inspector_offset, inspect_ascii_run, inspect_rton_payload,
    inspect_rton_string_info, inspect_special_region, maybe_collect_string_tables,
    read_rton_varint, rton_tag_info, string_mode_label,
};
use crate::i18n::I18n;

use super::logic::{byte_to_ascii, byte_to_hex};

#[component]
pub(super) fn HexByteInspector(bytes: ByteDocument, offset: usize, i18n: I18n) -> Element {
    let Some(byte) = bytes.byte_at(offset) else {
        return rsx! {
            aside { class: "rton-hex-inspector",
                div { class: "rton-hex-inspector-empty", {i18n.t("hex-inspector-empty")} }
            }
        };
    };
    let tag = rton_tag_info(byte);
    let special = inspect_special_region(&bytes, offset);
    let payload = inspect_rton_payload(&bytes, offset, byte, tag);
    let varint = read_rton_varint(&bytes, offset);
    let tables = if should_collect_string_tables(byte) {
        maybe_collect_string_tables(&bytes, offset)
    } else {
        None
    };
    let string_info = if is_rton_string_tag(byte) {
        inspect_rton_string_info(&bytes, offset, tables.as_ref())
    } else {
        None
    };
    let ascii_run = inspect_ascii_run(&bytes, offset);

    rsx! {
        aside { class: "rton-hex-inspector",
            header { class: "rton-hex-inspector-header",
                h2 { {i18n.t("hex-inspector-title")} }
                span { "{format_inspector_offset(offset)}" }
            }
            section { class: "rton-hex-inspector-section",
                InspectorRow { label: i18n.t("hex-inspector-offset"), value: format_inspector_offset(offset) }
                InspectorRow { label: i18n.t("hex-inspector-byte"), value: format!("0x{}", byte_to_hex(byte)) }
                InspectorRow { label: i18n.t("hex-inspector-decimal"), value: byte.to_string() }
                InspectorRow { label: i18n.t("hex-inspector-ascii"), value: byte_to_ascii(byte).to_string() }
                if let Some(special) = special {
                    InspectorRow { label: i18n.t("hex-inspector-region"), value: special }
                }
            }
            section { class: "rton-hex-inspector-section",
                h3 { {i18n.t("hex-inspector-tag")} }
                if let Some(tag) = tag {
                    InspectorRow { label: i18n.t("hex-inspector-tag"), value: format!("{} · 0x{}", tag.name, byte_to_hex(byte)), strong: true }
                    InspectorRow { label: i18n.t("hex-inspector-category"), value: tag.category.to_string() }
                    InspectorRow { label: i18n.t("hex-inspector-payload-kind"), value: tag.payload_kind.to_string() }
                } else {
                    p { class: "rton-hex-inspector-note", {i18n.t("hex-inspector-unknown-tag")} }
                }
            }
            if let Some(payload) = payload.as_ref() {
                section { class: "rton-hex-inspector-section",
                    h3 { {i18n.t("hex-inspector-payload")} }
                    InspectorRow { label: payload.label.clone(), value: payload.value.clone(), strong: true }
                    if let Some(bytes) = payload.bytes.as_ref() {
                        InspectorRow { label: i18n.t("hex-inspector-bytes"), value: bytes.clone() }
                    }
                    if let Some(range) = payload.range.as_ref() {
                        InspectorRow { label: i18n.t("hex-inspector-range"), value: range.clone() }
                    }
                }
            }
            if let Some(varint) = varint.as_ref() {
                section { class: "rton-hex-inspector-section",
                    h3 { {i18n.t("hex-inspector-varint")} }
                    InspectorRow { label: i18n.t("hex-inspector-unsigned"), value: varint.value.clone(), strong: true }
                    InspectorRow { label: i18n.t("hex-inspector-zigzag"), value: varint.zigzag.clone() }
                    InspectorRow { label: i18n.t("hex-inspector-length"), value: i18n.t_args("hex-inspector-byte-count", &[("count", varint.length.to_string())]) }
                    InspectorRow { label: i18n.t("hex-inspector-bytes"), value: varint.bytes.clone() }
                    InspectorRow { label: i18n.t("hex-inspector-next-offset"), value: format_inspector_offset(varint.next_offset) }
                }
            }
            if string_info.is_some() || tables.is_some() {
                section { class: "rton-hex-inspector-section",
                    h3 { {i18n.t("hex-inspector-string-table")} }
                    if let Some(info) = string_info.as_ref() {
                        InspectorRow { label: i18n.t("hex-inspector-mode"), value: string_mode_label(info.mode, i18n) }
                        InspectorRow { label: i18n.t("hex-inspector-encoding"), value: info.encoding.to_string() }
                        if let Some(table) = info.table {
                            InspectorRow { label: i18n.t("hex-inspector-table"), value: table.to_string() }
                        }
                        if let Some(index) = info.index.as_ref() {
                            InspectorRow { label: i18n.t("hex-inspector-index"), value: index.clone(), strong: true }
                        }
                        if let Some(length) = info.length.as_ref() {
                            InspectorRow { label: i18n.t("hex-inspector-length"), value: length.clone() }
                        }
                        if let Some(byte_length) = info.byte_length.as_ref() {
                            InspectorRow { label: i18n.t("hex-inspector-bytes"), value: byte_length.clone() }
                        }
                        if let Some(text) = info.text.as_ref() {
                            InspectorRow { label: i18n.t("hex-inspector-text"), value: text.clone() }
                        }
                        if let Some(resolved) = info.resolved_text.as_ref() {
                            InspectorRow { label: i18n.t("hex-inspector-resolved"), value: resolved.clone(), strong: true }
                        }
                        if info.scan_limited {
                            p { class: "rton-hex-inspector-note", {i18n.t("hex-inspector-scan-limited")} }
                        }
                        if let Some(error) = info.scan_error.as_ref() {
                            p { class: "rton-hex-inspector-note", {i18n.t_args("hex-inspector-scan-error", &[("message", error.clone())])} }
                        }
                    }
                    if let Some(tables) = tables.as_ref() {
                        if !tables.limited {
                            InspectorRow {
                                label: i18n.t("hex-inspector-table-counts"),
                                value: format!("ASCII {} · UTF-8 {}", tables.ascii.len(), tables.utf8.len())
                            }
                        }
                        if tables.limited {
                            p { class: "rton-hex-inspector-note", {i18n.t("hex-inspector-scan-limited")} }
                        }
                        if let Some(error) = tables.error.as_ref() {
                            p { class: "rton-hex-inspector-note", {i18n.t_args("hex-inspector-scan-error", &[("message", error.clone())])} }
                        }
                    }
                }
            }
            if let Some(run) = ascii_run.as_ref() {
                section { class: "rton-hex-inspector-section",
                    h3 { {i18n.t("hex-inspector-ascii-run")} }
                    InspectorRow { label: i18n.t("hex-inspector-range"), value: format!("{}..{}", format_inspector_offset(run.start), format_inspector_offset(run.end)) }
                    div { class: "rton-hex-inspector-preview", "{run.text}" }
                }
            }
        }
    }
}

#[component]
fn InspectorRow(label: String, value: String, #[props(default = false)] strong: bool) -> Element {
    rsx! {
        div { class: "rton-hex-inspector-row",
            span { "{label}" }
            strong { class: if strong { "is-strong" } else { "" }, "{value}" }
        }
    }
}

fn is_rton_string_tag(byte: u8) -> bool {
    matches!(byte, 0x81 | 0x82 | 0x90..=0x93 | 0xb0..=0xb7)
}

fn should_collect_string_tables(byte: u8) -> bool {
    matches!(byte, 0x90..=0x93 | 0xb0..=0xb7)
}
