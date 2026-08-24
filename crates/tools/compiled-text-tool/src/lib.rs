mod app;
mod platform;
mod processing;

use dioxus::prelude::*;
use toolkit_ui::ToolSurface;

pub use app::CompiledTextPage;

#[cfg(target_arch = "wasm32")]
pub(crate) const COMPILED_TEXT_ASSETS: Asset =
    asset!("/assets/compiled-text", AssetOptions::folder());

/// Mount the Compiled Text editor inside the shared Toolkit shell.
#[component]
pub fn CompiledTextTool() -> Element {
    rsx! {
        ToolSurface { namespace: "compiled-text",
            CompiledTextPage {}
        }
    }
}
