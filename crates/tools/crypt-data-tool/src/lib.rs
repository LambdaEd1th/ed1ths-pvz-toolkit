mod app;
mod platform;
mod processing;

use dioxus::prelude::*;
use toolkit_ui::ToolSurface;

pub use app::CryptDataPage;

/// Mount the Crypt-Data converter inside the shared Toolkit shell.
#[component]
pub fn CryptDataTool() -> Element {
    rsx! {
        ToolSurface { namespace: "crypt-data",
            CryptDataPage {}
        }
    }
}
