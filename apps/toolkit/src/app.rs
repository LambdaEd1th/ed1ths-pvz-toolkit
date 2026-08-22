use dioxus::prelude::*;
use toolkit_ui::{AppearanceProvider, UiStyles};

use crate::shell::AppShell;

#[component]
fn App() -> Element {
    rsx! {
        UiStyles {}
        AppearanceProvider {
            AppShell {}
        }
    }
}

pub(crate) fn launch() {
    #[cfg(target_arch = "wasm32")]
    dioxus::launch(App);

    #[cfg(not(target_arch = "wasm32"))]
    {
        use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

        let window = WindowBuilder::new()
            .with_title("Ed1th's PvZ Toolkit")
            .with_inner_size(LogicalSize::new(1440.0, 900.0))
            .with_min_inner_size(LogicalSize::new(720.0, 560.0))
            .with_transparent(true)
            .with_background_color((14, 17, 23, 255));
        let config = Config::new()
            .with_window(window)
            .with_menu(None)
            .with_disable_context_menu(true)
            .with_on_window(move |_window, _| {
                #[cfg(target_os = "macos")]
                {
                    pam_viewer_native_window::install_safe_reopen_handler();
                    pam_viewer_native_window::make_opaque(&_window);
                }
            });
        dioxus::LaunchBuilder::desktop()
            .with_cfg(config)
            .launch(App);
    }
}
