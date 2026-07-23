use dioxus::prelude::*;

use crate::navigation::AppRoute;
use crate::shell::BrandMark;
use crate::tool_registry::{TOOLS, ToolDescriptor};

#[component]
pub(crate) fn HomeDashboard(on_navigate: EventHandler<AppRoute>) -> Element {
    rsx! {
        div { class: "tk-home-scroll",
            div { class: "tk-dashboard",
                section { class: "tk-welcome-card tk-glass-card",
                    div { class: "tk-welcome-copy",
                        div { class: "tk-eyebrow",
                            span { class: "tk-eyebrow-dot" }
                            "Resource workspace"
                        }
                        h1 { "让 PvZ 资源处理回到一个工作区。" }
                        p {
                            "查看 PAM 动画、编辑 RTON 数据，并按需使用独立的 Rust 格式库。"
                            "工具保持专注，界面和工作流保持一致。"
                        }
                        div { class: "tk-welcome-actions",
                            button {
                                class: "tk-button tk-button--primary",
                                onclick: move |_| on_navigate.call(AppRoute::Pam),
                                span { class: "tk-button-icon", aria_hidden: "true", "▶" }
                                "打开 PAM Viewer"
                            }
                            button {
                                class: "tk-button tk-button--secondary",
                                onclick: move |_| on_navigate.call(AppRoute::Rton),
                                span { class: "tk-button-icon tk-button-icon--code", aria_hidden: "true", "{{ }}" }
                                "打开 RTON Editor"
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
                                    div { class: "tk-preview-sun" }
                                    div { class: "tk-preview-leaf tk-preview-leaf--one" }
                                    div { class: "tk-preview-leaf tk-preview-leaf--two" }
                                    div { class: "tk-preview-leaf tk-preview-leaf--three" }
                                    div { class: "tk-preview-ground" }
                                }
                                div { class: "tk-preview-inspector",
                                    span {}
                                    span {}
                                    span {}
                                    span {}
                                }
                            }
                        }
                        div { class: "tk-floating-format tk-floating-format--pam",
                            span { "▶" }
                            div { strong { "PAM" } small { "Animation" } }
                        }
                        div { class: "tk-floating-format tk-floating-format--rton",
                            span { "{{ }}" }
                            div { strong { "RTON" } small { "Data" } }
                        }
                    }
                }

                section { class: "tk-dashboard-section", id: "tools",
                    div { class: "tk-section-heading",
                        div {
                            span { class: "tk-kicker", "Quick access" }
                            h2 { "工作区" }
                        }
                        p { "每个工具保留自己的专业工作流，并共享统一的导航与视觉语言。" }
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
                                span { class: "tk-kicker", "For developers" }
                                h2 { "独立格式库" }
                            }
                            span { class: "tk-panel-badge", "2 crates" }
                        }
                        p { class: "tk-panel-intro",
                            "应用只组合所需的格式库；其他项目可以直接依赖单个 crate，无需引入 GUI 或聚合 SDK。"
                        }
                        div { class: "tk-library-list",
                            LibraryRow {
                                crate_name: "pam-codec",
                                version: "0.1.2",
                                description: "Typed PAM binary decoding and encoding",
                            }
                            LibraryRow {
                                crate_name: "serde_rton",
                                version: "0.4.0",
                                description: "Serde reader, writer, Value and crypto helpers",
                            }
                        }
                    }

                    section { class: "tk-dashboard-section tk-start-panel tk-glass-card", id: "about",
                        span { class: "tk-kicker", "Getting started" }
                        h2 { "从文件开始" }
                        ol { class: "tk-step-list",
                            li {
                                span { "1" }
                                div { strong { "选择工作区" } p { "PAM 用于动画资源，RTON 用于结构化数据。" } }
                            }
                            li {
                                span { "2" }
                                div { strong { "打开或拖入文件" } p { "工具会在本地读取资源，不需要 CLI。" } }
                            }
                            li {
                                span { "3" }
                                div { strong { "检查、编辑并导出" } p { "保留原始格式，也可转换为开放格式。" } }
                            }
                        }
                        div { class: "tk-growth-note",
                            span { aria_hidden: "true", "+" }
                            p { strong { "为更多格式预留" } "后续工具会沿用相同外壳，并继续保持库的独立发布。" }
                        }
                    }
                }

                footer { class: "tk-footer",
                    BrandMark { compact: true }
                    p { "One app shell · Focused format libraries" }
                    span { "Ed1th · 2026" }
                }
            }
        }
    }
}

#[component]
fn WorkspaceCard(tool: ToolDescriptor, onclick: EventHandler<MouseEvent>) -> Element {
    rsx! {
        article { class: "tk-workspace-card tk-glass-card tk-workspace-card--{tool.slug}",
            div { class: "tk-workspace-top",
                span { class: "tk-workspace-icon", aria_hidden: "true", "{tool.glyph}" }
                span { class: "tk-status-chip" }
            }
            h3 { "{tool.label}" }
            p { "{tool.description}" }
            div { class: "tk-tag-list",
                for tag in tool.tags {
                    span { "{tag}" }
                }
            }
            button {
                class: "tk-launch-button",
                onclick: move |event| onclick.call(event),
                span { "{tool.action}" }
                b { aria_hidden: "true", "→" }
            }
        }
    }
}

#[component]
fn LibraryRow(
    crate_name: &'static str,
    version: &'static str,
    description: &'static str,
) -> Element {
    rsx! {
        article { class: "tk-library-row",
            div { class: "tk-crate-mark", aria_hidden: "true", "◇" }
            div { class: "tk-crate-copy",
                div { class: "tk-crate-title",
                    code { "{crate_name}" }
                    span { "v{version}" }
                }
                p { "{description}" }
            }
            span { class: "tk-crate-kind", "Public crate" }
        }
    }
}
