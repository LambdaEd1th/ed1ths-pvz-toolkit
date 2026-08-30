use dioxus::prelude::*;

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
    #[props(default = "工具栏".to_string())] label: String,
    #[props(default = "展开工具栏".to_string())] open_label: String,
    #[props(default = "收起工具栏".to_string())] close_label: String,
) -> Element {
    let mut open = use_signal(|| false);
    let is_open = open();
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
            class: root_class,
            onkeydown: move |event| {
                if event.key() == Key::Escape && open() {
                    event.stop_propagation();
                    open.set(false);
                }
            },
            if is_open {
                div {
                    class: "ui-tool-drawer-backdrop",
                    aria_hidden: "true",
                    onclick: move |_| open.set(false),
                }
            }
            aside {
                class: "ui-tool-drawer-panel",
                aria_hidden: !is_open,
                aria_label: panel_label,
                div { class: "ui-tool-drawer-header",
                    strong { {header_label} }
                    button {
                        r#type: "button",
                        class: "ui-tool-drawer-close",
                        title: close_button_title,
                        aria_label: close_button_label,
                        onclick: move |event| {
                            event.stop_propagation();
                            open.set(false);
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
                r#type: "button",
                class: "ui-tool-drawer-handle",
                title: toggle_label.clone(),
                aria_label: toggle_label,
                aria_expanded: is_open,
                onclick: move |event| {
                    event.stop_propagation();
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
    if !open {
        return rsx! {};
    }

    rsx! {
        div { class: "ui-context-sheet-layer ui-context-sheet-layer--{side} {class}",
            button {
                r#type: "button",
                class: "ui-context-sheet-backdrop",
                aria_label: "{close_label}",
                onclick: move |_| on_close.call(()),
            }
            aside {
                class: "ui-context-sheet",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{title}",
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
