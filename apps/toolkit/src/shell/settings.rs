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
                    div { class: "tk-settings-section",
                        span { class: "tk-settings-label", "外观" }
                        SegmentedControl {
                            value: appearance.preference().code().to_string(),
                            options: APPEARANCE_OPTIONS.to_vec(),
                            aria_label: "外观模式".to_string(),
                            onchange: move |value: String| appearance.set(Appearance::from_code(&value)),
                        }
                    }
                    div { class: "tk-settings-section",
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
            circle { cx: "12", cy: "12", r: "3", stroke_width: "2" }
            path {
                stroke_linecap: "round",
                stroke_linejoin: "round",
                stroke_width: "2",
                d: "M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06-2.83 2.83-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21h-4v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06-2.83-2.83.06-.06A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1.51-1H3v-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06 2.83-2.83.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3h4v.09A1.65 1.65 0 0 0 15 4.6a1.65 1.65 0 0 0 1.82-.33l.06-.06 2.83 2.83-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21v4h-.09A1.65 1.65 0 0 0 19.4 15Z",
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
