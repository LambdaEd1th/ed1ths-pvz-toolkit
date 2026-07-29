use dioxus::prelude::*;
use newton_tool::{NewtonOpenRequest, NewtonTool};
use pam_tool::PamTool;
use rsb_tool::{RsbNewtonOpenRequest, RsbRtonOpenRequest, RsbTool, RsbWemOpenRequest};
use rton_tool::{RtonOpenRequest, RtonTool};
use wem_tool::{WemOpenRequest, WemTool};

use crate::navigation::{AppRoute, navigate};
use crate::pages::{AboutPage, HomeDashboard};

#[component]
pub(crate) fn ContentHost(
    route: Signal<AppRoute>,
    sidebar_open: Signal<bool>,
    compact: bool,
) -> Element {
    let mut next_newton_open_request_id = use_signal(|| 1_u64);
    let mut newton_open_request = use_signal(|| None::<NewtonOpenRequest>);
    let mut next_rton_open_request_id = use_signal(|| 1_u64);
    let mut rton_open_request = use_signal(|| None::<RtonOpenRequest>);
    let mut next_wem_open_request_id = use_signal(|| 1_u64);
    let mut wem_open_request = use_signal(|| None::<WemOpenRequest>);
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
            class: if active_route == AppRoute::Rsb { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Rsb,
            RsbTool {
                on_open_newton: move |request: RsbNewtonOpenRequest| {
                    let id = next_newton_open_request_id();
                    next_newton_open_request_id.set(id.wrapping_add(1).max(1));
                    newton_open_request.set(Some(NewtonOpenRequest::new(
                        id,
                        request.name,
                        request.bytes,
                    )));
                    navigate(route, sidebar_open, compact, AppRoute::Newton);
                },
                on_open_rton: move |request: RsbRtonOpenRequest| {
                    let id = next_rton_open_request_id();
                    next_rton_open_request_id.set(id.wrapping_add(1).max(1));
                    rton_open_request.set(Some(RtonOpenRequest::new(
                        id,
                        request.name,
                        request.bytes,
                    )));
                    navigate(route, sidebar_open, compact, AppRoute::Rton);
                },
                on_open_wem: move |request: RsbWemOpenRequest| {
                    let id = next_wem_open_request_id();
                    next_wem_open_request_id.set(id.wrapping_add(1).max(1));
                    wem_open_request.set(Some(WemOpenRequest::new(
                        id,
                        request.name,
                        request.bytes,
                    )));
                    navigate(route, sidebar_open, compact, AppRoute::Wem);
                }
            }
        }
        div {
            class: if active_route == AppRoute::Rton { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Rton,
            RtonTool {
                active: active_route == AppRoute::Rton,
                open_request: rton_open_request(),
            }
        }
        div {
            class: if active_route == AppRoute::Pam { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Pam,
            PamTool { active: active_route == AppRoute::Pam }
        }
        div {
            class: if active_route == AppRoute::Wem { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Wem,
            WemTool {
                active: active_route == AppRoute::Wem,
                open_request: wem_open_request(),
            }
        }
        div {
            class: if active_route == AppRoute::Newton { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Newton,
            NewtonTool {
                active: active_route == AppRoute::Newton,
                open_request: newton_open_request(),
            }
        }
    }
}
