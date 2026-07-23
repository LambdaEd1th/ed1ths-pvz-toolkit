use dioxus::prelude::*;

use crate::i18n::I18n;
use crate::platform;

#[component]
pub(super) fn PreferencesToolbarGroup(
    i18n: I18n,
    active: bool,
    line_wrapping_snapshot: bool,
    editor_search_panel_visible_snapshot: bool,
    mut line_wrapping: Signal<bool>,
    mut editor_search_panel_visible: Signal<bool>,
) -> Element {
    rsx! {
        div { class: "rton-toolbar-group",
            label { class: "rton-switch",
                input {
                    class: "rton-switch-input",
                    r#type: "checkbox",
                    checked: line_wrapping_snapshot,
                    onchange: move |event| {
                        let checked = event.checked();
                        let _ = platform::save_line_wrapping_preference(checked);
                        line_wrapping.set(checked);
                    }
                }
                span { class: "rton-switch-label", {i18n.t("toolbar-wrap")} }
                span { class: "rton-switch-track" }
            }
            label { class: "rton-switch",
                input {
                    class: "rton-switch-input",
                    r#type: "checkbox",
                    checked: editor_search_panel_visible_snapshot,
                    disabled: !active,
                    onchange: move |event| editor_search_panel_visible.set(event.checked())
                }
                span { class: "rton-switch-label", {i18n.t("toolbar-search")} }
                span { class: "rton-switch-track" }
            }
        }
    }
}
