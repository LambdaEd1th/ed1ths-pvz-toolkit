use dioxus::prelude::*;

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
