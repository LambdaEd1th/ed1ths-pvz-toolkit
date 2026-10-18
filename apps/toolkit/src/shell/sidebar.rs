use dioxus::prelude::*;
use toolkit_ui::use_modal_focus;

use crate::i18n::use_i18n;
use crate::navigation::{AppRoute, navigate};
use crate::tool_registry::TOOLS;

#[component]
pub(crate) fn Sidebar(route: Signal<AppRoute>, open: Signal<bool>, compact: bool) -> Element {
    let i18n = use_i18n();
    let active_route = route();
    let sidebar_class = if open() {
        "tk-sidebar tk-island tk-sidebar--open"
    } else {
        "tk-sidebar tk-island"
    };
    let mut close_sidebar = open;
    let mut close_by_keyboard = open;
    use_modal_focus(
        "tk-sidebar-modal",
        open() && compact,
        "#tk-sidebar",
        None,
        Some(".tk-sidebar-backdrop, .tk-appbar"),
    );

    rsx! {
        button {
            class: if open() { "tk-sidebar-backdrop tk-sidebar-backdrop--visible" } else { "tk-sidebar-backdrop" },
            aria_label: i18n.t("sidebar-close-navigation"),
            aria_hidden: !open(),
            inert: (!open()).then_some(true),
            onclick: move |_| close_sidebar.set(false),
        }

        aside {
            id: "tk-sidebar",
            class: "{sidebar_class}",
            aria_hidden: !open(),
            inert: (!open()).then_some(true),
            onkeydown: move |event| {
                if compact && open() && event.key() == Key::Escape {
                    event.prevent_default();
                    event.stop_propagation();
                    close_by_keyboard.set(false);
                }
            },
            nav { class: "tk-sidebar-nav", aria_label: i18n.t("sidebar-navigation"),
                div { class: "tk-nav-group",
                    span { class: "tk-nav-label", {i18n.t("nav-overview")} }
                    NavItem {
                        active: active_route == AppRoute::Home,
                        glyph: "⌂",
                        label: i18n.t("nav-home"),
                        onclick: move |_| navigate(route, open, compact, AppRoute::Home),
                    }
                }

                div { class: "tk-nav-group",
                    span { class: "tk-nav-label", {i18n.t("nav-workspaces")} }
                    for tool in TOOLS {
                        NavItem {
                            active: active_route == tool.route,
                            glyph: tool.nav_glyph,
                            label: tool.label(i18n),
                            kind: Some(tool.slug),
                            experimental: tool.experimental,
                            onclick: move |_| navigate(route, open, compact, tool.route),
                        }
                    }
                }

                div { class: "tk-nav-group",
                    span { class: "tk-nav-label", {i18n.t("nav-developer")} }
                    NavItem {
                        active: active_route == AppRoute::Libraries,
                        glyph: "◇",
                        label: i18n.t("nav-libraries"),
                        onclick: move |_| navigate(route, open, compact, AppRoute::Libraries),
                    }
                    NavItem {
                        active: active_route == AppRoute::About,
                        glyph: "i",
                        label: i18n.t("nav-about"),
                        onclick: move |_| navigate(route, open, compact, AppRoute::About),
                    }
                }
            }
        }
    }
}

#[component]
fn NavItem(
    active: bool,
    glyph: &'static str,
    label: String,
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
            "data-tool": kind.unwrap_or_default(),
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
