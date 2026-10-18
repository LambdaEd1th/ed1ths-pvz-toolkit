use dioxus::prelude::*;

const TOKENS_CSS: Asset = asset!("/assets/tokens.css");
const PRIMITIVES_CSS: Asset = asset!("/assets/primitives.css");
const MODAL_FOCUS_JS: Asset = asset!("/assets/modal-focus.js");
const TOOL_DRAWER_JS: Asset = asset!("/assets/tool-drawer.js");
const TOOLTIPS_JS: Asset = asset!("/assets/tooltips.js");

#[component]
pub fn UiStyles() -> Element {
    rsx! {
        document::Script { src: MODAL_FOCUS_JS }
        document::Script { src: TOOL_DRAWER_JS }
        document::Script { src: TOOLTIPS_JS }
        document::Stylesheet { href: TOKENS_CSS }
        document::Stylesheet { href: PRIMITIVES_CSS }
    }
}

#[component]
pub fn PillTabs(#[props(default)] class: String, aria_label: String, children: Element) -> Element {
    rsx! {
        div {
            class: "ui-pill-tabs {class}",
            role: "tablist",
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
    #[props(default)] expanded: Option<bool>,
    #[props(default)] controls: Option<String>,
    onclick: EventHandler<MouseEvent>,
    children: Element,
) -> Element {
    rsx! {
        button {
            class: "ui-icon-button {class}",
            aria_label: "{label}",
            aria_pressed: pressed,
            aria_expanded: expanded,
            aria_controls: controls,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub disabled: bool,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[component]
pub fn Select(
    options: Vec<SelectOption>,
    value: String,
    onchange: EventHandler<String>,
    #[props(default)] aria_label: String,
    #[props(default)] id: String,
    #[props(default)] title: String,
    #[props(default)] disabled: bool,
    #[props(default)] class: String,
    #[props(default)] compact: bool,
) -> Element {
    let mut open = use_signal(|| false);
    let selected = options
        .iter()
        .find(|option| option.value == value)
        .map(|option| option.label.clone())
        .unwrap_or_default();
    let trigger_class = if compact {
        "ui-select-trigger is-compact"
    } else {
        "ui-select-trigger"
    };
    rsx! {
        div {
            class: "ui-select {class}",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    open.set(false);
                }
            },
            button {
                r#type: "button",
                id: "{id}",
                class: "{trigger_class}",
                title: "{title}",
                aria_label: "{aria_label}",
                role: "combobox",
                aria_haspopup: "listbox",
                aria_expanded: *open.read(),
                disabled,
                onclick: move |_| {
                    if !disabled {
                        open.toggle();
                    }
                },
                span { class: "ui-select-value", "{selected}" }
                span { class: "ui-select-caret", aria_hidden: "true",
                    svg {
                        view_box: "0 0 24 24",
                        width: "12",
                        height: "12",
                        path { d: "m6 9 6 6 6-6", fill: "none", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round", stroke_linejoin: "round" }
                    }
                }
            }
            if *open.read() {
                button {
                    r#type: "button",
                    class: "ui-select-backdrop",
                    aria_label: "关闭选项",
                    onclick: move |_| open.set(false),
                }
                div { class: "ui-select-menu", role: "listbox", aria_label: "{aria_label}",
                    for option in options {
                        {
                            let option_value = option.value.clone();
                            let active = option.value == value;
                            let option_class = if active { "ui-select-option is-active" } else { "ui-select-option" };
                            rsx! {
                                button {
                                    r#type: "button",
                                    class: "{option_class}",
                                    role: "option",
                                    aria_selected: active,
                                    disabled: option.disabled,
                                    onclick: move |_| {
                                        onchange.call(option_value.clone());
                                        open.set(false);
                                    },
                                    span { class: "ui-select-option-label", "{option.label}" }
                                    span { class: "ui-select-option-check", aria_hidden: "true",
                                        if active {
                                            svg {
                                                view_box: "0 0 24 24",
                                                width: "13",
                                                height: "13",
                                                path { d: "m5 12 4 4L19 6", fill: "none", stroke: "currentColor", stroke_width: "2.2", stroke_linecap: "round", stroke_linejoin: "round" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
