use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdFileArchive, LdListTree, LdSearch};
use rton_editor_core::{TreeRows, ValueSearchResult, ValueStats};
use std::collections::HashSet;
use toolkit_ui::{PillTabs, WorkbenchPanel};

use super::layout::FileSummaryPanel;
use crate::components::{ValueSearchResults, ValueTree, lucide_icon};
use crate::domain::IdentityArc;
use crate::i18n::I18n;

#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectorSection {
    Index,
    Properties,
}

#[component]
pub(super) fn IndexPanel(
    i18n: I18n,
    active_file_id: Option<usize>,
    active_file_label: String,
    input_value: String,
    output_value: String,
    active_stats: Option<ValueStats>,
    has_parsed_document: bool,
    active_search: String,
    selected_path: String,
    tree_rows: IdentityArc<TreeRows>,
    expanded_paths: IdentityArc<HashSet<String>>,
    value_search_result: Option<IdentityArc<ValueSearchResult>>,
    on_search_change: EventHandler<String>,
    on_select_path: EventHandler<String>,
    on_toggle_path: EventHandler<String>,
    suppress_resize_observer: bool,
) -> Element {
    let mut active_section = use_signal(|| InspectorSection::Properties);
    use_effect(use_reactive(&active_file_id, move |_| {
        active_section.set(InspectorSection::Properties);
    }));
    let active_section_snapshot = *active_section.read();

    rsx! {
        WorkbenchPanel { id: "rton-inspector-drawer", class: "rton-side-panel rton-side-panel-right",
            PillTabs {
                class: "rton-inspector-tabs",
                aria_label: i18n.t("panel-inspector-tabs"),
                button {
                    r#type: "button",
                    class: if active_section_snapshot == InspectorSection::Properties { "active" } else { "" },
                    role: "tab",
                    aria_selected: active_section_snapshot == InspectorSection::Properties,
                    onclick: move |_| active_section.set(InspectorSection::Properties),
                    span { class: "panel-header-icon", {lucide_icon(LdFileArchive)} }
                    span { {i18n.t("panel-file-properties")} }
                }
                button {
                    r#type: "button",
                    class: if active_section_snapshot == InspectorSection::Index { "active" } else { "" },
                    role: "tab",
                    aria_selected: active_section_snapshot == InspectorSection::Index,
                    onclick: move |_| active_section.set(InspectorSection::Index),
                    span { class: "panel-header-icon", {lucide_icon(LdListTree)} }
                    span { {i18n.t("panel-inspector")} }
                }
            }
            div {
                class: if active_section_snapshot == InspectorSection::Properties { "rton-properties-section rton-inspector-section active" } else { "rton-properties-section rton-inspector-section" },
                FileSummaryPanel {
                    i18n,
                    active_file_label,
                    input_value,
                    output_value,
                    stats: active_stats
                }
            }
            div {
                class: if active_section_snapshot == InspectorSection::Index { "rton-index-panel rton-inspector-section active" } else { "rton-index-panel rton-inspector-section" },
                div { class: "rton-index-header",
                    div { class: "rton-index-title",
                        div { class: "rton-index-title-line",
                            span { class: "panel-header-icon", {lucide_icon(LdListTree)} }
                            h2 { {i18n.t("panel-inspector")} }
                        }
                        p { "RtonValue" }
                    }
                    div { class: "rton-index-search",
                        span { class: "search-icon", {lucide_icon(LdSearch)} }
                        input {
                            r#type: "search",
                            placeholder: i18n.t("inspector-search-placeholder"),
                            value: "{active_search}",
                            disabled: !has_parsed_document,
                            oninput: move |event| on_search_change.call(event.value()),
                            onkeydown: move |event| {
                                if event.key().to_string() == "Escape" {
                                    event.prevent_default();
                                    on_search_change.call(String::new());
                                }
                            }
                        }
                        if !active_search.is_empty() {
                            button {
                                r#type: "button",
                                class: "rton-index-search-clear",
                                aria_label: i18n.t("inspector-clear-search"),
                                title: i18n.t("inspector-clear-search"),
                                onclick: move |_| on_search_change.call(String::new()),
                                "×"
                            }
                        }
                    }
                }
                if let Some(search_result) = value_search_result {
                    ValueSearchResults {
                        result: search_result,
                        i18n,
                        on_select: on_select_path,
                        suppress_resize_observer
                    }
                } else if has_parsed_document {
                    ValueTree {
                        rows: tree_rows,
                        expanded_paths,
                        selected_path,
                        i18n,
                        on_select: on_select_path,
                        on_toggle: on_toggle_path,
                        suppress_resize_observer
                    }
                } else {
                    div { class: "empty-state",
                        strong { {i18n.t("inspector-empty-title")} }
                        p { {i18n.t("inspector-empty-body")} }
                    }
                }
            }
        }
    }
}
