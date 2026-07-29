mod actions;
mod components;
mod i18n;
mod platform;
mod state;

use dioxus::prelude::*;
use toolkit_ui::{Appearance, ToolSurface, use_appearance};

use crate::components::PamPage;
use crate::state::{AppContext, Theme};

fn pam_theme(appearance: Appearance) -> Theme {
    match appearance {
        Appearance::System => Theme::System,
        Appearance::Light => Theme::Light,
        Appearance::Dark => Theme::Dark,
    }
}

/// Mount the PAM viewer/exporter inside the Toolkit shell.
#[component]
pub fn PamTool(#[props(default = true)] active: bool) -> Element {
    let appearance = use_appearance().preference();
    let theme = pam_theme(appearance);
    let mut context = use_hook(move || AppContext::new(theme));
    use_context_provider(|| context);
    use_effect(use_reactive(&active, move |active| {
        if !active {
            context.playing.set(false);
            document::eval(
                "window.pamStage?.destroy?.(); \
                 window.pamNativeStageHost?.destroy?.(); \
                 window.pamStagePointerCapture?.destroy?.();",
            );
        }
    }));
    use_effect(use_reactive(&appearance, move |appearance| {
        let theme = pam_theme(appearance);
        if context.preferences.peek().theme != theme {
            context.preferences.write().theme = theme;
            context.sync_stage();
        }
    }));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let renderer =
            crate::platform::native_renderer::use_native_renderer(context.shared_stage());
        use_context_provider(|| renderer.clone());
        let theme_renderer = renderer.clone();
        use_effect(use_reactive(&appearance, move |appearance| {
            theme_renderer.set_theme(pam_theme(appearance));
        }));
        let active_renderer = renderer.clone();
        use_effect(use_reactive(&active, move |active| {
            if active {
                context.sync_stage();
                document::eval("document.documentElement.classList.add('native-wgpu-host');");
            } else {
                context
                    .stage
                    .read()
                    .update(|scene| scene.set_document(None));
                document::eval("document.documentElement.classList.remove('native-wgpu-host');");
            }
            active_renderer.request_redraw();
        }));
    }
    #[cfg(target_arch = "wasm32")]
    {
        let mut warm_up_started = use_signal(|| false);
        use_effect(use_reactive(&active, move |active| {
            if active && !warm_up_started() {
                warm_up_started.set(true);
                spawn(async {
                    match crate::platform::processing::warm_up().await {
                        Ok(()) => {
                            crate::platform::log_buffer::push(
                                "INFO",
                                "WORKER",
                                "Initialized (web)",
                            );
                        }
                        Err(error) => {
                            crate::platform::log_buffer::push(
                                "ERROR",
                                "WORKER",
                                &format!("Warm-up failed: {error}"),
                            );
                        }
                    }
                });
            }
        }));
    }
    actions::use_playback_clock();
    rsx! {
        if active {
            ToolSurface { namespace: "pam",
                PamPage {}
            }
        }
    }
}
