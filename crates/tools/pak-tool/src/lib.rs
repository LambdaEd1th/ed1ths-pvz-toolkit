mod app;
mod platform;

use dioxus::prelude::*;
use std::sync::Arc;
use toolkit_ui::ToolSurface;

pub use app::PakArchivePage;

#[derive(Clone, Debug)]
pub struct PakRtonOpenRequest {
    pub name: String,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub struct PakNewtonOpenRequest {
    pub name: String,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub struct PakWemOpenRequest {
    pub name: String,
    pub bytes: Arc<[u8]>,
}

/// Mount the PopCap PAK archive workspace inside the Toolkit shell.
#[component]
pub fn PakTool(
    on_open_newton: Option<EventHandler<PakNewtonOpenRequest>>,
    on_open_rton: Option<EventHandler<PakRtonOpenRequest>>,
    on_open_wem: Option<EventHandler<PakWemOpenRequest>>,
    #[props(default = true)] active: bool,
) -> Element {
    let _keep_workspace_mounted = active;
    rsx! {
        ToolSurface { namespace: "pak",
            PakArchivePage { on_open_newton, on_open_rton, on_open_wem }
        }
    }
}
