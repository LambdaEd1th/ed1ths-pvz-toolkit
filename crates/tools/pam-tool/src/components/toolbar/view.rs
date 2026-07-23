use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{
    LdCheck, LdChevronDown, LdGithub, LdInfo, LdLanguages, LdRotateCcw, LdScrollText,
};

use crate::actions::{reset_view, set_export_dimension, set_export_scale, set_locale};
use crate::i18n::tr;
use crate::state::{AppContext, Locale};

use super::super::primitives::{NumberControl, SelectControl, SelectOption, icon};

const AUTHOR_NAME: &str = "LambdaEd1th";
const AUTHOR_URL: &str = "https://space.bilibili.com/8217621";
const GITHUB_URL: &str = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit";

#[component]
pub(super) fn ViewGroup() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    rsx! {
        button { r#type: "button", class: "pam-button", onclick: move |_| reset_view(context),
            {icon(LdRotateCcw)} span { {tr(locale, "reset_view")} }
        }
    }
}

#[component]
pub(super) fn SizeGroup() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let tab = context.active_tab_snapshot();
    let disabled = tab.is_none();
    let size = tab.as_ref().map(|tab| tab.export_size).unwrap_or([0, 0]);
    let scale = tab
        .as_ref()
        .and_then(|tab| tab.export_scale)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "custom".into());
    let scales = [
        ("custom", tr(locale, "custom")),
        ("1", "1x"),
        ("2", "2x"),
        ("3", "3x"),
        ("4", "4x"),
    ]
    .into_iter()
    .map(|(value, label)| SelectOption::new(value, label))
    .collect();
    rsx! {
        span { class: "pam-field-label",
            span { {tr(locale, "export_size")} }
            NumberControl { value: size[0], min: 1, max: 99_999, disabled, onchange: move |value| set_export_dimension(context, 0, value) }
            span { class: "pam-range-separator", "x" }
            NumberControl { value: size[1], min: 1, max: 99_999, disabled, onchange: move |value| set_export_dimension(context, 1, value) }
            SelectControl {
                value: scale, options: scales, compact: true, disabled,
                onchange: move |value: String| set_export_scale(context, value.parse::<u32>().ok()),
            }
        }
    }
}

#[component]
pub(super) fn PreferenceGroup(on_open_logs: EventHandler<()>) -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let mut language_open = use_signal(|| false);
    let language_open_snapshot = *language_open.read();
    let language_options = [(Locale::ZhCn, "中文"), (Locale::En, "English")];
    let current_language = language_options
        .iter()
        .find(|(option, _)| *option == locale)
        .map(|(_, label)| *label)
        .unwrap_or(locale.code());

    rsx! {
        section { class: "pam-settings-section",
            div { class: "pam-settings-section-heading",
                span { class: "pam-settings-section-icon", {icon(LdLanguages)} }
                span { {tr(locale, "language")} }
            }
            div { class: if language_open_snapshot { "pam-settings-language open" } else { "pam-settings-language" },
                button {
                    r#type: "button",
                    class: "pam-settings-language-control",
                    aria_label: tr(locale, "language"),
                    aria_haspopup: "listbox",
                    aria_expanded: language_open_snapshot,
                    onclick: move |_| language_open.set(!language_open_snapshot),
                    span { "{current_language}" }
                    span { class: "pam-settings-language-caret", {icon(LdChevronDown)} }
                }
                div {
                    class: "pam-settings-language-menu",
                    role: "listbox",
                    aria_label: tr(locale, "language"),
                    aria_hidden: !language_open_snapshot,
                    for (option_locale, option_label) in language_options {
                        {
                            let active = option_locale == locale;
                            rsx! {
                                button {
                                    key: "{option_locale:?}",
                                    r#type: "button",
                                    class: if active { "active" } else { "" },
                                    role: "option",
                                    tabindex: if language_open_snapshot { "0" } else { "-1" },
                                    aria_selected: active,
                                    onclick: move |_| {
                                        language_open.set(false);
                                        set_locale(context, option_locale);
                                    },
                                    span { "{option_label}" }
                                    if active {
                                        span { class: "pam-settings-language-check", {icon(LdCheck)} }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        section { class: "pam-settings-section pam-settings-logs-section",
            div { class: "pam-settings-section-heading",
                span { class: "pam-settings-section-icon", {icon(LdScrollText)} }
                span { {tr(locale, "settings_logs")} }
            }
            button {
                r#type: "button",
                class: "pam-settings-action-button",
                onclick: move |_| on_open_logs.call(()),
                {icon(LdScrollText)}
                span { {tr(locale, "settings_logs_open")} }
            }
        }

        section { class: "pam-settings-section pam-settings-about-section",
            div { class: "pam-settings-section-heading",
                span { class: "pam-settings-section-icon", {icon(LdInfo)} }
                span { {tr(locale, "settings_about")} }
            }
            dl { class: "pam-settings-about-list",
                div { class: "pam-settings-about-item",
                    dt { {tr(locale, "about_version")} }
                    dd { {env!("CARGO_PKG_VERSION")} }
                }
                div { class: "pam-settings-about-item",
                    dt { {tr(locale, "about_license")} }
                    dd { {env!("CARGO_PKG_LICENSE")} }
                }
                div { class: "pam-settings-about-item",
                    dt { {tr(locale, "about_author")} }
                    dd {
                        a {
                            href: AUTHOR_URL,
                            target: "_blank",
                            rel: "noopener noreferrer",
                            "{AUTHOR_NAME}"
                        }
                    }
                }
            }
            a {
                class: "pam-settings-github-link",
                href: GITHUB_URL,
                target: "_blank",
                rel: "noopener noreferrer",
                title: tr(locale, "about_github"),
                {icon(LdGithub)}
                span { {tr(locale, "about_github")} }
            }
        }
    }
}
