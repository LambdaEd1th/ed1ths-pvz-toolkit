mod app;
mod platform;

use dioxus::prelude::*;
use toolkit_ui::ToolSurface;

pub use app::ParticlePage;

/// Mount the Particle editor inside the shared Toolkit shell.
#[component]
pub fn ParticleTool() -> Element {
    rsx! {
        ToolSurface { namespace: "particle",
            ParticlePage {}
        }
    }
}
