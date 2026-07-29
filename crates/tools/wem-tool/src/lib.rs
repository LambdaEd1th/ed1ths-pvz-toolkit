mod app;
mod platform;
mod processing;

use dioxus::prelude::*;
use std::sync::Arc;
use toolkit_ui::ToolSurface;

pub use app::WemAudioPage;

/// An audio file supplied by another Toolkit workspace for opening in WEM Audio.
#[derive(Clone, Debug)]
pub struct WemOpenRequest {
    id: u64,
    name: String,
    bytes: Arc<[u8]>,
}

impl WemOpenRequest {
    pub fn new(id: u64, name: impl Into<String>, bytes: Arc<[u8]>) -> Self {
        Self {
            id,
            name: name.into(),
            bytes,
        }
    }
}

impl PartialEq for WemOpenRequest {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) const WEM_ASSETS: Asset = asset!("/assets/wem", AssetOptions::folder());

/// Mount the WEM converter and player inside the Toolkit shell.
#[component]
pub fn WemTool(
    open_request: Option<WemOpenRequest>,
    #[props(default = true)] active: bool,
) -> Element {
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
            WemAudioPage { open_request }
        }
    }
}
