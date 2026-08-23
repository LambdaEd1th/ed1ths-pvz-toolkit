use dioxus::prelude::*;

use crate::i18n::use_i18n;
use crate::library_registry::{LIBRARIES, LibraryDescriptor};

const TOOLKIT_REPOSITORY: &str = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit";

#[component]
pub(crate) fn LibrariesPage() -> Element {
    let i18n = use_i18n();
    let library_count = LIBRARIES.len();

    rsx! {
        div { class: "tk-libraries-scroll",
            div { class: "tk-libraries-page",
                header { class: "tk-libraries-hero tk-glass-card",
                    div { class: "tk-libraries-hero-copy",
                        span { class: "tk-kicker", {i18n.t("libraries-kicker")} }
                        h1 { {i18n.t("libraries-title")} }
                        p { {i18n.t("libraries-intro")} }
                        div { class: "tk-libraries-hero-actions",
                            a {
                                class: "tk-button tk-button--primary",
                                href: TOOLKIT_REPOSITORY,
                                target: "_blank",
                                rel: "noopener noreferrer",
                                {i18n.t("libraries-toolkit-github")}
                                span { aria_hidden: "true", "↗" }
                            }
                        }
                    }
                    div { class: "tk-libraries-summary", aria_label: i18n.t("libraries-summary-label"),
                        div {
                            strong { "{library_count}" }
                            span { {i18n.t("libraries-independent-repositories")} }
                        }
                        div {
                            strong { "Rust" }
                            span { {i18n.t("libraries-typed-apis")} }
                        }
                        div {
                            strong { "No SDK" }
                            span { {i18n.t("libraries-depend-only")} }
                        }
                    }
                }

                section { class: "tk-libraries-section",
                    div { class: "tk-libraries-heading",
                        div {
                            span { class: "tk-kicker", {i18n.t("libraries-format-crates")} }
                            h2 { {i18n.t("libraries-section-title")} }
                        }
                        span { class: "tk-panel-badge", {i18n.t_args("libraries-repository-count", &[("count", library_count.to_string())])} }
                    }
                    p { {i18n.t("libraries-section-intro")} }

                    div { class: "tk-libraries-grid",
                        for library in LIBRARIES {
                            LibraryCard { library }
                        }
                    }
                }

                section { class: "tk-libraries-principles tk-glass-card",
                    div {
                        span { class: "tk-libraries-principle-mark", aria_hidden: "true", "◇" }
                        h2 { {i18n.t("libraries-principle-release-title")} }
                        p { {i18n.t("libraries-principle-release-body")} }
                    }
                    div {
                        span { class: "tk-libraries-principle-mark", aria_hidden: "true", "◇" }
                        h2 { {i18n.t("libraries-principle-compose-title")} }
                        p { {i18n.t("libraries-principle-compose-body")} }
                    }
                    div {
                        span { class: "tk-libraries-principle-mark", aria_hidden: "true", "◇" }
                        h2 { {i18n.t("libraries-principle-gui-title")} }
                        p { {i18n.t("libraries-principle-gui-body")} }
                    }
                }

                footer { class: "tk-libraries-footer",
                    span { "Ed1th's PvZ Toolkit" }
                    p { {i18n.t("libraries-footer")} }
                    b { "AGPL-3.0-or-later" }
                }
            }
        }
    }
}

#[component]
fn LibraryCard(library: LibraryDescriptor) -> Element {
    let i18n = use_i18n();
    let overview = i18n.t(&format!("library-{}-overview", library.name));
    rsx! {
        a {
            class: "tk-library-card tk-library-card--{library.family}",
            href: library.repository,
            target: "_blank",
            rel: "noopener noreferrer",
            aria_label: i18n.t_args("github-open-library", &[("name", library.name.to_string())]),
            div { class: "tk-library-card-top",
                span { class: "tk-library-card-mark", aria_hidden: "true", "◇" }
                span { class: "tk-library-card-version", "v{library.version}" }
            }
            div { class: "tk-library-card-title",
                code { "{library.name}" }
                span { aria_hidden: "true", "↗" }
            }
            p { "{overview}" }
            div { class: "tk-library-card-tags",
                for tag in library.tags {
                    span { "{tag}" }
                }
            }
        }
    }
}
