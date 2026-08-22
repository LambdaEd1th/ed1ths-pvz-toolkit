mod app;
mod platform;
mod processing;

use dioxus::prelude::*;
use toolkit_ui::ToolSurface;

pub use app::RsbPatchPage;

#[cfg(target_arch = "wasm32")]
pub(crate) const RSB_PATCH_ASSETS: Asset = asset!("/assets/rsb-patch", AssetOptions::folder());

/// Mount the RSBPatch workspace inside the shared Toolkit shell.
#[component]
pub fn RsbPatchTool() -> Element {
    rsx! {
        ToolSurface { namespace: "rsb-patch",
            RsbPatchPage {}
        }
    }
}
