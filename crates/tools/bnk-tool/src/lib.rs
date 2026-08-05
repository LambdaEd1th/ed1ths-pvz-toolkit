mod app;
mod platform;

use dioxus::prelude::*;
use std::sync::Arc;
use toolkit_ui::ToolSurface;

pub use app::BnkArchivePage;

#[derive(Clone, Debug)]
pub struct BnkWemOpenRequest {
    pub name: String,
    pub bytes: Arc<[u8]>,
}

/// Mount the experimental Wwise SoundBank browser inside the Toolkit shell.
#[component]
pub fn BnkTool(
    on_open_wem: Option<EventHandler<BnkWemOpenRequest>>,
    #[props(default = true)] active: bool,
) -> Element {
    let _keep_workspace_mounted = active;

    rsx! {
        ToolSurface { namespace: "bnk",
            BnkArchivePage { on_open_wem }
        }
    }
}
