use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdCheckCheck, LdFileArchive, LdSearch, LdSquare};
use toolkit_ui::WorkbenchPanel;

use crate::components::{FileList, FileListItem, FileSelection, button_class, lucide_icon};
use crate::domain::BatchExportMode;
use crate::domain::IdentityArc;
use crate::i18n::I18n;

#[component]
pub(super) fn FilePanel(
    i18n: I18n,
    file_list_subtitle: String,
    file_search: String,
    file_list_empty_message: String,
    file_list_items_empty: bool,
    filtered_file_list_items: IdentityArc<Vec<FileListItem>>,
    file_selection: FileSelection,
    selected_file_count: usize,
    selected_visible_file_count: usize,
    on_select_all: EventHandler<()>,
    on_clear_selected: EventHandler<()>,
    on_search_input: EventHandler<String>,
    on_batch_export: EventHandler<BatchExportMode>,
    on_open_file: EventHandler<usize>,
    on_activate: EventHandler<usize>,
    on_remove: EventHandler<String>,
    on_remove_path: EventHandler<String>,
    on_toggle_selected: EventHandler<(String, bool)>,
    on_toggle_path: EventHandler<(String, bool)>,
    suppress_resize_observer: bool,
) -> Element {
    rsx! {
        WorkbenchPanel { id: "rton-file-drawer", class: "rton-side-panel rton-side-panel-left",
            header { class: "panel-header",
                div { class: "panel-header-top",
                    div { class: "panel-header-main",
                        div { class: "panel-header-title-line",
                            span { class: "panel-header-icon", {lucide_icon(LdFileArchive)} }
                            h2 { {i18n.t("file-list-title")} }
                        }
                        p { "{file_list_subtitle}" }
                    }
                    div { class: "panel-header-actions",
                        button {
                            class: button_class("secondary"),
                            disabled: filtered_file_list_items.is_empty(),
                            onclick: move |_| on_select_all.call(()),
                            span { class: "button-icon", {lucide_icon(LdCheckCheck)} }
                            {i18n.t("file-list-select-all")}
                        }
                        button {
                            class: button_class("secondary"),
                            disabled: selected_visible_file_count == 0,
                            onclick: move |_| on_clear_selected.call(()),
                            span { class: "button-icon", {lucide_icon(LdSquare)} }
                            {i18n.t("file-list-select-none")}
                        }
                    }
                }
                div { class: "panel-header-below",
                    div { class: "batch-export-grid",
                        for mode in BatchExportMode::ALL {
                            button {
                                class: if mode == BatchExportMode::Rton { button_class("primary") } else { button_class("secondary") },
                                disabled: selected_file_count == 0,
                                onclick: move |_| on_batch_export.call(mode),
                                {mode.label()}
                            }
                        }
                    }
                }
            }
            div { class: "file-search-box",
                div { class: "file-search-inner",
                    span { class: "search-icon", {lucide_icon(LdSearch)} }
                    input {
                        r#type: "search",
                        placeholder: i18n.t("file-list-search-placeholder"),
                        value: "{file_search}",
                        disabled: file_list_items_empty,
                        oninput: move |event| on_search_input.call(event.value())
                    }
                    if !file_search.is_empty() {
                        button {
                            class: "file-search-clear",
                            title: i18n.t("file-list-clear-search"),
                            onclick: move |_| on_search_input.call(String::new()),
                            "×"
                        }
                    }
                }
            }
            FileList {
                items: filtered_file_list_items.clone(),
                selection: file_selection,
                empty_message: file_list_empty_message,
                i18n,
                on_open_file,
                on_activate,
                on_remove,
                on_remove_path,
                on_toggle_selected,
                on_toggle_path,
                suppress_resize_observer
            }
        }
    }
}
