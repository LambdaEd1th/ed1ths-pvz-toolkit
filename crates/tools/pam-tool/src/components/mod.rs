mod logs;
mod panels;
mod primitives;
mod stage;
mod status;
mod tabs;
mod toolbar;

use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdImage, LdShapes};

use crate::actions::finish_toolbar_reorder;
use crate::i18n::tr;
use crate::state::{AppContext, PanelSide, Theme, panel_state_for_layout};

use panels::{ImagePanel, PanelResizeHandle, SpritePanel};
use primitives::icon;
use stage::Stage;
use status::{ExportOverlay, StatusBar};
use tabs::TabStrip;
use toolbar::Toolbar;

// Keep token definitions ahead of the base and workbench component overrides.
const TOKENS_CSS: Asset = asset!("/assets/pam/tokens.css");
const APP_CSS: Asset = asset!("/assets/pam/app.css");
const WORKBENCH_CSS: Asset = asset!("/assets/pam/workbench.css");
#[cfg(target_arch = "wasm32")]
pub(crate) const APP_ASSETS: Asset = asset!("/assets/pam", AssetOptions::folder());
const RESPONSIVE_LAYOUT_HOST: &str = include_str!("../../assets/pam/responsive_layout_host.js");

#[component]
pub fn Workbench() -> Element {
    let context = use_context::<AppContext>();
    let preferences = context.preferences.read().clone();
    let locale = preferences.locale;
    let tab = context.active_tab_snapshot();
    let panel_resizing = context.panel_resize.read().is_some();
    let compact_layout = *context.compact_layout.read();
    let images_panel_visible = *context.images_panel_open.read();
    let sprites_panel_visible = *context.sprites_panel_open.read();
    let theme_class = match preferences.theme {
        Theme::System => "system-theme",
        Theme::Light => "light-theme",
        Theme::Dark => "dark-theme",
    };
    let layout_class = if compact_layout {
        "compact-layout"
    } else {
        "wide-layout"
    };
    let workspace_style = format!(
        "--images-width:{}px;--sprites-width:{}px",
        preferences.image_panel_width, preferences.sprite_panel_width,
    );
    let workspace_class = format!(
        "pam-workspace {} {} {} {} {}",
        if panel_resizing { "resizing-panel" } else { "" },
        if images_panel_visible {
            "images-drawer-mounted"
        } else {
            "images-drawer-unmounted"
        },
        if images_panel_visible {
            "images-drawer-open"
        } else {
            "images-drawer-closed"
        },
        if sprites_panel_visible {
            "sprites-drawer-mounted"
        } else {
            "sprites-drawer-unmounted"
        },
        if sprites_panel_visible {
            "sprites-drawer-open"
        } else {
            "sprites-drawer-closed"
        },
    );

    rsx! {
        document::Stylesheet { href: TOKENS_CSS }
        document::Stylesheet { href: APP_CSS }
        document::Stylesheet { href: WORKBENCH_CSS }
        WorkbenchRoot { theme_class, layout_class,
            Toolbar {}
            div {
                class: "{workspace_class}",
                style: "{workspace_style}",
                if compact_layout && (images_panel_visible || sprites_panel_visible) {
                    div { class: "pam-drawer-backdrop", aria_hidden: "true" }
                }
                div { class: "pam-images-column ui-workbench-panel",
                    if tab.is_some() { ImagePanel {} }
                    else { EmptyPanel { title: tr(locale, "images").to_string(), side: PanelSide::Images } }
                }
                div { class: "pam-left-resizer", PanelResizeHandle { side: PanelSide::Images } }
                main { class: "pam-center-column",
                    TabStrip {}
                    Stage {}
                }
                div { class: "pam-right-resizer", PanelResizeHandle { side: PanelSide::Sprites } }
                div { class: "pam-sprites-column ui-workbench-panel",
                    if tab.is_some() { SpritePanel {} }
                    else { EmptyPanel { title: tr(locale, "sprites").to_string(), side: PanelSide::Sprites } }
                }
            }
            StatusBar {}
            ExportOverlay {}
        }
    }
}

#[component]
fn WorkbenchRoot(theme_class: String, layout_class: String, children: Element) -> Element {
    let mut context = use_context::<AppContext>();
    let start_responsive_host = move |_| {
        let mut evaluator = document::eval(RESPONSIVE_LAYOUT_HOST);
        spawn(async move {
            while let Ok(compact) = evaluator.recv::<bool>().await {
                let (images_open, sprites_open) = panel_state_for_layout(
                    compact,
                    *context.images_panel_open.read(),
                    *context.sprites_panel_open.read(),
                );
                if *context.images_panel_open.read() != images_open {
                    context.images_panel_open.set(images_open);
                }
                if *context.sprites_panel_open.read() != sprites_open {
                    context.sprites_panel_open.set(sprites_open);
                }
                if *context.compact_layout.read() != compact {
                    context.compact_layout.set(compact);
                }
            }
        });
    };
    rsx! {
        div {
            class: "pam-app app-shell {theme_class} {layout_class}",
            onmounted: start_responsive_host,
            onmousemove: move |event| {
                if context.panel_resize.read().is_some() {
                    event.prevent_default();
                }
                resize_panel_from_pointer(context, event.client_coordinates().x);
            },
            onmouseup: move |_| finish_pointer_gestures(context),
            onmouseleave: move |_| cancel_pointer_gestures(context),
            {children}
        }
    }
}

fn resize_panel_from_pointer(mut context: AppContext, pointer_x: f64) {
    let Some(resize) = *context.panel_resize.read() else {
        return;
    };
    let delta = match resize.side {
        PanelSide::Images => pointer_x - resize.start_x,
        PanelSide::Sprites => resize.start_x - pointer_x,
    };
    let width = (resize.start_width as f64 + delta)
        .round()
        .clamp(180.0, 500.0) as u32;
    let mut preferences = context.preferences.write();
    match resize.side {
        PanelSide::Images => preferences.image_panel_width = width,
        PanelSide::Sprites => preferences.sprite_panel_width = width,
    }
}

fn finish_pointer_gestures(mut context: AppContext) {
    if context.panel_resize.read().is_some() {
        context.panel_resize.set(None);
        context.save_preferences();
    }
    context.stage_drag.set(None);
    context.dragged_tab.set(None);
    finish_toolbar_reorder(context);
}

fn cancel_pointer_gestures(mut context: AppContext) {
    context.stage_drag.set(None);
    context.dragged_tab.set(None);
    finish_toolbar_reorder(context);
}

#[component]
fn EmptyPanel(title: String, side: PanelSide) -> Element {
    let panel_icon = match side {
        PanelSide::Images => icon(LdImage),
        PanelSide::Sprites => icon(LdShapes),
    };
    rsx! {
        aside { class: "pam-side-panel empty",
            header { class: "pam-panel-header",
                div { class: "pam-panel-title",
                    {panel_icon}
                    h2 { "{title}" }
                }
            }
        }
    }
}
