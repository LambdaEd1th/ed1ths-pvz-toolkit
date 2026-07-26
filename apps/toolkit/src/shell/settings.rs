use dioxus::prelude::*;
use toolkit_ui::{Appearance, IconButton, SegmentedControl, SegmentedOption, use_appearance};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsView {
    #[default]
    General,
    Logs,
}

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

fn copy_to_clipboard(text: &str) {
    let Ok(text_json) = serde_json::to_string(text) else {
        return;
    };
    document::eval(&format!(
        r#"
        (() => {{
            const text = {text_json};
            const fallbackCopy = () => {{
                const previous = document.activeElement;
                const textarea = document.createElement("textarea");
                textarea.value = text;
                textarea.setAttribute("readonly", "true");
                textarea.style.position = "fixed";
                textarea.style.left = "-10000px";
                textarea.style.top = "0";
                document.body.appendChild(textarea);
                textarea.focus();
                textarea.select();
                try {{
                    document.execCommand("copy");
                }} catch (_error) {{}}
                textarea.remove();
                previous?.focus?.({{ preventScroll: true }});
            }};
            if (navigator.clipboard?.writeText) {{
                navigator.clipboard.writeText(text).catch(fallbackCopy);
            }} else {{
                fallbackCopy();
            }}
        }})();
        "#
    ));
}

#[component]
pub(crate) fn SettingsPanel(open: Signal<bool>) -> Element {
    let appearance = use_appearance();
    let mut active_view = use_signal(SettingsView::default);
    let mut log_revision = use_signal(|| 0_u64);
    let mut copied = use_signal(|| false);

    if !open() {
        return rsx! {};
    }

    let mut close_backdrop = open;
    let mut close_button = open;
    let active_view_snapshot = active_view();
    let _log_revision_snapshot = log_revision();
    let log_lines = if active_view_snapshot == SettingsView::Logs {
        toolkit_ui::application_log_lines()
    } else {
        Vec::new()
    };
    let log_text = log_lines.join("\n");
    let log_count = log_lines.len();
    let has_logs = log_count > 0;

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
                div {
                    class: "tk-settings-nav",
                    role: "tablist",
                    aria_label: "设置页面",
                    button {
                        r#type: "button",
                        class: if active_view_snapshot == SettingsView::General { "is-active" } else { "" },
                        role: "tab",
                        aria_selected: active_view_snapshot == SettingsView::General,
                        onclick: move |_| {
                            active_view.set(SettingsView::General);
                            copied.set(false);
                        },
                        "常规"
                    }
                    button {
                        r#type: "button",
                        class: if active_view_snapshot == SettingsView::Logs { "is-active" } else { "" },
                        role: "tab",
                        aria_selected: active_view_snapshot == SettingsView::Logs,
                        onclick: move |_| {
                            active_view.set(SettingsView::Logs);
                            copied.set(false);
                        },
                        "日志"
                    }
                }
                div { class: "tk-settings-content",
                    if active_view_snapshot == SettingsView::General {
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
                    } else {
                        div {
                            class: "tk-settings-logs",
                            aria_live: "polite",
                            div { class: "tk-settings-log-summary",
                                strong { "应用日志" }
                                span { "PAM 与 RTON · {log_count} 条" }
                            }
                            textarea {
                                class: "tk-settings-log-viewer",
                                readonly: true,
                                value: "{log_text}",
                                spellcheck: "false",
                                wrap: "soft",
                                aria_label: "应用日志",
                                placeholder: "暂无日志",
                                onfocus: move |_| {
                                    copied.set(false);
                                    log_revision.set(log_revision().wrapping_add(1));
                                },
                            }
                            div { class: "tk-settings-log-actions",
                                button {
                                    r#type: "button",
                                    onclick: move |_| {
                                        copied.set(false);
                                        log_revision.set(log_revision().wrapping_add(1));
                                    },
                                    "刷新"
                                }
                                button {
                                    r#type: "button",
                                    disabled: !has_logs,
                                    onclick: move |_| {
                                        copy_to_clipboard(&log_text);
                                        copied.set(true);
                                    },
                                    if copied() { "已复制" } else { "复制" }
                                }
                                button {
                                    r#type: "button",
                                    class: "is-danger",
                                    disabled: !has_logs,
                                    onclick: move |_| {
                                        toolkit_ui::clear_application_logs();
                                        copied.set(false);
                                        log_revision.set(log_revision().wrapping_add(1));
                                    },
                                    "清空"
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
