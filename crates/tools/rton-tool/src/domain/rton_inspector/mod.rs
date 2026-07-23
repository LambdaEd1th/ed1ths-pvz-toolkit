use crate::i18n::I18n;

pub(crate) use rton_editor_core::{
    RtonStringMode, format_inspector_offset, inspect_ascii_run, inspect_rton_payload,
    inspect_rton_string_info, inspect_special_region, locate_rton_value_offset,
    maybe_collect_string_tables, read_rton_varint, rton_tag_info,
};

pub(crate) fn string_mode_label(mode: RtonStringMode, i18n: I18n) -> String {
    match mode {
        RtonStringMode::Direct => i18n.t("hex-inspector-direct"),
        RtonStringMode::Definition => i18n.t("hex-inspector-definition"),
        RtonStringMode::Reference => i18n.t("hex-inspector-reference"),
    }
}
