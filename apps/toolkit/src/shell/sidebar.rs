use dioxus::prelude::*;

use crate::navigation::{AppRoute, navigate};
use crate::tool_registry::TOOLS;

#[component]
pub(crate) fn Sidebar(route: Signal<AppRoute>, open: Signal<bool>, compact: bool) -> Element {
    let active_route = route();
    let sidebar_class = if open() {
        "tk-sidebar tk-island tk-sidebar--open"
    } else {
        "tk-sidebar tk-island"
    };
    let mut close_sidebar = open;

    rsx! {
        button {
            class: if open() { "tk-sidebar-backdrop tk-sidebar-backdrop--visible" } else { "tk-sidebar-backdrop" },
            aria_label: "关闭导航",
            onclick: move |_| close_sidebar.set(false),
        }

        aside { class: "{sidebar_class}",
            nav { class: "tk-sidebar-nav", aria_label: "Toolkit 导航",
                div { class: "tk-nav-group",
                    span { class: "tk-nav-label", "Overview" }
                    NavItem {
                        active: active_route == AppRoute::Home,
                        glyph: "⌂",
                        label: "Home",
                        onclick: move |_| navigate(route, open, compact, AppRoute::Home),
                    }
                }

                div { class: "tk-nav-group",
                    span { class: "tk-nav-label", "Workspaces" }
                    for tool in TOOLS {
                        NavItem {
                            active: active_route == tool.route,
                            glyph: tool.glyph,
                            label: tool.label,
                            kind: Some(tool.slug),
                            experimental: tool.experimental,
                            onclick: move |_| navigate(route, open, compact, tool.route),
                        }
                    }
                }

                div { class: "tk-nav-group",
                    span { class: "tk-nav-label", "Developer" }
                    NavAnchor {
                        href: "#libraries",
                        glyph: "◇",
                        label: "Libraries",
                        onclick: move |_| navigate(route, open, compact, AppRoute::Home),
                    }
                    NavItem {
                        active: active_route == AppRoute::About,
                        glyph: "i",
                        label: "About",
                        onclick: move |_| navigate(route, open, compact, AppRoute::About),
                    }
                }
            }

            div { class: "tk-sidebar-foot",
                div { class: "tk-sidebar-version",
                    span { "Toolkit" }
                    strong { "0.1.0 preview" }
                }
                span { class: "tk-sidebar-online", title: "Application ready" }
            }
        }
    }
}

#[component]
fn NavItem(
    active: bool,
    glyph: &'static str,
    label: &'static str,
    #[props(default)] kind: Option<&'static str>,
    #[props(default)] experimental: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let glyph_class = kind
        .map(|kind| format!("tk-nav-glyph tk-nav-glyph--{kind}"))
        .unwrap_or_else(|| "tk-nav-glyph".to_string());
    rsx! {
        button {
            class: if active { "tk-nav-item tk-nav-item--active" } else { "tk-nav-item" },
            aria_current: if active { "page" } else { "false" },
            onclick: move |event| onclick.call(event),
            span { class: "{glyph_class}", aria_hidden: "true", "{glyph}" }
            span { "{label}" }
            if experimental {
                span { class: "tk-nav-experimental", "EXP" }
            }
        }
    }
}

#[component]
fn NavAnchor(
    href: &'static str,
    glyph: &'static str,
    label: &'static str,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        a {
            class: "tk-nav-item",
            href,
            onclick: move |event| onclick.call(event),
            span { class: "tk-nav-glyph", aria_hidden: "true", "{glyph}" }
            span { "{label}" }
        }
    }
}
