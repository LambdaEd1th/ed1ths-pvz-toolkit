use std::sync::OnceLock;

use base64::Engine;
use dioxus::prelude::*;

use crate::i18n::{I18n, use_i18n};
use crate::library_registry::{LIBRARIES, LibraryDescriptor};
use crate::navigation::AppRoute;
use crate::shell::BrandLabel;
use crate::tool_registry::{TOOLS, ToolDescriptor};

const HOME_SUNFLOWER_BYTES: &[u8] = include_bytes!("../../assets/home/sunflower-pam.webp");
static HOME_SUNFLOWER_DATA_URL: OnceLock<String> = OnceLock::new();

#[component]
pub(crate) fn HomeDashboard(on_navigate: EventHandler<AppRoute>) -> Element {
    let i18n = use_i18n();
    let library_count = LIBRARIES.len();

    rsx! {
        div { class: "tk-home-scroll",
            div { class: "tk-dashboard",
                section { class: "tk-welcome-card tk-glass-card",
                    div { class: "tk-welcome-copy",
                        div { class: "tk-eyebrow",
                            span { class: "tk-eyebrow-dot" }
                            {i18n.t("home-eyebrow")}
                        }
                        h1 { {i18n.t("home-title")} }
                        p { {i18n.t("home-description")} }
                        div { class: "tk-welcome-actions",
                            button {
                                class: "tk-button tk-button--primary",
                                onclick: move |_| on_navigate.call(AppRoute::Rsb),
                                span { class: "tk-button-icon tk-button-icon--code", aria_hidden: "true", "▤" }
                                {open_tool_label(i18n, "RSB Archive")}
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::RsbPatch),
                                span { class: "tk-button-icon", aria_hidden: "true", "⇄" }
                                {open_tool_label(i18n, "RSB Patch")}
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::Pak),
                                span { class: "tk-button-icon tk-button-icon--code", aria_hidden: "true", "PAK" }
                                {open_tool_label(i18n, "PAK Archive")}
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::Dzip),
                                span { class: "tk-button-icon tk-button-icon--code", aria_hidden: "true", "DZ" }
                                {open_tool_label(i18n, "DZip Archive")}
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::Smf),
                                span { class: "tk-button-icon tk-button-icon--code", aria_hidden: "true", "SMF" }
                                {open_tool_label(i18n, "SMF Container")}
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::Rton),
                                span { class: "tk-button-icon tk-button-icon--code", aria_hidden: "true", "{{ }}" }
                                {open_tool_label(i18n, "RTON Editor")}
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::Pam),
                                span { class: "tk-button-icon", aria_hidden: "true", "▶" }
                                {open_tool_label(i18n, "PAM Viewer")}
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::Wem),
                                span { class: "tk-button-icon", aria_hidden: "true", "♫" }
                                {open_tool_label(i18n, "WEM Audio")}
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::Bnk),
                                span { class: "tk-button-icon tk-button-icon--code", aria_hidden: "true", "BNK" }
                                {i18n.t_args("home-open-experimental-tool", &[("tool", "BNK Archive".to_string())])}
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::Newton),
                                span { class: "tk-button-icon tk-button-icon--code", aria_hidden: "true", "N" }
                                {open_tool_label(i18n, "NEWTON Manifest")}
                            }
                        }
                    }

                    div { class: "tk-welcome-preview", aria_hidden: "true",
                        div { class: "tk-preview-window",
                            div { class: "tk-preview-toolbar",
                                span {}
                                span {}
                                span {}
                                b { "TOOLKIT" }
                            }
                            div { class: "tk-preview-body",
                                div { class: "tk-preview-rail",
                                    i {}
                                    i {}
                                    i {}
                                    i {}
                                }
                                div { class: "tk-preview-canvas",
                                    AnimatedSunflower {}
                                }
                                div { class: "tk-preview-inspector",
                                    span {}
                                    span {}
                                    span {}
                                    span {}
                                }
                            }
                        }
                        div { class: "tk-floating-format tk-floating-format--rsb",
                            span { "▤" }
                            div { strong { "RSB" } small { "Archive" } }
                        }
                        div { class: "tk-floating-format tk-floating-format--rton",
                            span { "{{ }}" }
                            div { strong { "RTON" } small { "Data" } }
                        }
                        div { class: "tk-floating-format tk-floating-format--pam",
                            span { "▶" }
                            div { strong { "PAM" } small { "Animation" } }
                        }
                        div { class: "tk-floating-format tk-floating-format--wem",
                            span { "♫" }
                            div { strong { "WEM" } small { "Audio" } }
                        }
                    }
                }

                section { class: "tk-dashboard-section", id: "tools",
                    div { class: "tk-section-heading",
                        div {
                            span { class: "tk-kicker", {i18n.t("home-quick-access")} }
                            h2 { {i18n.t("home-workspaces-title")} }
                        }
                        p { {i18n.t("home-workspaces-intro")} }
                    }

                    div { class: "tk-workspace-grid",
                        for tool in TOOLS {
                            WorkspaceCard {
                                tool,
                                onclick: move |_| on_navigate.call(tool.route),
                            }
                        }
                    }
                }

                div { class: "tk-lower-grid",
                    section { class: "tk-dashboard-section tk-library-panel tk-glass-card", id: "libraries",
                        div { class: "tk-panel-heading",
                            div {
                                span { class: "tk-kicker", {i18n.t("home-for-developers")} }
                                h2 { {i18n.t("home-libraries-title")} }
                            }
                            button {
                                class: "tk-panel-link",
                                onclick: move |_| on_navigate.call(AppRoute::Libraries),
                                span { "{library_count} crates" }
                                b { aria_hidden: "true", "→" }
                            }
                        }
                        p { class: "tk-panel-intro", {i18n.t("home-libraries-intro")} }
                        div { class: "tk-library-list",
                            for library in LIBRARIES {
                                LibraryRow { library }
                            }
                        }
                    }

                    section { class: "tk-dashboard-section tk-start-panel tk-glass-card", id: "getting-started",
                        span { class: "tk-kicker", {i18n.t("home-getting-started")} }
                        h2 { {i18n.t("home-start-title")} }
                        ol { class: "tk-step-list",
                            li {
                                span { "1" }
                                div { strong { {i18n.t("home-step-one-title")} } p { {i18n.t("home-step-one-body")} } }
                            }
                            li {
                                span { "2" }
                                div { strong { {i18n.t("home-step-two-title")} } p { {i18n.t("home-step-two-body")} } }
                            }
                            li {
                                span { "3" }
                                div { strong { {i18n.t("home-step-three-title")} } p { {i18n.t("home-step-three-body")} } }
                            }
                        }
                        div { class: "tk-growth-note",
                            span { aria_hidden: "true", "+" }
                            p { strong { {i18n.t("home-growth-title")} } {i18n.t("home-growth-body")} }
                        }
                    }
                }

                footer { class: "tk-footer",
                    BrandLabel { compact: true }
                    p { {i18n.t("home-footer")} }
                    span { "Ed1th · 2026" }
                }
            }
        }
    }
}

