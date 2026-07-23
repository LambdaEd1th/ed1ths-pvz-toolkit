use dioxus::prelude::*;
use rton_editor_core::{TreeRows, ValueRow};
use std::collections::HashSet;

use crate::domain::{
    IdentityArc, VALUE_TREE_DEFAULT_VIEWPORT_HEIGHT, ValueTreeVirtualScroll,
    measured_value_tree_viewport_height, value_tree_virtual_row_top, value_tree_virtual_scroll,
};
use crate::i18n::I18n;

#[component]
pub(crate) fn ValueTree(
    rows: IdentityArc<TreeRows>,
    expanded_paths: IdentityArc<HashSet<String>>,
    selected_path: String,
    i18n: I18n,
    on_select: EventHandler<String>,
    on_toggle: EventHandler<String>,
    suppress_resize_observer: bool,
) -> Element {
    let mut scroll_top = use_signal(|| 0_f64);
    let viewport_height = use_signal(|| VALUE_TREE_DEFAULT_VIEWPORT_HEIGHT);
    let mut mounted = use_signal(|| None::<MountedEvent>);
    let row_count = rows.rows.len();
    let scroll_top_snapshot = *scroll_top.read();
    let viewport_height_snapshot = *viewport_height.read();
    let virtual_scroll =
        value_tree_virtual_scroll(row_count, scroll_top_snapshot, viewport_height_snapshot);
    let visible_rows = value_tree_visible_window(&rows.rows, virtual_scroll, scroll_top_snapshot);

    use_effect(use_reactive(&suppress_resize_observer, move |suppressed| {
        if suppressed {
            return;
        }
        let Some(event) = mounted.peek().clone() else {
            return;
        };
        spawn(async move {
            if let Ok(rect) = event.get_client_rect().await {
                update_value_tree_viewport_height(viewport_height, rect.height());
            }
        });
    }));

    rsx! {
        div {
            class: "value-tree",
            onmounted: move |event| {
                mounted.set(Some(event.clone()));
                async move {
                    if let Ok(rect) = event.get_client_rect().await {
                        update_value_tree_viewport_height(viewport_height, rect.height());
                    }
                }
            },
            onresize: move |event| {
                if suppress_resize_observer {
                    return;
                }
                if let Ok(size) = event.get_content_box_size() {
                    update_value_tree_viewport_height(viewport_height, size.height);
                }
            },
            onscroll: move |event| scroll_top.set(event.scroll_top()),
            if rows.rows.is_empty() {
                div { class: "empty-state compact",
                    {i18n.t("value-tree-no-matches")}
                }
            } else {
                div {
                    class: "value-tree-virtual-space",
                    style: "height: {virtual_scroll.content_height}px",
                    for (row_top, row) in visible_rows {
                        {
                            let is_collapsed = row.child_count > 0 && !expanded_paths.contains(&row.path);
                            rsx! {
                                div {
                                    class: "value-tree-virtual-row",
                                    style: "transform: translateY({row_top}px)",
                                    ValueTreeRow {
                                        selected: row.path == selected_path,
                                        collapsed: is_collapsed,
                                        row,
                                        on_select,
                                        on_toggle_collapsed: on_toggle
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if rows.truncated {
                div { class: "tree-truncated",
                    {i18n.t("value-tree-truncated")}
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn value_tree_visible_indices(
    rows: &[ValueRow],
    collapsed_paths: &[String],
) -> Option<Vec<usize>> {
    if collapsed_paths.is_empty() {
        return None;
    }

    let collapsed_paths = collapsed_paths
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut indices = Vec::with_capacity(rows.len());
    let mut hidden_depth = None::<usize>;

    for (index, row) in rows.iter().enumerate() {
        if let Some(depth) = hidden_depth {
            if row.depth > depth {
                continue;
            }
            hidden_depth = None;
        }

        indices.push(index);
        if row.child_count > 0 && collapsed_paths.contains(row.path.as_str()) {
            hidden_depth = Some(row.depth);
        }
    }

    Some(indices)
}

fn value_tree_visible_window(
    rows: &[ValueRow],
    virtual_scroll: ValueTreeVirtualScroll,
    scroll_top: f64,
) -> Vec<(i64, ValueRow)> {
    let take_count = virtual_scroll
        .end_row
        .saturating_sub(virtual_scroll.start_row);

    rows.iter()
        .enumerate()
        .skip(virtual_scroll.start_row)
        .take(take_count)
        .map(|(row_index, row)| {
            (
                value_tree_virtual_row_top(row_index, scroll_top, virtual_scroll),
                row.clone(),
            )
        })
        .collect()
}

fn update_value_tree_viewport_height(mut viewport_height: Signal<usize>, height: f64) {
    let next = measured_value_tree_viewport_height(height);
    if next != *viewport_height.read() {
        viewport_height.set(next);
    }
}

#[component]
fn ValueTreeRow(
    row: ValueRow,
    selected: bool,
    collapsed: bool,
    on_select: EventHandler<String>,
    on_toggle_collapsed: EventHandler<String>,
) -> Element {
    let path = row.path.clone();
    let toggle_path = row.path.clone();
    let class = match (selected, row.child_count > 0, row.depth > 0, collapsed) {
        (true, true, true, true) => "value-row selected has-children has-guide collapsed",
        (true, true, true, false) => "value-row selected has-children has-guide",
        (true, true, false, true) => "value-row selected has-children collapsed",
        (true, true, false, false) => "value-row selected has-children",
        (true, false, true, _) => "value-row selected is-leaf has-guide",
        (true, false, false, _) => "value-row selected is-leaf",
        (false, true, true, true) => "value-row has-children has-guide collapsed",
        (false, true, true, false) => "value-row has-children has-guide",
        (false, true, false, true) => "value-row has-children collapsed",
        (false, true, false, false) => "value-row has-children",
        (false, false, true, _) => "value-row is-leaf has-guide",
        (false, false, false, _) => "value-row is-leaf",
    };
    let indent = row.depth * 16;
    let display_label = if row.path == "$" {
        "$".to_string()
    } else {
        row.label.clone()
    };

    rsx! {
        div {
            class,
            role: "treeitem",
            aria_selected: selected,
            tabindex: "0",
            style: "--value-indent: {indent}px",
            onclick: move |_| on_select.call(path.clone()),
            if row.child_count > 0 {
                button {
                    r#type: "button",
                    class: "row-disclosure",
                    aria_expanded: !collapsed,
                    onclick: move |event| {
                        event.stop_propagation();
                        on_toggle_collapsed.call(toggle_path.clone());
                    }
                }
            } else {
                span { class: "row-disclosure leaf", aria_hidden: "true" }
            }
            span { class: "row-label", "{display_label}" }
            span { class: "row-kind", "{row.kind}" }
            if row.child_count > 0 {
                span { class: "row-spacer", aria_hidden: "true" }
                span { class: "row-count", "{row.child_count}" }
            } else {
                span { class: "row-preview", "{row.preview}" }
            }
        }
    }
}
