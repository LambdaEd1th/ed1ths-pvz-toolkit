use bnk_tool::{BnkTool, BnkWemOpenRequest};
use compiled_text_tool::CompiledTextTool;
use crypt_data_tool::CryptDataTool;
use dioxus::prelude::*;
use dzip_tool::DzipTool;
use newton_tool::{NewtonOpenRequest, NewtonTool};
use pak_tool::{PakNewtonOpenRequest, PakRtonOpenRequest, PakTool, PakWemOpenRequest};
use pam_tool::PamTool;
use particle_tool::ParticleTool;
use reanim_tool::ReanimTool;
use rsb_patch_tool::RsbPatchTool;
use rsb_tool::{RsbNewtonOpenRequest, RsbRtonOpenRequest, RsbTool, RsbWemOpenRequest};
use rton_tool::{RtonOpenRequest, RtonTool};
use smf_tool::SmfTool;
use wem_tool::{WemOpenRequest, WemTool};

use crate::i18n::use_locale;
use crate::navigation::{AppRoute, navigate};
use crate::pages::{AboutPage, HomeDashboard, LibrariesPage};

#[component]
pub(crate) fn ContentHost(
    route: Signal<AppRoute>,
    sidebar_open: Signal<bool>,
    compact: bool,
) -> Element {
    let locale_code = use_locale().locale().code().to_string();
    let mut next_newton_open_request_id = use_signal(|| 1_u64);
    let mut newton_open_request = use_signal(|| None::<NewtonOpenRequest>);
    let mut next_rton_open_request_id = use_signal(|| 1_u64);
    let mut rton_open_request = use_signal(|| None::<RtonOpenRequest>);
    let mut next_wem_open_request_id = use_signal(|| 1_u64);
    let mut wem_open_request = use_signal(|| None::<WemOpenRequest>);
    let mut tool_request = use_signal(|| None::<toolkit_ui::ToolOpenRequest>);
    let mut next_tool_request = use_signal(|| 1_u64);
    use_context_provider(|| toolkit_ui::ToolOpenBus(tool_request));
    use_context_provider(|| {
        toolkit_ui::ToolOpenHandler(EventHandler::new(
            move |(kind, files): (toolkit_ui::ToolKind, Vec<toolkit_ui::ToolFile>)| {
                use toolkit_ui::ToolKind;
                let id = next_tool_request();
                next_tool_request.set(id.wrapping_add(1).max(1));
                let target = match kind {
                    ToolKind::Rsb => AppRoute::Rsb,
                    ToolKind::RsbPatch => AppRoute::RsbPatch,
                    ToolKind::Pak => AppRoute::Pak,
                    ToolKind::Dzip => AppRoute::Dzip,
                    ToolKind::Smf => AppRoute::Smf,
                    ToolKind::CompiledText => AppRoute::CompiledText,
                    ToolKind::CryptData => AppRoute::CryptData,
                    ToolKind::Rton => AppRoute::Rton,
                    ToolKind::Pam => AppRoute::Pam,
                    ToolKind::Particle => AppRoute::Particle,
                    ToolKind::Reanim => AppRoute::Reanim,
                    ToolKind::Wem => AppRoute::Wem,
                    ToolKind::Bnk => AppRoute::Bnk,
                    ToolKind::Newton => AppRoute::Newton,
                };
                if let Some(file) = files.first() {
                    match kind {
                        ToolKind::Rton => {
                            let id = next_rton_open_request_id();
                            next_rton_open_request_id.set(id.wrapping_add(1).max(1));
                            rton_open_request.set(Some(RtonOpenRequest::new(
                                id,
                                file.name.clone(),
                                file.bytes.as_slice().into(),
                            )));
                        }
                        ToolKind::Newton => {
                            let id = next_newton_open_request_id();
                            next_newton_open_request_id.set(id.wrapping_add(1).max(1));
                            newton_open_request.set(Some(NewtonOpenRequest::new(
                                id,
                                file.name.clone(),
                                file.bytes.as_slice().into(),
                            )));
                        }
                        ToolKind::Wem => {
                            let id = next_wem_open_request_id();
                            next_wem_open_request_id.set(id.wrapping_add(1).max(1));
                            wem_open_request.set(Some(WemOpenRequest::new(
                                id,
                                file.name.clone(),
                                file.bytes.as_slice().into(),
                            )));
                        }
                        _ => {
                            tool_request.set(Some(toolkit_ui::ToolOpenRequest { id, kind, files }))
                        }
                    }
                    navigate(route, sidebar_open, compact, target);
                }
            },
        ))
    });
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
            class: if active_route == AppRoute::Libraries { "tk-page-slot tk-page-slot--active" } else { "tk-page-slot" },
            aria_hidden: active_route != AppRoute::Libraries,
            LibrariesPage {}
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
            class: if active_route == AppRoute::RsbPatch { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::RsbPatch,
            RsbPatchTool {}
        }
        div {
            class: if active_route == AppRoute::Pak { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Pak,
            PakTool {
                active: active_route == AppRoute::Pak,
                on_open_newton: move |request: PakNewtonOpenRequest| {
                    let id = next_newton_open_request_id();
                    next_newton_open_request_id.set(id.wrapping_add(1).max(1));
                    newton_open_request.set(Some(NewtonOpenRequest::new(id, request.name, request.bytes)));
                    navigate(route, sidebar_open, compact, AppRoute::Newton);
                },
                on_open_rton: move |request: PakRtonOpenRequest| {
                    let id = next_rton_open_request_id();
                    next_rton_open_request_id.set(id.wrapping_add(1).max(1));
                    rton_open_request.set(Some(RtonOpenRequest::new(id, request.name, request.bytes)));
                    navigate(route, sidebar_open, compact, AppRoute::Rton);
                },
                on_open_wem: move |request: PakWemOpenRequest| {
                    let id = next_wem_open_request_id();
                    next_wem_open_request_id.set(id.wrapping_add(1).max(1));
                    wem_open_request.set(Some(WemOpenRequest::new(id, request.name, request.bytes)));
                    navigate(route, sidebar_open, compact, AppRoute::Wem);
                },
            }
        }
        div {
            class: if active_route == AppRoute::Dzip { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Dzip,
            DzipTool { active: active_route == AppRoute::Dzip }
        }
        div {
            class: if active_route == AppRoute::Smf { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Smf,
            SmfTool {}
        }
        div {
            class: if active_route == AppRoute::CompiledText { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::CompiledText,
            CompiledTextTool {}
        }
        div {
            class: if active_route == AppRoute::CryptData { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::CryptData,
            CryptDataTool {}
        }
        div {
            class: if active_route == AppRoute::Rton { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Rton,
            RtonTool {
                active: active_route == AppRoute::Rton,
                locale_code: Some(locale_code.clone()),
                open_request: rton_open_request(),
            }
        }
        div {
            class: if active_route == AppRoute::Pam { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Pam,
            PamTool {
                active: active_route == AppRoute::Pam,
                locale_code: Some(locale_code.clone()),
            }
        }
        div {
            class: if active_route == AppRoute::Particle { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Particle,
            ParticleTool {}
        }
        div {
            class: if active_route == AppRoute::Reanim { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Reanim,
            ReanimTool {}
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
            class: if active_route == AppRoute::Bnk { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Bnk,
            BnkTool {
                active: active_route == AppRoute::Bnk,
                on_open_wem: move |request: BnkWemOpenRequest| {
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
            class: if active_route == AppRoute::Newton { "tk-tool-slot tk-tool-slot--active" } else { "tk-tool-slot" },
            aria_hidden: active_route != AppRoute::Newton,
            NewtonTool {
                active: active_route == AppRoute::Newton,
                open_request: newton_open_request(),
            }
        }
    }
}
