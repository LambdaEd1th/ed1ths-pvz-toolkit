use dioxus::prelude::*;
use rton_editor_core::{ValueSearchMatch, ValueSearchResult};

use crate::domain::{
    IdentityArc, VALUE_SEARCH_DEFAULT_VIEWPORT_HEIGHT, measured_value_search_viewport_height,
    value_search_virtual_row_top, value_search_virtual_scroll,
};
use crate::i18n::I18n;

#[component]
pub(crate) fn ValueSearchResults(
    result: IdentityArc<ValueSearchResult>,
    i18n: I18n,
    on_select: EventHandler<String>,
    suppress_resize_observer: bool,
) -> Element {
    let mut scroll_top = use_signal(|| 0_f64);
    let viewport_height = use_signal(|| VALUE_SEARCH_DEFAULT_VIEWPORT_HEIGHT);
    let mut mounted = use_signal(|| None::<MountedEvent>);
    let match_count = result.matches.len();
    let scroll_top_snapshot = *scroll_top.read();
    let viewport_height_snapshot = *viewport_height.read();
    let virtual_scroll =
        value_search_virtual_scroll(match_count, scroll_top_snapshot, viewport_height_snapshot);
    let visible_matches = result
        .matches
        .iter()
        .enumerate()
        .skip(virtual_scroll.start_row)
        .take(
            virtual_scroll
                .end_row
                .saturating_sub(virtual_scroll.start_row),
        )
        .map(|(match_index, search_match)| {
            (
                value_search_virtual_row_top(match_index, scroll_top_snapshot, virtual_scroll),
                search_match.clone(),
            )
        })
        .collect::<Vec<_>>();

    use_effect(use_reactive(&suppress_resize_observer, move |suppressed| {
        if suppressed {
            return;
        }
        let Some(event) = mounted.peek().clone() else {
            return;
        };
        spawn(async move {
            if let Ok(rect) = event.get_client_rect().await {
                update_value_search_viewport_height(viewport_height, rect.height());
            }
        });
    }));

    if result.matches.is_empty() {
        let message = if result.done {
            i18n.t("inspector-no-matches")
        } else {
            i18n.t_args(
                "inspector-searching-scanned",
                &[("count", result.scanned.to_string())],
            )
        };

        return rsx! {
            div { class: "value-search-results",
                div { class: "value-search-empty", "{message}" }
            }
        };
    }

    let summary = if result.capped {
        i18n.t_args(
            "inspector-capped",
            &[("limit", result.matches.len().to_string())],
        )
    } else if result.done {
        i18n.t_args(
            "inspector-done-summary",
            &[
                ("matches", result.matches.len().to_string()),
                ("scanned", result.scanned.to_string()),
            ],
        )
    } else {
        i18n.t_args(
            "inspector-searching-summary",
            &[("matches", result.matches.len().to_string())],
        )
    };
    let query = result.query.clone();

    rsx! {
        div { class: "value-search-results",
            div { class: "value-search-summary",
                "{summary} · {query}"
            }
            div {
                class: "value-search-scroll",
                onmounted: move |event| {
                    mounted.set(Some(event.clone()));
                    async move {
                        if let Ok(rect) = event.get_client_rect().await {
                            update_value_search_viewport_height(viewport_height, rect.height());
                        }
                    }
                },
                onresize: move |event| {
                    if suppress_resize_observer {
                        return;
                    }
                    if let Ok(size) = event.get_content_box_size() {
                        update_value_search_viewport_height(viewport_height, size.height);
                    }
                },
                onscroll: move |event| scroll_top.set(event.scroll_top()),
                div {
                    class: "value-search-virtual-space",
                    style: "height: {virtual_scroll.content_height}px",
                    for (row_top, search_match) in visible_matches {
                        div {
                            class: "value-search-virtual-row",
                            style: "transform: translateY({row_top}px)",
                            ValueSearchResultRow {
                                search_match,
                                on_select
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ValueSearchResultRow(
    search_match: ValueSearchMatch,
    on_select: EventHandler<String>,
) -> Element {
    let path = search_match.path.clone();
    let button_path = path.clone();
    let display_path = search_match.display_path.clone();
    let preview = search_match.preview.clone();
    let title = format!("{display_path} - {preview}");

    rsx! {
        div {
            class: "rton-search-result-row",
            title: "{title}",
            onclick: move |_| on_select.call(path.clone()),
            button {
                r#type: "button",
                class: "rton-search-result-path",
                title: "{display_path}",
                onclick: move |event| {
                    event.stop_propagation();
                    on_select.call(button_path.clone());
                },
                "{display_path}"
            }
            span {
                class: "rton-search-result-preview",
                title: "{preview}",
                "{preview}"
            }
        }
    }
}

fn update_value_search_viewport_height(mut viewport_height: Signal<usize>, height: f64) {
    let next = measured_value_search_viewport_height(height);
    if next != *viewport_height.read() {
        viewport_height.set(next);
    }
}
