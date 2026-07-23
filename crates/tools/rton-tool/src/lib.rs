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
use toolkit_ui::{Appearance, WorkbenchSurface, use_appearance};

static LOG_INIT: std::sync::Once = std::sync::Once::new();

use domain::ThemePreference;

fn rton_theme(appearance: Appearance) -> ThemePreference {
    match appearance {
        Appearance::System => ThemePreference::System,
        Appearance::Light => ThemePreference::Light,
        Appearance::Dark => ThemePreference::Dark,
    }
}

/// Mount the RTON editor inside the Toolkit shell.
#[component]
pub fn RtonTool() -> Element {
    use_hook(|| {
        LOG_INIT.call_once(|| {
            #[cfg(target_arch = "wasm32")]
            platform::log_buffer::init_wasm(log::LevelFilter::Debug);

            #[cfg(not(target_arch = "wasm32"))]
            {
                let logger = env_logger::Builder::from_default_env()
                    .filter_level(log::LevelFilter::Info)
                    .build();
                platform::log_buffer::init(Box::new(logger), log::LevelFilter::Debug);
            }
        });
    });
    let appearance = use_appearance().preference();
    rsx! {
        WorkbenchSurface { namespace: "rton",
            app_view::App { theme: rton_theme(appearance) }
        }
    }
}
