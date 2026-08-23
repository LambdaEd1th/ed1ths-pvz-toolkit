use dioxus::prelude::*;
use toolkit_ui::{Appearance, IconButton, use_appearance};

use crate::i18n::{language_options, use_i18n, use_locale};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SettingsView {
    #[default]
    General,
    Logs,
}

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
    let i18n = use_i18n();
    let app_version = env!("CARGO_PKG_VERSION");
    let locale_context = use_locale();
    let appearance = use_appearance();
    let mut active_view = use_signal(SettingsView::default);
    let mut log_revision = use_signal(|| 0_u64);
    let mut copied = use_signal(|| false);
    let mut language_open = use_signal(|| false);

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
    let languages = language_options(i18n);
    let active_locale = locale_context.locale();
    let active_language = languages
        .iter()
        .find(|option| option.locale == active_locale)
        .map(|option| option.label.clone())
        .unwrap_or_else(|| active_locale.code().to_string());
    let appearance_options = [
        (Appearance::System, i18n.t("appearance-system")),
        (Appearance::Light, i18n.t("appearance-light")),
        (Appearance::Dark, i18n.t("appearance-dark")),
    ];

    rsx! {
        div { class: "tk-settings-overlay",
            button {
                class: "tk-settings-backdrop",
                aria_label: i18n.t("settings-close"),
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
                        {i18n.t("settings-title")}
                    }
                    IconButton {
                        class: "tk-settings-close".to_string(),
                        label: i18n.t("settings-close"),
                        onclick: move |_| close_button.set(false),
                        CloseGlyph {}
                    }
                }
                div {
                    class: "tk-settings-nav",
                    role: "tablist",
                    aria_label: i18n.t("settings-pages"),
                    button {
                        r#type: "button",
                        class: if active_view_snapshot == SettingsView::General { "is-active" } else { "" },
                        role: "tab",
                        aria_selected: active_view_snapshot == SettingsView::General,
                        onclick: move |_| {
                            active_view.set(SettingsView::General);
                            copied.set(false);
                        },
                        {i18n.t("settings-general")}
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
                        {i18n.t("settings-logs")}
                    }
                }
                div { class: "tk-settings-content",
                    if active_view_snapshot == SettingsView::General {
                        div { class: "tk-settings-section tk-settings-section--language",
                            span { class: "tk-settings-label", {i18n.t("settings-language")} }
                            div { class: if language_open() { "tk-settings-language is-open" } else { "tk-settings-language" },
                                button {
                                    r#type: "button",
                                    class: "tk-settings-language-control",
                                    aria_label: i18n.t("settings-language"),
                                    aria_expanded: language_open(),
                                    onclick: move |_| language_open.set(!language_open()),
                                    span { "{active_language}" }
                                    span { class: "tk-settings-language-caret", aria_hidden: "true", "⌄" }
                                }
                                div { class: "tk-settings-language-menu", role: "menu",
                                    for option in languages {
                                        button {
                                            r#type: "button",
                                            class: if option.locale == active_locale { "is-active" } else { "" },
                                            role: "menuitemradio",
                                            aria_checked: option.locale == active_locale,
                                            onclick: move |_| {
                                                locale_context.set(option.locale);
                                                language_open.set(false);
                                            },
                                            span { "{option.label}" }
                                            if option.locale == active_locale {
                                                b { aria_hidden: "true", "✓" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "tk-settings-section tk-settings-section--appearance",
                            span { class: "tk-settings-label", {i18n.t("settings-appearance")} }
                            div { class: "ui-segmented", role: "group", aria_label: i18n.t("appearance-mode"),
                                for (option, label) in appearance_options {
                                    button {
                                        class: if appearance.preference() == option { "ui-segmented-item is-active" } else { "ui-segmented-item" },
                                        aria_pressed: appearance.preference() == option,
                                        onclick: move |_| appearance.set(option),
                                        "{label}"
                                    }
                                }
                            }
                        }
                        div { class: "tk-settings-section tk-settings-section--info",
                            span { class: "tk-settings-label", {i18n.t("settings-app-info")} }
                            div { class: "tk-settings-info",
                                div {
                                    span { {i18n.t("settings-version")} }
                                    strong { "{app_version}" }
                                }
                                div {
                                    span { {i18n.t("settings-license")} }
                                    strong { "AGPL-3.0-or-later" }
                                }
                            }
                        }
                    } else {
                        div {
                            class: "tk-settings-logs",
                            aria_live: "polite",
                            div { class: "tk-settings-log-summary",
                                strong { {i18n.t("logs-title")} }
                                span { {i18n.t_args("logs-summary", &[("count", log_count.to_string())])} }
                            }
                            textarea {
                                class: "tk-settings-log-viewer",
                                readonly: true,
                                value: "{log_text}",
                                spellcheck: "false",
                                wrap: "soft",
                                aria_label: i18n.t("logs-title"),
                                placeholder: i18n.t("logs-empty"),
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
                                    {i18n.t("logs-refresh")}
                                }
                                button {
                                    r#type: "button",
                                    disabled: !has_logs,
                                    onclick: move |_| {
                                        copy_to_clipboard(&log_text);
                                        copied.set(true);
                                    },
                                    if copied() { {i18n.t("logs-copied")} } else { {i18n.t("logs-copy")} }
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
                                    {i18n.t("logs-clear")}
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