#[component]
fn AnimatedSunflower() -> Element {
    rsx! {
        div { class: "tk-preview-plant",
            img {
                class: "tk-preview-plant-animation",
                src: home_sunflower_data_url(),
                alt: "",
                loading: "eager",
                decoding: "async",
                draggable: "false",
            }
        }
    }
}

fn home_sunflower_data_url() -> &'static str {
    HOME_SUNFLOWER_DATA_URL.get_or_init(|| {
        let encoded = base64::engine::general_purpose::STANDARD.encode(HOME_SUNFLOWER_BYTES);
        format!("data:image/webp;base64,{encoded}")
    })
}

#[component]
fn WorkspaceCard(tool: ToolDescriptor, onclick: EventHandler<MouseEvent>) -> Element {
    let i18n = use_i18n();
    let description = i18n.t(&format!("tool-{}-description", tool.slug));
    let action = i18n.t(&format!("tool-{}-action", tool.slug));
    rsx! {
        article { class: "tk-workspace-card tk-glass-card tk-workspace-card--{tool.slug}",
            div { class: "tk-workspace-top",
                span { class: "tk-workspace-icon", aria_hidden: "true", "{tool.glyph}" }
                if tool.experimental {
                    span { class: "tk-experimental-chip", {i18n.t("experimental")} }
                } else {
                    span { class: "tk-status-chip" }
                }
            }
            h3 { "{tool.label}" }
            p { "{description}" }
            div { class: "tk-tag-list",
                for tag in tool.tags {
                    span { "{tag}" }
                }
            }
            button {
                class: "tk-launch-button",
                onclick: move |event| onclick.call(event),
                span { "{action}" }
                b { aria_hidden: "true", "→" }
            }
        }
    }
}

#[component]
fn LibraryRow(library: LibraryDescriptor) -> Element {
    let i18n = use_i18n();
    let summary = i18n.t(&format!("library-{}-summary", library.name));
    rsx! {
        a {
            class: "tk-library-row",
            href: library.repository,
            target: "_blank",
            rel: "noopener noreferrer",
            aria_label: i18n.t_args("github-open-library", &[("name", library.name.to_string())]),
            div { class: "tk-crate-mark", aria_hidden: "true", "◇" }
            div { class: "tk-crate-copy",
                div { class: "tk-crate-title",
                    code { "{library.name}" }
                    span { "v{library.version}" }
                }
                p { "{summary}" }
            }
            span { class: "tk-crate-kind", {i18n.t("github-link")} " ↗" }
        }
    }
}

fn open_tool_label(i18n: I18n, tool: &str) -> String {
    i18n.t_args("home-open-tool", &[("tool", tool.to_string())])
}
