use dioxus::prelude::*;
use toolkit_ui::{Appearance, IconButton, SegmentedControl, SegmentedOption, use_appearance};

const APPEARANCE_OPTIONS: [SegmentedOption; 3] = [
    SegmentedOption {
        value: "system",
        label: "自适应",
    },
    SegmentedOption {
        value: "light",
        label: "浅色",
    },
    SegmentedOption {
        value: "dark",
        label: "深色",
    },
];

#[component]
pub(crate) fn SettingsPanel(open: Signal<bool>) -> Element {
    if !open() {
        return rsx! {};
    }

    let appearance = use_appearance();
    let mut close_backdrop = open;
    let mut close_button = open;

    rsx! {
        div { class: "tk-settings-overlay",
            button {
                class: "tk-settings-backdrop",
                aria_label: "关闭设置",
                onclick: move |_| close_backdrop.set(false),
            }
            section {
                class: "tk-settings-panel",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "tk-settings-title",
                div { class: "tk-settings-header",
                    h2 { id: "tk-settings-title",
                        SettingsGlyph {}
                        "设置"
                    }
                    IconButton {
                        class: "tk-settings-close".to_string(),
                        label: "关闭设置".to_string(),
                        onclick: move |_| close_button.set(false),
                        CloseGlyph {}
                    }
                }
                div { class: "tk-settings-content",
                    div { class: "tk-settings-section tk-settings-section--appearance",
                        span { class: "tk-settings-label", "外观" }
                        SegmentedControl {
                            value: appearance.preference().code().to_string(),
                            options: APPEARANCE_OPTIONS.to_vec(),
                            aria_label: "外观模式".to_string(),
                            onchange: move |value: String| appearance.set(Appearance::from_code(&value)),
                        }
                    }
                    div { class: "tk-settings-section tk-settings-section--info",
                        span { class: "tk-settings-label", "应用信息" }
                        div { class: "tk-settings-info",
                            div {
                                span { "版本" }
                                strong { "0.1.0 Preview" }
                            }
                            div {
                                span { "许可证" }
                                strong { "AGPL-3.0-or-later" }
                            }
                            div {
                                span { "工作区" }
                                strong { "PAM + RTON" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SettingsGlyph() -> Element {
    rsx! {
        svg {
            class: "tk-settings-icon",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            path {
                stroke_linecap: "round",
                stroke_linejoin: "round",
                stroke_width: "2",
                d: "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 0 0 2.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 0 0 1.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 0 0-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 0 0-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 0 0-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 0 0-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 0 0 1.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065Z",
            }
            path {
                stroke_linecap: "round",
                stroke_linejoin: "round",
                stroke_width: "2",
                d: "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
            }
        }
    }
}

#[component]
fn CloseGlyph() -> Element {
    rsx! {
        svg {
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            path {
                stroke_linecap: "round",
                stroke_linejoin: "round",
                stroke_width: "2",
                d: "M6 18 18 6M6 6l12 12",
            }
        }
    }
}
