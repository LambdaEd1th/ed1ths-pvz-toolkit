use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::focus::{use_modal_focus, use_toolbar_labels};

const TOOL_DRAWER_HANDLE_DRAG_THRESHOLD: f64 = 4.0;

static NEXT_MODAL_ID: AtomicU64 = AtomicU64::new(1);

fn next_modal_id(prefix: &str) -> String {
    format!("{prefix}-{}", NEXT_MODAL_ID.fetch_add(1, Ordering::Relaxed))
}

fn handle_moved(start_y: Option<f64>, current_y: f64) -> bool {
    start_y.is_some_and(|start_y| (current_y - start_y).abs() >= TOOL_DRAWER_HANDLE_DRAG_THRESHOLD)
}

fn close_tool_drawer(mut open: Signal<bool>, handle_id: &str) {
    open.set(false);
    if let Ok(handle_id) = serde_json::to_string(handle_id) {
        let _ = document::eval(&format!("document.getElementById({handle_id})?.focus();"));
    }
}

#[component]
pub fn ToolPage(
    namespace: &'static str,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "ui-tool-page ui-tool-page--{namespace} {class}",
            "data-tool-page": namespace,
            {children}
        }
    }
}

#[component]
pub fn ToolPageToolbar(
    actions: Element,
    #[props(default)] class: String,
    #[props(default)] label: Option<String>,
    #[props(default)] open_label: Option<String>,
    #[props(default)] close_label: Option<String>,
) -> Element {
    let labels = use_toolbar_labels();
    let label = label.unwrap_or(labels.label);
    let open_label = open_label.unwrap_or(labels.open_label);
    let close_label = close_label.unwrap_or(labels.close_label);
    let layer_id = use_hook(|| next_modal_id("tk-tool-toolbar"));
    let layer_selector = format!("#{layer_id}");
    let panel_id = format!("{layer_id}-panel");
    let handle_id = format!("{layer_id}-handle");
    let handle_id_for_keydown = handle_id.clone();
    let handle_id_for_backdrop = handle_id.clone();
    let handle_id_for_close = handle_id.clone();
    let mut open = use_signal(|| false);
    let mut handle_drag_start = use_signal(|| None::<f64>);
    let mut handle_dragged = use_signal(|| false);
    let is_open = open();
    use_modal_focus(
        &layer_id,
        is_open,
        &layer_selector,
        Some(".ui-tool-drawer-backdrop"),
        None,
    );
    let root_class = if is_open {
        format!("ui-tool-page-toolbar is-open {class}")
    } else {
        format!("ui-tool-page-toolbar {class}")
    };
    let toggle_label = if is_open {
        close_label.clone()
    } else {
        open_label
    };
    let panel_label = label.clone();
    let header_label = label.clone();
    let handle_label = label;
    let close_button_title = close_label.clone();
    let close_button_label = close_label;

    rsx! {
        section {
            id: "{layer_id}",
            class: root_class,
            onkeydown: move |event| {
                if event.key() == Key::Escape && open() {
                    event.prevent_default();
                    event.stop_propagation();
                    close_tool_drawer(open, &handle_id_for_keydown);
                }
            },
            if is_open {
                div {
                    class: "ui-tool-drawer-backdrop",
                    aria_hidden: "true",
                    onclick: move |_| close_tool_drawer(open, &handle_id_for_backdrop),
                }
            }
            aside {
                id: "{panel_id}",
                class: "ui-tool-drawer-panel",
                aria_hidden: !is_open,
                aria_label: panel_label,
                "data-modal-content": "true",
                div { class: "ui-tool-drawer-header",
                    strong { {header_label} }
                    button {
                        r#type: "button",
                        class: "ui-tool-drawer-close",
                        title: close_button_title,
                        aria_label: close_button_label,
                        onclick: move |event| {
                            event.stop_propagation();
                            close_tool_drawer(open, &handle_id_for_close);
                        },
                        svg {
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2",
                            stroke_linecap: "round",
                            path { d: "M6 6l12 12" }
                            path { d: "M18 6 6 18" }
                        }
                    }
                }
                div { class: "ui-tool-drawer-body",
                    {actions}
                }
            }
            button {
                id: "{handle_id}",
                r#type: "button",
                class: "ui-tool-drawer-handle",
                title: toggle_label.clone(),
                aria_label: toggle_label,
                aria_expanded: is_open,
                aria_controls: "{panel_id}",
                "data-modal-restore-focus": "true",
                onmounted: move |_| {
                    let _ = document::eval("window.toolkitToolDrawerHandleDrag?.refresh?.();");
                },
                onpointerdown: move |event| {
                    if !event.is_primary()
                        || !matches!(
                            event.trigger_button(),
                            None | Some(MouseButton::Primary)
                        )
                    {
                        return;
                    }
                    event.stop_propagation();
                    handle_drag_start.set(Some(event.client_coordinates().y));
                    handle_dragged.set(false);
                },
                onpointermove: move |event| {
                    if handle_moved(handle_drag_start(), event.client_coordinates().y) {
                        event.prevent_default();
                        event.stop_propagation();
                        handle_dragged.set(true);
                    }
                },
                onpointerup: move |event| {
                    event.stop_propagation();
                    handle_drag_start.set(None);
                },
                onpointercancel: move |_| {
                    handle_drag_start.set(None);
                    handle_dragged.set(false);
                },
                onlostpointercapture: move |_| {
                    handle_drag_start.set(None);
                },
                onclick: move |event| {
                    event.stop_propagation();
                    if handle_dragged() {
                        handle_dragged.set(false);
                        return;
                    }
                    open.toggle();
                },
                svg {
                    class: "ui-tool-drawer-toolbar-icon",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    path { d: "M4 6h10" }
                    path { d: "M18 6h2" }
                    path { d: "M4 12h2" }
                    path { d: "M10 12h10" }
                    path { d: "M4 18h7" }
                    path { d: "M15 18h5" }
                    circle { cx: "16", cy: "6", r: "2" }
                    circle { cx: "8", cy: "12", r: "2" }
                    circle { cx: "13", cy: "18", r: "2" }
                }
                span { class: "ui-tool-drawer-handle-label", {handle_label} }
                svg {
                    class: "ui-tool-drawer-chevron",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    path { d: "m9 5 7 7-7 7" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::handle_moved;

    #[test]
    fn toolbar_handle_requires_a_real_drag_before_suppressing_click() {
        assert!(!handle_moved(None, 100.0));
        assert!(!handle_moved(Some(100.0), 103.99));
        assert!(handle_moved(Some(100.0), 104.0));
        assert!(handle_moved(Some(100.0), 92.0));
    }
}

#[component]
pub fn WorkspaceCard(
    #[props(default)] class: String,
    #[props(default)] aria_label: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        article {
            class: "ui-island ui-workspace-card {class}",
            aria_label,
            {children}
        }
    }
}

#[component]
pub fn DropIndicator(title: String) -> Element {
    rsx! {
        div {
            class: "ui-workspace-drop-indicator",
            aria_hidden: "true",
            span { class: "ui-workspace-drop-indicator-icon",
                svg {
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    path { d: "M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z" }
                    polyline { points: "14 2 14 8 20 8" }
                    path { d: "M12 18v-6" }
                    path { d: "m9 15 3-3 3 3" }
                }
            }
            strong { "{title}" }
        }
    }
}

#[component]
pub fn ContextSheet(
    open: bool,
    title: String,
    close_label: String,
    on_close: EventHandler<()>,
    #[props(default = "right".to_string())] side: String,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    let layer_id = use_hook(|| next_modal_id("tk-context-sheet"));
    let layer_selector = format!("#{layer_id}");
    use_modal_focus(&layer_id, open, &layer_selector, None, None);

    if !open {
        return rsx! {};
    }

    let close_backdrop = on_close;
    let close_escape = on_close;
    rsx! {
        div {
            id: "{layer_id}",
            class: "ui-context-sheet-layer ui-context-sheet-layer--{side} {class}",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    event.stop_propagation();
                    close_escape.call(());
                }
            },
            button {
                r#type: "button",
                class: "ui-context-sheet-backdrop",
                aria_label: "{close_label}",
                onclick: move |_| close_backdrop.call(()),
            }
            aside {
                class: "ui-context-sheet",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{title}",
                tabindex: "-1",
                "data-modal-content": "true",
                div { class: "ui-context-sheet-grabber", aria_hidden: "true" }
                {children}
            }
        }
    }
}

#[component]
pub fn InlineNotice(
    #[props(default)] class: String,
    #[props(default = "neutral".to_string())] tone: String,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "ui-inline-notice ui-inline-notice--{tone} {class}",
            role: "status",
            {children}
        }
    }
}
