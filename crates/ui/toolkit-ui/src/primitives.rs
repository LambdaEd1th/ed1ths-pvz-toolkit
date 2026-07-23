use dioxus::prelude::*;

const TOKENS_CSS: Asset = asset!("/assets/tokens.css");
const PRIMITIVES_CSS: Asset = asset!("/assets/primitives.css");

#[component]
pub fn UiStyles() -> Element {
    rsx! {
        document::Stylesheet { href: TOKENS_CSS }
        document::Stylesheet { href: PRIMITIVES_CSS }
    }
}

#[component]
pub fn Island(
    #[props(default)] class: String,
    #[props(default)] aria_label: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        section {
            class: "ui-island {class}",
            aria_label,
            {children}
        }
    }
}

#[component]
pub fn IconButton(
    label: String,
    #[props(default)] class: String,
    #[props(default)] title: Option<String>,
    #[props(default)] pressed: Option<bool>,
    onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        button {
            class: "ui-icon-button {class}",
            aria_label: "{label}",
            aria_pressed: pressed,
            title,
            onclick: move |event| onclick.call(event),
            {children}
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SegmentedOption {
    pub value: &'static str,
    pub label: &'static str,
}

#[component]
pub fn SegmentedControl(
    value: String,
    options: Vec<SegmentedOption>,
    aria_label: String,
    onchange: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "ui-segmented", role: "group", aria_label,
            for option in options {
                button {
                    class: if value == option.value { "ui-segmented-item is-active" } else { "ui-segmented-item" },
                    aria_pressed: value == option.value,
                    onclick: move |_| onchange.call(option.value.to_string()),
                    "{option.label}"
                }
            }
        }
    }
}
