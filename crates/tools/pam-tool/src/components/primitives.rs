use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdCheck, LdChevronDown};
use dioxus_free_icons::{Icon, IconShape};

pub fn icon<T>(shape: T) -> Element
where
    T: IconShape + Clone + PartialEq + 'static,
{
    rsx! {
        Icon {
            class: "pam-icon",
            width: 16,
            height: 16,
            fill: "currentColor",
            icon: shape,
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
}

#[component]
pub fn SelectControl(
    value: String,
    options: Vec<SelectOption>,
    onchange: EventHandler<String>,
    #[props(default = false)] disabled: bool,
    #[props(default = false)] compact: bool,
    #[props(default = String::new())] title: String,
) -> Element {
    let mut open = use_signal(|| false);
    let mut size = use_signal(move || [if compact { 92.0_f64 } else { 148.0_f64 }, 30.0_f64]);
    let mut anchor = use_signal(|| [8.0_f64, 46.0_f64]);
    let selected = options
        .iter()
        .find(|option| option.value == value)
        .map(|option| option.label.clone())
        .unwrap_or_default();
    let class_name = if compact {
        "pam-select compact"
    } else {
        "pam-select"
    };
    let [anchor_x, anchor_y] = *anchor.read();
    let [width, height] = *size.read();
    let menu_style = format!(
        "--select-x:{anchor_x}px;--select-y:{}px;--select-width:{}px",
        anchor_y + height + 6.0,
        width.max(if compact { 92.0 } else { 148.0 })
    );
    let select_button = rsx! {
        button {
            r#type: "button",
            class: "{class_name}",
            title: "{title}",
            disabled,
            aria_haspopup: "listbox",
            aria_expanded: *open.read(),
            onresize: move |event| {
                if let Ok(box_size) = event.get_content_box_size() {
                    size.set([box_size.width, box_size.height]);
                }
            },
            onclick: move |event| {
                if disabled {
                    return;
                }
                let client = event.client_coordinates();
                let element = event.element_coordinates();
                anchor.set([client.x - element.x, client.y - element.y]);
                open.toggle();
            },
            span { class: "pam-select-value", "{selected}" }
            span { class: "pam-select-caret", {icon(LdChevronDown)} }
        }
    };

    rsx! {
        div { class: "pam-select-shell",
            {select_button}
            if *open.read() {
                button {
                    r#type: "button",
                    class: "pam-select-backdrop",
                    aria_label: "Close",
                    onclick: move |_| open.set(false),
                }
                div {
                    class: "pam-select-menu",
                    style: "{menu_style}",
                    role: "listbox",
                    for option in options {
                        {
                            let option_value = option.value.clone();
                            let active = option.value == value;
                            let option_class = if active {
                                "pam-select-option active"
                            } else {
                                "pam-select-option"
                            };
                            rsx! {
                                button {
                                    r#type: "button",
                                    class: "{option_class}",
                                    disabled: option.disabled,
                                    role: "option",
                                    aria_selected: active,
                                    onclick: move |_| {
                                        onchange.call(option_value.clone());
                                        open.set(false);
                                    },
                                    span { class: "pam-select-option-label", "{option.label}" }
                                    span { class: "pam-select-option-check",
                                        if active { {icon(LdCheck)} }
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

#[component]
pub fn SwitchControl(
    label: String,
    checked: bool,
    onchange: EventHandler<bool>,
    #[props(default = false)] disabled: bool,
) -> Element {
    rsx! {
        label { class: "pam-switch",
            span { class: "pam-switch-label", "{label}" }
            input {
                class: "pam-switch-input",
                r#type: "checkbox",
                checked,
                disabled,
                onchange: move |event| onchange.call(event.checked()),
            }
            span { class: "pam-switch-track" }
        }
    }
}

#[component]
pub fn NumberControl(
    value: u32,
    onchange: EventHandler<u32>,
    #[props(default = 0)] min: u32,
    #[props(default = 99_999)] max: u32,
    #[props(default = false)] disabled: bool,
    #[props(default = String::new())] title: String,
) -> Element {
    rsx! {
        input {
            class: "pam-number-input",
            r#type: "number",
            min,
            max,
            value,
            disabled,
            title: "{title}",
            onchange: move |event| {
                if let Ok(value) = event.value().parse::<u32>() {
                    onchange.call(value.clamp(min, max));
                }
            },
        }
    }
}
