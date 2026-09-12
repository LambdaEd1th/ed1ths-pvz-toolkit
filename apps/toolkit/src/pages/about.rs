use dioxus::prelude::*;

use crate::i18n::use_i18n;

const REPOSITORY_URL: &str = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit";
const TWINNING_URL: &str = "https://github.com/twinstar6980/Twinning";
const MOESEKAI_URL: &str = "https://github.com/StarMoe-org/Moesekai";
const MAINTAINER_URL: &str = "https://space.bilibili.com/8217621";
const MAINTAINER_AVATAR_URL: &str = "https://github.com/LambdaEd1th.png?size=192";
const TECH_STACK: [&str; 6] = [
    "Rust",
    "Dioxus 0.7.10",
    "WGPU 30",
    "WebAssembly",
    "Serde",
    "WebGPU",
];

#[component]
pub(crate) fn AboutPage() -> Element {
    let i18n = use_i18n();
    let mut show_maintainer_avatar = use_signal(|| true);

    rsx! {
        div { class: "tk-about-scroll",
            div { class: "tk-about-page",
                header { class: "tk-about-hero",
                    span { class: "tk-kicker", {i18n.t("about-kicker")} }
                    h1 { {i18n.t("about-title")} }
                    p { {i18n.t("about-intro")} }
                }

                div { class: "tk-about-grid",
                    article { class: "tk-about-card tk-about-card--mission tk-about-span-2",
                        div { class: "tk-about-card-heading",
                            div {
                                h2 { {i18n.t("about-mission-title")} }
                                span { {i18n.t("about-mission-kicker")} }
                            }
                            span { class: "tk-about-version", "v{env!(\"CARGO_PKG_VERSION\")}" }
                        }
                        div { class: "tk-about-divider" }
                        p { {i18n.t("about-mission-body-one")} }
                        p { {i18n.t("about-mission-body-two")} }
                        div { class: "tk-about-principles",
                            span { {i18n.t("about-principle-local")} }
                            span { {i18n.t("about-principle-libraries")} }
                            span { {i18n.t("about-principle-workspace")} }
                        }
                    }

                    article { class: "tk-about-card tk-about-card--maintainer",
                        span { class: "tk-about-card-label", {i18n.t("about-maintainer")} }
                        a {
                            class: "tk-about-maintainer-mark",
                            href: MAINTAINER_URL,
                            target: "_blank",
                            rel: "noopener noreferrer",
                            aria_label: i18n.t("about-maintainer-link"),
                            span { class: "tk-about-maintainer-fallback", aria_hidden: "true", "E" }
                            if show_maintainer_avatar() {
                                img {
                                    src: MAINTAINER_AVATAR_URL,
                                    alt: "",
                                    onerror: move |_| show_maintainer_avatar.set(false),
                                }
                            }
                        }
                        h2 { "LambdaEd1th" }
                        p { {i18n.t("about-maintainer-role")} }
                        small { {i18n.t("about-maintainer-tagline")} }
                    }

                    article { class: "tk-about-card tk-about-card--stack",
                        span { class: "tk-about-card-label", {i18n.t("about-tech-stack")} }
                        h2 { {i18n.t("about-tech-title")} }
                        div { class: "tk-about-tech-list",
                            for technology in TECH_STACK {
                                span { "{technology}" }
                            }
                        }
                        p { {i18n.t("about-tech-body")} }
                    }

                    article { class: "tk-about-card tk-about-card--credits tk-about-span-2",
                        h2 { {i18n.t("about-credits-title")} }
                        div { class: "tk-about-credit-grid",
                            section {
                                h3 { {i18n.t("about-research-title")} }
                                ul {
                                    li {
                                        span { aria_hidden: "true" }
                                        p {
                                            strong { "PopCap / Plants vs. Zombies" }
                                            {i18n.t("about-research-body")}
                                        }
                                    }
                                    li {
                                        span { aria_hidden: "true" }
                                        p {
                                            a {
                                                class: "tk-about-credit-link",
                                                href: TWINNING_URL,
                                                target: "_blank",
                                                rel: "noopener noreferrer",
                                                strong {
                                                    "Twinning"
                                                    b { aria_hidden: "true", "↗" }
                                                }
                                            }
                                            {i18n.t("about-twinning-body")}
                                        }
                                    }
                                    li {
                                        span { aria_hidden: "true" }
                                        p {
                                            a {
                                                class: "tk-about-credit-link",
                                                href: MOESEKAI_URL,
                                                target: "_blank",
                                                rel: "noopener noreferrer",
                                                strong {
                                                    "Moesekai"
                                                    b { aria_hidden: "true", "↗" }
                                                }
                                            }
                                            {i18n.t("about-moesekai-body")}
                                        }
                                    }
                                }
                            }
                            section {
                                h3 { {i18n.t("about-license-title")} }
                                p {
                                    {i18n.t("about-license-prefix")}
                                    strong { "AGPL-3.0-or-later" }
                                    {i18n.t("about-license-suffix")}
                                }
                                a {
                                    class: "tk-about-repository",
                                    href: REPOSITORY_URL,
                                    target: "_blank",
                                    rel: "noreferrer",
                                    span { {i18n.t("github-repository")} }
                                    b { aria_hidden: "true", "↗" }
                                }
                            }
                        }
                    }

                    article { class: "tk-about-card tk-about-card--formats tk-about-span-3",
                        div { class: "tk-about-card-heading",
                            div {
                                span { class: "tk-about-card-label", {i18n.t("about-workspaces-label")} }
                                h2 { {i18n.t("about-workspaces-title")} }
                            }
                            span { class: "tk-about-count", {i18n.t_args("about-tools-count", &[("count", "14".to_string())])} }
                        }
                        div { class: "tk-about-format-grid",
                            section { class: "tk-about-format tk-about-format--rsb",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "RSB" }
                                    div {
                                        h3 { "RSB Archive" }
                                        code { "rsb-archive" }
                                    }
                                }
                                p { {i18n.t("tool-rsb-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--rsb-patch",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "⇄" }
                                    div {
                                        h3 { "RSB Patch" }
                                        code { "rsb-patch" }
                                    }
                                }
                                p { {i18n.t("tool-rsb-patch-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--pak",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "PAK" }
                                    div {
                                        h3 { "PAK Archive" }
                                        code { "pak-archive" }
                                    }
                                }
                                p { {i18n.t("tool-pak-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--dzip",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "◇" }
                                    div {
                                        h3 { "DZip Archive" }
                                        code { "dzip-archive" }
                                    }
                                }
                                p { {i18n.t("tool-dzip-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--smf",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "◇" }
                                    div {
                                        h3 { "SMF Container" }
                                        code { "smf-container" }
                                    }
                                }
                                p { {i18n.t("tool-smf-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--compiled-text",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "≋" }
                                    div {
                                        h3 { "Compiled Text" }
                                        code { "compiled-text" }
                                    }
                                }
                                p { {i18n.t("tool-compiled-text-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--crypt-data",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "◈" }
                                    div {
                                        h3 { "Crypt-Data" }
                                        code { "crypt-data" }
                                    }
                                }
                                p { {i18n.t("tool-crypt-data-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--rton",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "RTON" }
                                    div {
                                        h3 { "RTON Editor" }
                                        code { "serde-rton" }
                                    }
                                }
                                p { {i18n.t("tool-rton-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--pam",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "PAM" }
                                    div {
                                        h3 { "PAM Editor" }
                                        code { "pam-codec" }
                                    }
                                }
                                p { {i18n.t("tool-pam-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--particle",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "✦" }
                                    div {
                                        h3 { "Particle Editor" }
                                        code { "particle-codec" }
                                    }
                                }
                                p { {i18n.t("tool-particle-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--reanim",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "◫" }
                                    div {
                                        h3 { "REANIM Editor" }
                                        code { "reanim-codec" }
                                    }
                                }
                                p { {i18n.t("tool-reanim-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--wem",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "WEM" }
                                    div {
                                        h3 { "WEM Audio" }
                                        code { "wem-audio" }
                                    }
                                }
                                p { {i18n.t("tool-wem-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--bnk",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "BNK" }
                                    div {
                                        h3 { "BNK Archive" }
                                        code { "bnk-archive" }
                                    }
                                }
                                p { {i18n.t("tool-bnk-description")} }
                            }
                            section { class: "tk-about-format tk-about-format--newton",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "N" }
                                    div {
                                        h3 { "NEWTON Manifest" }
                                        code { "newton-manifest" }
                                    }
                                }
                                p { {i18n.t("tool-newton-description")} }
                            }
                        }
                    }

                    article { class: "tk-about-card tk-about-card--notice tk-about-span-3",
                        div {
                            span { class: "tk-about-card-label", {i18n.t("about-unofficial-label")} }
                            h2 { {i18n.t("about-notice-title")} }
                        }
                        p { {i18n.t("about-notice-body")} }
                    }
                }

                footer { class: "tk-about-footer",
                    span { "Ed1th's PvZ Toolkit" }
                    p { {i18n.t("about-footer")} }
                    b { "2026" }
                }
            }
        }
    }
}
