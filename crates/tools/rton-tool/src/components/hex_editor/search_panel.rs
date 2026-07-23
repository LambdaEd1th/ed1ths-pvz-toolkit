use dioxus::prelude::*;

use crate::domain::HexSearchMode;
use crate::i18n::I18n;

#[component]
#[allow(clippy::too_many_arguments)]
pub(super) fn HexSearchPanel(
    i18n: I18n,
    search_mode_snapshot: HexSearchMode,
    search_query_snapshot: String,
    replace_query_snapshot: String,
    case_sensitive_snapshot: bool,
    search_controls_disabled: bool,
    replace_controls_disabled: bool,
    search_status_text: String,
    mut search_mode: Signal<HexSearchMode>,
    mut search_query: Signal<String>,
    mut replace_query: Signal<String>,
    mut case_sensitive: Signal<bool>,
    go_to_previous_match: EventHandler<()>,
    go_to_next_match: EventHandler<()>,
    replace_current_match: EventHandler<()>,
    replace_all_matches: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "rton-hex-search-panel",
            div {
                class: "rton-hex-search-mode",
                role: "group",
                aria_label: i18n.t("hex-search-mode"),
                button {
                    r#type: "button",
                    class: if search_mode_snapshot == HexSearchMode::Hex { "rton-hex-search-mode-button is-active" } else { "rton-hex-search-mode-button" },
                    onclick: move |_| search_mode.set(HexSearchMode::Hex),
                    "HEX"
                }
                button {
                    r#type: "button",
                    class: if search_mode_snapshot == HexSearchMode::Ascii { "rton-hex-search-mode-button is-active" } else { "rton-hex-search-mode-button" },
                    onclick: move |_| search_mode.set(HexSearchMode::Ascii),
                    "ASCII"
                }
            }
            input {
                class: "rton-hex-search-field",
                value: "{search_query_snapshot}",
                placeholder: if search_mode_snapshot == HexSearchMode::Hex { i18n.t("hex-search-hex") } else { i18n.t("hex-search-ascii") },
                spellcheck: "false",
                oninput: move |event| search_query.set(event.value())
            }
            button {
                r#type: "button",
                class: "rton-hex-search-button",
                disabled: search_controls_disabled,
                onclick: move |_| go_to_previous_match.call(()),
                {i18n.t("hex-previous")}
            }
            button {
                r#type: "button",
                class: "rton-hex-search-button",
                disabled: search_controls_disabled,
                onclick: move |_| go_to_next_match.call(()),
                {i18n.t("hex-next")}
            }
            label { class: "rton-hex-search-check",
                input {
                    r#type: "checkbox",
                    checked: case_sensitive_snapshot,
                    disabled: search_mode_snapshot != HexSearchMode::Ascii,
                    onchange: move |event| case_sensitive.set(event.checked())
                }
                {i18n.t("hex-case-sensitive")}
            }
            input {
                class: "rton-hex-search-field",
                value: "{replace_query_snapshot}",
                placeholder: if search_mode_snapshot == HexSearchMode::Hex { i18n.t("hex-replace-hex") } else { i18n.t("hex-replace-ascii") },
                spellcheck: "false",
                oninput: move |event| replace_query.set(event.value())
            }
            button {
                r#type: "button",
                class: "rton-hex-search-button",
                disabled: replace_controls_disabled,
                onclick: move |_| replace_current_match.call(()),
                {i18n.t("hex-replace")}
            }
            button {
                r#type: "button",
                class: "rton-hex-search-button",
                disabled: replace_controls_disabled,
                onclick: move |_| replace_all_matches.call(()),
                {i18n.t("hex-replace-all")}
            }
            span { class: "rton-hex-search-status", "{search_status_text}" }
            button {
                r#type: "button",
                class: "rton-hex-search-close",
                aria_label: i18n.t("hex-close-search"),
                onclick: move |_| on_close.call(()),
                "×"
            }
        }
    }
}
