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
        div { class: "{shell_class}", onmounted: start_responsive_host,
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
