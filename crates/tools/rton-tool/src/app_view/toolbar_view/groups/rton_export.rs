use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdCircleCheck, LdDownload};

use crate::components::{button_class, lucide_icon};
use crate::i18n::I18n;

#[component]
pub(super) fn RtonExportToolbarGroup(
    i18n: I18n,
    active: bool,
    compact_snapshot: bool,
    encrypt_snapshot: bool,
    mut encrypt_output: Signal<bool>,
    on_compact_change: EventHandler<bool>,
    parse_current: EventHandler<()>,
    export_rton: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "rton-toolbar-group",
            label { class: "rton-switch",
                input {
                    class: "rton-switch-input",
                    r#type: "checkbox",
                    checked: compact_snapshot,
                    disabled: !active,
                    onchange: move |event| on_compact_change.call(event.checked())
                }
                span { class: "rton-switch-label", {i18n.t("toolbar-compact")} }
                span { class: "rton-switch-track" }
            }
            label { class: "rton-switch",
                input {
                    class: "rton-switch-input",
                    r#type: "checkbox",
                    checked: encrypt_snapshot,
                    disabled: !active,
                    onchange: move |event| encrypt_output.set(event.checked())
                }
                span { class: "rton-switch-label", {i18n.t("toolbar-encrypted")} }
                span { class: "rton-switch-track" }
            }
            button {
                class: button_class("secondary"),
                disabled: !active,
                onclick: move |_| parse_current.call(()),
                span { class: "button-icon", {lucide_icon(LdCircleCheck)} }
                {i18n.t("toolbar-validate")}
            }
            button {
                class: button_class("primary"),
                disabled: !active,
                onclick: move |_| export_rton.call(()),
                span { class: "button-icon", {lucide_icon(LdDownload)} }
                "RTON"
            }
        }
    }
}
