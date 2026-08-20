mod app;
mod model;
mod platform;
mod service;

use dioxus::prelude::*;
use toolkit_ui::ToolSurface;

pub use app::DzipArchivePage;

/// Mount the DZip archive workspace inside the shared Toolkit shell.
#[component]
pub fn DzipTool(#[props(default = true)] active: bool) -> Element {
    let _keep_workspace_mounted = active;
    rsx! {
        ToolSurface { namespace: "dzip",
            DzipArchivePage {}
        }
    }
}
