use dioxus::prelude::*;
use toolkit_ui::{AppearanceProvider, UiStyles};

use crate::i18n::LocaleProvider;
use crate::shell::AppShell;

#[component]
fn App() -> Element {
    rsx! {
        UiStyles {}
        AppearanceProvider {
            LocaleProvider {
                AppShell {}
            }
        }
    }
}

pub(crate) fn launch() {
    #[cfg(target_arch = "wasm32")]
    dioxus::launch(App);

    #[cfg(not(target_arch = "wasm32"))]
    {
        use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

        let saved_size = crate::preferences::read_window_size();
        let window = WindowBuilder::new()
            .with_title("Ed1th's PvZ Toolkit")
            .with_inner_size(LogicalSize::new(
                f64::from(saved_size.width),
                f64::from(saved_size.height),
            ))
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
                    use dioxus::desktop::tao::platform::macos::WindowExtMacOS;

                    pam_viewer_native_window::install_safe_reopen_handler();
                    // SAFETY: Dioxus/Tao owns this NSWindow for the duration of
                    // the callback and returns its live native pointer here.
                    unsafe { pam_viewer_native_window::make_opaque(_window.ns_window()) };
                }
            });
        dioxus::LaunchBuilder::desktop()
            .with_cfg(config)
            .launch(App);
    }
}
