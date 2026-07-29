mod app;
mod domain;
mod editing;
mod loader;
mod platform;
mod preview;
mod processing;
mod view_model;
mod virtual_scroll;

use dioxus::prelude::*;
use std::sync::Arc;
use toolkit_ui::ToolSurface;

pub use app::RsbArchivePage;

#[derive(Clone, Debug)]
pub struct RsbRtonOpenRequest {
    pub name: String,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub struct RsbNewtonOpenRequest {
    pub name: String,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub struct RsbWemOpenRequest {
    pub name: String,
    pub bytes: Arc<[u8]>,
}

#[cfg(target_arch = "wasm32")]
pub(crate) const RSB_ASSETS: Asset = asset!("/assets/rsb", AssetOptions::folder());

/// Mount the RSB archive browser inside the Toolkit shell.
#[component]
pub fn RsbTool(
    on_open_newton: Option<EventHandler<RsbNewtonOpenRequest>>,
    on_open_rton: Option<EventHandler<RsbRtonOpenRequest>>,
    on_open_wem: Option<EventHandler<RsbWemOpenRequest>>,
) -> Element {
    rsx! {
        ToolSurface { namespace: "rsb",
            RsbArchivePage { on_open_newton, on_open_rton, on_open_wem }
        }
    }
}
