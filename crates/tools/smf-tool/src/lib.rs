mod app;
mod platform;
mod processing;

use dioxus::prelude::*;
use toolkit_ui::ToolSurface;

pub use app::SmfPage;

#[cfg(target_arch = "wasm32")]
pub(crate) const SMF_ASSETS: Asset = asset!("/assets/smf", AssetOptions::folder());

/// Mount the SMF compressor inside the shared Toolkit shell.
#[component]
pub fn SmfTool() -> Element {
    rsx! {
        ToolSurface { namespace: "smf",
            SmfPage {}
        }
    }
}
