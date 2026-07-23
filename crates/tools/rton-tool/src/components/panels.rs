use dioxus::prelude::*;

use crate::i18n::I18n;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanelSide {
    Left,
    Right,
}

impl PanelSide {
    fn class(self) -> &'static str {
        match self {
            PanelSide::Left => "left",
            PanelSide::Right => "right",
        }
    }

    fn resize_label_key(self) -> &'static str {
        match self {
            PanelSide::Left => "resize-file-list",
            PanelSide::Right => "resize-file-properties",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PanelResizeDrag {
    pub(crate) side: PanelSide,
    pub(crate) start_x: f64,
    pub(crate) start_width: i32,
}

fn panel_resize_handle_class(side: PanelSide, dragging: bool) -> String {
    let mut class_name = format!("rton-resize-handle rton-resize-handle-{}", side.class());
    if dragging {
        class_name.push_str(" dragging");
    }
    class_name
}

#[component]
pub(crate) fn PanelResizeHandle(
    side: PanelSide,
    width: i32,
    dragging: bool,
    i18n: I18n,
    on_start: EventHandler<PanelResizeDrag>,
) -> Element {
    let class_name = panel_resize_handle_class(side, dragging);

    rsx! {
        div {
            class: "{class_name}",
            role: "separator",
            aria_orientation: "vertical",
            aria_label: i18n.t(side.resize_label_key()),
            onmousedown: move |event| {
                event.prevent_default();
                on_start.call(PanelResizeDrag {
                    side,
                    start_x: event.client_coordinates().x,
                    start_width: width,
                });
            }
        }
    }
}

#[component]
pub(crate) fn PanelHeader(
    icon: Element,
    title: String,
    subtitle: String,
    children: Element,
) -> Element {
    rsx! {
        header { class: "panel-header",
            div { class: "panel-header-main",
                div { class: "panel-header-title-line",
                    span { class: "panel-header-icon", {icon} }
                    h2 { "{title}" }
                }
                p { "{subtitle}" }
            }
            div { class: "panel-header-below", {children} }
        }
    }
}

#[component]
pub(crate) fn MetaItem(label: String, value: String) -> Element {
    rsx! {
        div { class: "meta-item",
            dt { "{label}" }
            dd { "{value}" }
        }
    }
}
