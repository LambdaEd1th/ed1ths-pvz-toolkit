use dioxus::prelude::*;
use toolkit_ui::IconButton;

use crate::navigation::AppRoute;

use super::brand::BrandMark;

#[component]
pub(crate) fn TopBar(
    route: Signal<AppRoute>,
    sidebar_open: Signal<bool>,
    settings_open: Signal<bool>,
) -> Element {
    let active_route = route();
    let mut menu_sidebar = sidebar_open;
    let mut open_settings = settings_open;

    rsx! {
        header { class: "tk-appbar tk-island",
            div { class: "tk-appbar-start",
                IconButton {
                    class: "tk-icon-button".to_string(),
                    label: "切换导航".to_string(),
                    title: Some("切换导航".to_string()),
                    onclick: move |_| menu_sidebar.set(!menu_sidebar()),
                    span { class: "tk-menu-icon", aria_hidden: "true",
                        i {}
                        i {}
                        i {}
                    }
                }
                BrandMark { compact: true }
                span { class: "tk-appbar-divider", aria_hidden: "true" }
                div { class: "tk-breadcrumb",
                    span { "Toolkit" }
                    b { "/" }
                    strong { "{active_route.label()}" }
                }
            }

            div { class: "tk-appbar-context",
                span { class: "tk-context-dot" }
                span { "{active_route.label()}" }
            }

            div { class: "tk-appbar-actions",
                IconButton {
                    class: "tk-settings-button".to_string(),
                    label: "打开设置".to_string(),
                    title: Some("设置".to_string()),
                    pressed: Some(settings_open()),
                    onclick: move |_| open_settings.set(!open_settings()),
                    SettingsIcon {}
                }
            }
        }
    }
}

#[component]
fn SettingsIcon() -> Element {
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
