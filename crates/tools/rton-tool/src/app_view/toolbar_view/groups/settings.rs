use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{
    LdCheck, LdChevronDown, LdGithub, LdInfo, LdLanguages, LdScrollText, LdSettings, LdX,
};

use crate::components::{LogViewerDialog, lucide_icon};
use crate::domain::{Status, Tone};
use crate::i18n::{I18n, LanguageOption, Locale};
use crate::platform;

const AUTHOR_NAME: &str = "LambdaEd1th";
const AUTHOR_URL: &str = "https://space.bilibili.com/8217621";
const GITHUB_URL: &str = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit";

#[component]
pub(crate) fn SettingsDialog(
    i18n: I18n,
    closing: bool,
    locale_snapshot: Locale,
    language_options: Vec<LanguageOption>,
    mut locale: Signal<Locale>,
    mut status: Signal<Status>,
    on_close: EventHandler<()>,
) -> Element {
    let mut language_open = use_signal(|| false);
    let mut logs_open = use_signal(|| false);
    let language_open_snapshot = *language_open.read();
    let title = i18n.t("settings-title");
    let close_label = i18n.t("about-close");
    let current_language_label = language_options
        .iter()
        .find(|option| option.locale == locale_snapshot)
        .map(|option| option.label.clone())
        .unwrap_or_else(|| locale_snapshot.code().to_string());

    rsx! {
        div {
            class: if closing { "rton-settings-backdrop closing" } else { "rton-settings-backdrop" },
            tabindex: "-1",
            onmounted: move |event| {
                spawn(async move {
                    let _ = event.set_focus(true).await;
                });
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    on_close.call(());
                }
            },
            onclick: move |_| on_close.call(()),
            div {
                class: "rton-settings-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{title}",
                onclick: move |event| event.stop_propagation(),
                header { class: "rton-settings-header",
                    div { class: "rton-settings-title-row",
                        span { class: "rton-settings-title-icon", {lucide_icon(LdSettings)} }
                        h2 { "{title}" }
                    }
                    button {
                        r#type: "button",
                        class: "rton-settings-close",
                        title: "{close_label}",
                        aria_label: "{close_label}",
                        onclick: move |_| on_close.call(()),
                        {lucide_icon(LdX)}
                    }
                }

                div { class: "rton-settings-content",
                    section { class: "rton-settings-section",
                        div { class: "rton-settings-section-heading",
                            span { class: "rton-settings-section-icon", {lucide_icon(LdLanguages)} }
                            span { {i18n.t("settings-language")} }
                        }
                        div { class: if language_open_snapshot { "rton-settings-language open" } else { "rton-settings-language" },
                            button {
                                r#type: "button",
                                class: "rton-settings-language-control",
                                aria_label: i18n.t("settings-language"),
                                aria_haspopup: "listbox",
                                aria_expanded: language_open_snapshot,
                                onclick: move |_| language_open.set(!language_open_snapshot),
                                span { "{current_language_label}" }
                                span { class: "rton-settings-language-caret", {lucide_icon(LdChevronDown)} }
                            }
                            div {
                                class: "rton-settings-language-menu",
                                role: "listbox",
                                aria_label: i18n.t("settings-language"),
                                aria_hidden: !language_open_snapshot,
                                for option in language_options {
                                    {
                                        let option_locale = option.locale;
                                        let active = option_locale == locale_snapshot;
                                        rsx! {
                                            button {
                                                key: "{option_locale.code()}",
                                                r#type: "button",
                                                class: if active { "active" } else { "" },
                                                role: "option",
                                                tabindex: if language_open_snapshot { "0" } else { "-1" },
                                                aria_selected: active,
                                                onclick: move |_| {
                                                    language_open.set(false);
                                                    let _ = platform::save_locale_preference(option_locale.code());
                                                    locale.set(option_locale);
                                                    status.set(Status::new(
                                                        I18n::new(option_locale).t("status-ready"),
                                                        Tone::Ok,
                                                    ));
                                                },
                                                span { "{option.label}" }
                                                if active {
                                                    span { class: "rton-settings-language-check", {lucide_icon(LdCheck)} }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    section { class: "rton-settings-section rton-settings-logs-section",
                        div { class: "rton-settings-section-heading",
                            span { class: "rton-settings-section-icon", {lucide_icon(LdScrollText)} }
                            span { {i18n.t("settings-logs")} }
                        }
                        button {
                            r#type: "button",
                            class: "rton-settings-action-button",
                            onclick: move |_| logs_open.set(true),
                            {lucide_icon(LdScrollText)}
                            span { {i18n.t("settings-logs-open")} }
                        }
                    }

                    section { class: "rton-settings-section rton-settings-about-section",
                        div { class: "rton-settings-section-heading",
                            span { class: "rton-settings-section-icon", {lucide_icon(LdInfo)} }
                            span { {i18n.t("settings-about")} }
                        }
                        dl { class: "rton-settings-about-list",
                            SettingsAboutItem {
                                label: i18n.t("about-version"),
                                value: env!("CARGO_PKG_VERSION").to_string()
                            }
                            SettingsAboutItem {
                                label: i18n.t("about-license"),
                                value: env!("CARGO_PKG_LICENSE").to_string()
                            }
                            div { class: "rton-settings-about-item",
                                dt { {i18n.t("about-author")} }
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
                            class: "rton-settings-github-link",
                            href: GITHUB_URL,
                            target: "_blank",
                            rel: "noopener noreferrer",
                            title: i18n.t("about-github"),
                            {lucide_icon(LdGithub)}
                            span { {i18n.t("about-github")} }
                        }
                    }
                }
            }
        }
        if *logs_open.read() {
            LogViewerDialog {
                i18n,
                status,
                on_close: move |_| logs_open.set(false),
            }
        }
    }
}

#[component]
fn SettingsAboutItem(label: String, value: String) -> Element {
    rsx! {
        div { class: "rton-settings-about-item",
            dt { "{label}" }
            dd { "{value}" }
        }
    }
}
