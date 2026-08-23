use dioxus::prelude::*;
use toolkit_ui::use_appearance;

use crate::navigation::AppRoute;

use super::content_host::ContentHost;
use super::settings::SettingsPanel;
use super::sidebar::Sidebar;
use super::topbar::TopBar;

const TOOLKIT_CSS: Asset = asset!("/assets/toolkit.css");
const SHELL_RESPONSIVE_HOST: &str = r#"
return (async function () {
    window.toolkitResponsiveHost?.destroy();
    const media = window.matchMedia("(max-width: 1200px)");
    const report = () => dioxus.send(media.matches);
    const host = {
        destroy() {
            media.removeEventListener("change", report);
            if (window.toolkitResponsiveHost === host) window.toolkitResponsiveHost = null;
        },
    };
    window.toolkitResponsiveHost = host;
    media.addEventListener("change", report);
    report();
    await new Promise(() => {});
})();
"#;

#[component]
pub(crate) fn AppShell() -> Element {
    let route = use_signal(AppRoute::default);
    let sidebar_open = use_signal(|| true);
    let compact_shell = use_signal(|| false);
    let settings_open = use_signal(|| false);
    #[cfg(not(target_arch = "wasm32"))]
    let last_saved_window_size = use_signal(|| Some(crate::preferences::read_window_size()));
    #[cfg(not(target_arch = "wasm32"))]
    let window_size_save_generation = use_signal(|| 0_u64);
    let appearance = use_appearance().preference();

    let shell_class = format!("tk-shell tk-portal {}", appearance.class());
    let viewport_class = if sidebar_open() {
        "tk-content-viewport tk-content-viewport--sidebar"
    } else {
        "tk-content-viewport"
    };

    let mut responsive_sidebar = sidebar_open;
    let mut compact_signal = compact_shell;
    let start_responsive_host = move |_| {
        let mut evaluator = document::eval(SHELL_RESPONSIVE_HOST);
        spawn(async move {
            while let Ok(compact) = evaluator.recv::<bool>().await {
                compact_signal.set(compact);
                responsive_sidebar.set(!compact);
            }
        });
    };

    rsx! {
        document::Stylesheet { href: TOOLKIT_CSS }
        div {
            class: "{shell_class}",
            onmounted: start_responsive_host,
            onresize: move |event| {
                #[cfg(not(target_arch = "wasm32"))]
                if let Ok(size) = event.get_content_box_size() {
                    schedule_window_size_save(
                        size.width,
                        size.height,
                        last_saved_window_size,
                        window_size_save_generation,
                    );
                }
                #[cfg(target_arch = "wasm32")]
                let _ = event;
            },
            div { class: "tk-backdrop", aria_hidden: "true" }
            div { class: "tk-ambient tk-ambient--one", aria_hidden: "true" }
            div { class: "tk-ambient tk-ambient--two", aria_hidden: "true" }

            TopBar {
                route,
                sidebar_open,
                settings_open,
            }
            SettingsPanel { open: settings_open }
            Sidebar {
                route,
                open: sidebar_open,
                compact: compact_shell(),
            }

            main { class: "{viewport_class}",
                ContentHost {
                    route,
                    sidebar_open,
                    compact: compact_shell(),
                }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn schedule_window_size_save(
    width: f64,
    height: f64,
    mut last_saved: Signal<Option<crate::preferences::WindowSize>>,
    mut generation: Signal<u64>,
) {
    let Some(size) = crate::preferences::window_size_from_viewport(width, height) else {
        return;
    };
    let next_generation = generation.peek().wrapping_add(1);
    generation.set(next_generation);
    if *last_saved.peek() == Some(size) {
        return;
    }

    spawn(async move {
        futures_timer::Delay::new(std::time::Duration::from_millis(350)).await;
        if *generation.peek() != next_generation || *last_saved.peek() == Some(size) {
            return;
        }
        if crate::preferences::save_window_size(size).is_ok() {
            last_saved.set(Some(size));
        }
    });
}
