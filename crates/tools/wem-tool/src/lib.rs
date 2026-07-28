mod app;
mod platform;
mod processing;

use dioxus::prelude::*;
use toolkit_ui::ToolSurface;

pub use app::WemAudioPage;

#[cfg(target_arch = "wasm32")]
pub(crate) const WEM_ASSETS: Asset = asset!("/assets/wem", AssetOptions::folder());

/// Mount the WEM converter and player inside the Toolkit shell.
#[component]
pub fn WemTool(#[props(default = true)] active: bool) -> Element {
    use_effect(use_reactive(&active, move |active| {
        if !active {
            document::eval(
                "document.querySelectorAll('[data-tool=\"wem\"] audio').forEach((audio) => audio.pause()); \
                 window.wemPlayer?.suspend?.();",
            );
        }
    }));

    rsx! {
        ToolSurface { namespace: "wem",
            WemAudioPage {}
        }
    }
}
