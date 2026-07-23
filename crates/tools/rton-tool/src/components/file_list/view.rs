use std::collections::HashSet;
use std::sync::Arc;

use dioxus::prelude::*;

use crate::domain::{
    FILE_LIST_DEFAULT_VIEWPORT_HEIGHT, FILE_LIST_ROW_HEIGHT, FileListVirtualScroll, IdentityArc,
    file_list_virtual_row_top, file_list_virtual_scroll, measured_file_list_viewport_height,
};
use crate::i18n::I18n;

use super::model::{build_file_list_rows, file_list_row_key};
use super::{FileListItem, FileListRow, FileSelection};

#[component]
pub(crate) fn FileList(
    items: IdentityArc<Vec<FileListItem>>,
    selection: FileSelection,
    empty_message: String,
    i18n: I18n,
    on_open_file: EventHandler<usize>,
    on_activate: EventHandler<usize>,
    on_remove: EventHandler<String>,
    on_remove_path: EventHandler<String>,
    on_toggle_selected: EventHandler<(String, bool)>,
    on_toggle_path: EventHandler<(String, bool)>,
    suppress_resize_observer: bool,
) -> Element {
    let mut scroll_top = use_signal(|| 0_f64);
    let viewport_height = use_signal(|| FILE_LIST_DEFAULT_VIEWPORT_HEIGHT);
    let mut mounted = use_signal(|| None::<MountedEvent>);
    let mut collapsed_paths = use_signal(HashSet::<String>::new);
    let collapsed_snapshot = collapsed_paths.read().clone();
    let rows = use_memo(use_reactive(
        &(items.clone(), selection.clone(), collapsed_snapshot),
        move |(items, selection, collapsed_paths)| {
            Arc::new(build_file_list_rows(
                items.as_ref(),
                &selection,
                &collapsed_paths,
            ))
        },
    ));
    let rows = rows.read().clone();
    let row_count = rows.len();
    let scroll_top_snapshot = *scroll_top.read();
    let viewport_height_snapshot = *viewport_height.read();
    let virtual_scroll =
        file_list_virtual_scroll(row_count, scroll_top_snapshot, viewport_height_snapshot);
    let visible_rows = file_list_visible_window(&rows, virtual_scroll, scroll_top_snapshot);

    use_effect(use_reactive(&suppress_resize_observer, move |suppressed| {
        if suppressed {
            return;
        }
        let Some(event) = mounted.peek().clone() else {
            return;
        };
        spawn(async move {
            if let Ok(rect) = event.get_client_rect().await {
                update_file_list_viewport_height(viewport_height, rect.height());
            }
        });
    }));

    rsx! {
        div {
            class: "file-list",
            role: "tree",
            aria_label: i18n.t("file-list-loaded-files"),
            style: "--file-list-row-height: {FILE_LIST_ROW_HEIGHT}px",
            onmounted: move |event| {
                mounted.set(Some(event.clone()));
                async move {
                    if let Ok(rect) = event.get_client_rect().await {
                    update_file_list_viewport_height(viewport_height, rect.height());
                    }
                }
            },
            onresize: move |event| {
                if suppress_resize_observer {
                    return;
                }
                if let Ok(size) = event.get_content_box_size() {
                    update_file_list_viewport_height(viewport_height, size.height);
                }
            },
            onscroll: move |event| scroll_top.set(event.scroll_top()),
            if rows.is_empty() {
                div { class: "file-list-empty", "{empty_message}" }
            } else {
                div {
                    class: "file-list-virtual-space",
                    style: "height: {virtual_scroll.content_height}px",
                    for (row_top, row) in visible_rows {
                        div {
                            key: "{file_list_row_key(&row)}",
                            class: "file-list-virtual-row",
                            style: "transform: translateY({row_top}px)",
                            FileListRowView {
                                row,
                                i18n,
                                on_open_file,
                                on_activate,
                                on_remove,
                                on_remove_path,
                                on_toggle_selected,
                                on_toggle_path,
                                on_toggle_collapsed: EventHandler::new(move |path: String| {
                                    let mut collapsed = collapsed_paths.write();
                                    if !collapsed.insert(path.clone()) {
                                        collapsed.remove(&path);
                                    }
                                })
                            }
                        }
                    }
                }
            }
        }
    }
}

fn file_list_visible_window(
    rows: &[FileListRow],
    virtual_scroll: FileListVirtualScroll,
    scroll_top: f64,
) -> Vec<(i64, FileListRow)> {
    let take_count = virtual_scroll
        .end_row
        .saturating_sub(virtual_scroll.start_row);

    rows.iter()
        .enumerate()
        .skip(virtual_scroll.start_row)
        .take(take_count)
        .map(|(row_index, row)| {
            (
                file_list_virtual_row_top(row_index, scroll_top, virtual_scroll),
                row.clone(),
            )
        })
        .collect()
}

