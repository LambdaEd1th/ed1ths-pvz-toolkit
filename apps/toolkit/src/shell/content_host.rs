use dioxus::prelude::*;
use pam_tool::PamTool;
use rton_tool::RtonTool;

use crate::navigation::{AppRoute, navigate};
use crate::pages::{AboutPage, HomeDashboard};

#[component]
pub(crate) fn ContentHost(
    route: Signal<AppRoute>,
    sidebar_open: Signal<bool>,
    compact: bool,
) -> Element {
    let active_route = route();
    rsx! {
        div {
            class: if active_route == AppRoute::Home { "tk-page-slot tk-page-slot--active" } else { "tk-page-slot" },
            aria_hidden: active_route != AppRoute::Home,
            HomeDashboard {
                on_navigate: move |target| navigate(route, sidebar_open, compact, target)
            }
        }
        div {
            class: if active_route == AppRoute::About { "tk-page-slot tk-page-slot--active" } else { "tk-page-slot" },
            aria_hidden: active_route != AppRoute::About,
            AboutPage {}
        }
        div {
            class: if active_route == AppRoute::Pam { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Pam,
            PamTool { active: active_route == AppRoute::Pam }
        }
        div {
            class: if active_route == AppRoute::Rton { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Rton,
            RtonTool {}
        }
    }
}
