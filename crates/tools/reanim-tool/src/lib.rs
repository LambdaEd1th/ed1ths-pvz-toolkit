mod app;
mod platform;

use dioxus::prelude::*;
use toolkit_ui::ToolSurface;

pub use app::ReanimPage;

/// Mount the REANIM editor inside the shared Toolkit shell.
#[component]
pub fn ReanimTool() -> Element {
    rsx! {
        ToolSurface { namespace: "reanim",
            ReanimPage {}
        }
    }
}