fn update_file_list_viewport_height(mut viewport_height: Signal<usize>, height: f64) {
    let next = measured_file_list_viewport_height(height);
    if next != *viewport_height.read() {
        viewport_height.set(next);
    }
}

#[component]
fn FileListRowView(
    row: FileListRow,
    i18n: I18n,
    on_open_file: EventHandler<usize>,
    on_activate: EventHandler<usize>,
    on_remove: EventHandler<String>,
    on_remove_path: EventHandler<String>,
    on_toggle_selected: EventHandler<(String, bool)>,
    on_toggle_path: EventHandler<(String, bool)>,
    on_toggle_collapsed: EventHandler<String>,
) -> Element {
    match row {
        FileListRow::Folder {
            name,
            path,
            depth,
            count,
            selected_count,
            collapsed,
            ..
        } => {
            let checked = count > 0 && selected_count == count;
            let partial = selected_count > 0 && selected_count < count;
            let folder_class = if partial {
                "file-tree-folder-check partial"
            } else {
                "file-tree-folder-check"
            };
            let row_class = if collapsed {
                "file-tree-folder-row collapsed"
            } else {
                "file-tree-folder-row"
            };
            let count_label = i18n.t_args("file-list-file-count", &[("count", count.to_string())]);
            let indent = depth * 14;
            let toggle_path = path.clone();
            let select_path = path.clone();
            let remove_path = path.clone();

            rsx! {
                div {
                    class: "{row_class}",
                    role: "treeitem",
                    aria_expanded: !collapsed,
                    style: "--file-tree-indent: {indent}px",
                    button {
                        r#type: "button",
                        class: "file-tree-disclosure",
                        aria_label: "{path}",
                        onclick: move |_| on_toggle_collapsed.call(toggle_path.clone())
                    }
                    label {
                        class: "{folder_class}",
                        onclick: move |event| event.stop_propagation(),
                        input {
                            r#type: "checkbox",
                            checked,
                            aria_label: i18n.t_args("aria-select-file", &[("name", path.clone())]),
                            onclick: move |event| event.stop_propagation(),
                            onchange: move |event| on_toggle_path.call((select_path.clone(), event.checked()))
                        }
                    }
                    span { class: "file-tree-folder-name", "{name}" }
                    span { class: "file-tree-folder-count", "{count_label}" }
                    button {
                        class: "file-tree-folder-remove",
                        title: i18n.t("title-remove-path"),
                        aria_label: i18n.t("title-remove-path"),
                        onmousedown: move |event| {
                            event.prevent_default();
                            event.stop_propagation();
                        },
                        onclick: move |event| {
                            event.prevent_default();
                            event.stop_propagation();
                            on_remove_path.call(remove_path.clone());
                        },
                        "×"
                    }
                }
            }
        }
        FileListRow::File {
            name,
            item,
            depth,
            selected,
            ..
        } => {
            let selected_key = item.key.clone();
            let toggle_key = item.key.clone();
            let tab_id = item.tab_id;
            let file_id = item.file_id;
            let title = item.path.clone();
            let active = item.active;
            let indent = depth * 14;
            rsx! {
                div {
                    class: if active { "file-item active" } else { "file-item" },
                    role: "treeitem",
                    aria_selected: active,
                    style: "--file-tree-indent: {indent}px",
                    label { class: "file-item-check",
                        input {
                            r#type: "checkbox",
                            checked: selected,
                            aria_label: i18n.t_args("aria-select-file", &[("name", item.path.clone())]),
                            onclick: move |event| event.stop_propagation(),
                            onchange: move |event| on_toggle_selected.call((toggle_key.clone(), event.checked()))
                        }
                    }
                    button {
                        class: "file-item-main",
                        title: "{title}",
                        onclick: move |_| {
                            if let Some(tab_id) = tab_id {
                                on_activate.call(tab_id);
                            } else if let Some(file_id) = file_id {
                                on_open_file.call(file_id);
                            }
                        },
                        span { class: "file-name",
                            "{name}"
                            if item.dirty {
                                span { class: "file-dirty", "*" }
                            }
                        }
                        span { class: "file-detail", "{item.detail}" }
                    }
                    button {
                        class: "file-item-close",
                        title: i18n.t("title-remove-file"),
                        aria_label: i18n.t("title-remove-file"),
                        onclick: move |event| {
                            event.stop_propagation();
                            on_remove.call(selected_key.clone());
                        },
                        "×"
                    }
                }
            }
        }
    }
}
