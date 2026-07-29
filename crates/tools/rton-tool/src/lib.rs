mod app_actions;
mod app_constants;
mod app_i18n;
#[cfg(test)]
mod app_sample;
mod app_view;
mod application;
mod batch_export_runner;
mod components;
mod domain;
mod file_import;
mod i18n;
mod platform;
#[cfg(test)]
mod tests;

use dioxus::prelude::*;
use std::sync::Arc;
use toolkit_ui::{Appearance, ToolSurface, use_appearance};

static LOG_INIT: std::sync::Once = std::sync::Once::new();

use domain::ThemePreference;

/// A file supplied by another Toolkit workspace for opening in RTON Editor.
#[derive(Clone, Debug)]
pub struct RtonOpenRequest {
    id: u64,
    name: String,
    bytes: Arc<[u8]>,
}

impl RtonOpenRequest {
    pub fn new(id: u64, name: impl Into<String>, bytes: Arc<[u8]>) -> Self {
        Self {
            id,
            name: name.into(),
            bytes,
        }
    }
}

impl PartialEq for RtonOpenRequest {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

fn rton_theme(appearance: Appearance) -> ThemePreference {
    match appearance {
        Appearance::System => ThemePreference::System,
        Appearance::Light => ThemePreference::Light,
        Appearance::Dark => ThemePreference::Dark,
    }
}

/// Mount the RTON editor inside the Toolkit shell.
#[component]
pub fn RtonTool(
    open_request: Option<RtonOpenRequest>,
    #[props(default = true)] active: bool,
) -> Element {
    use_hook(|| {
        LOG_INIT.call_once(|| {
            #[cfg(target_arch = "wasm32")]
            platform::log_buffer::init_wasm(log::LevelFilter::Debug);

            #[cfg(not(target_arch = "wasm32"))]
            platform::log_buffer::init_silent(log::LevelFilter::Debug);
        });
    });
    let appearance = use_appearance().preference();
    rsx! {
        ToolSurface { namespace: "rton",
            app_view::App {
                theme: rton_theme(appearance),
                open_request,
                active,
            }
        }
    }
}
