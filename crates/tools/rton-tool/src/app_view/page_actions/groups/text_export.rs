use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdFileJson, LdFileText};
use rton_editor_core::TextFormat;

use crate::components::{button_class, lucide_icon};

#[component]
pub(super) fn TextExportActions(active: bool, export_text: EventHandler<TextFormat>) -> Element {
    rsx! {
        div { class: "rton-action-row",
            button {
                class: button_class("secondary"),
                disabled: !active,
                onclick: move |_| export_text.call(TextFormat::Json),
                span { class: "button-icon", {lucide_icon(LdFileJson)} }
                "JSON"
            }
            button {
                class: button_class("secondary"),
                disabled: !active,
                onclick: move |_| export_text.call(TextFormat::Yaml),
                span { class: "button-icon", {lucide_icon(LdFileText)} }
                "YAML"
            }
            button {
                class: button_class("secondary"),
                disabled: !active,
                onclick: move |_| export_text.call(TextFormat::Toml),
                span { class: "button-icon", {lucide_icon(LdFileText)} }
                "TOML"
            }
        }
    }
}
