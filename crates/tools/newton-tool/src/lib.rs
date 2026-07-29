mod app;
mod platform;

use dioxus::prelude::*;
use std::sync::Arc;
use toolkit_ui::ToolSurface;

pub use app::NewtonManifestPage;

/// A NEWTON manifest supplied by another Toolkit workspace.
#[derive(Clone, Debug)]
pub struct NewtonOpenRequest {
    id: u64,
    name: String,
    bytes: Arc<[u8]>,
}

impl NewtonOpenRequest {
    pub fn new(id: u64, name: impl Into<String>, bytes: Arc<[u8]>) -> Self {
        Self {
            id,
            name: name.into(),
            bytes,
        }
    }
}

impl PartialEq for NewtonOpenRequest {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Mount the NEWTON manifest editor inside the Toolkit shell.
#[component]
pub fn NewtonTool(
    open_request: Option<NewtonOpenRequest>,
    #[props(default = true)] active: bool,
) -> Element {
    rsx! {
        ToolSurface { namespace: "newton",
            NewtonManifestPage { open_request, active }
        }
    }
}
