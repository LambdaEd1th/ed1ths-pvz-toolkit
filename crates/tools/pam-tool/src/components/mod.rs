mod editor;
mod page_actions;
mod panels;
mod primitives;
mod stage;
mod status;
mod tabs;
mod workspace;

use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdImage, LdShapes};
use toolkit_ui::{ContextSheet, ToolPage, ToolPageToolbar, WorkspaceCard};

use crate::actions::set_resource_sheet_open;
use crate::i18n::tr;
use crate::state::{AppContext, Theme};

use page_actions::{PageActions, PlaybackDock};
use panels::{ImagePanel, SpritePanel};
use primitives::icon;
use stage::Stage;
use status::{ExportOverlay, InlineStatus};
use tabs::TabStrip;

// Keep token definitions ahead of page component overrides.
const TOKENS_CSS: Asset = asset!("/assets/pam/tokens.css");
const PAGE_CSS: Asset = asset!("/assets/pam/page.css");
const EDITOR_CSS: Asset = asset!("/assets/pam/editor.css");
#[cfg(target_arch = "wasm32")]
pub(crate) const APP_ASSETS: Asset = asset!("/assets/pam", AssetOptions::folder());

#[component]
pub fn PamPage() -> Element {
    let context = use_context::<AppContext>();
    let preferences = context.preferences.read().clone();
    let locale = preferences.locale;
    let tab = context.active_tab_snapshot();
    let mut inspector_open = use_signal(|| false);
    let images_sheet_open = *context.images_sheet_open.read();
    let sprites_sheet_open = *context.sprites_sheet_open.read();
    let theme_class = match preferences.theme {
        Theme::System => "system-theme",
        Theme::Light => "light-theme",
        Theme::Dark => "dark-theme",
    };

    rsx! {
        document::Stylesheet { href: TOKENS_CSS }
        document::Stylesheet { href: PAGE_CSS }
        document::Stylesheet { href: EDITOR_CSS }
        div {
            class: "pam-page-host {theme_class}",
            onmouseup: move |_| finish_pointer_gestures(context),
            onmouseleave: move |_| cancel_pointer_gestures(context),
            ToolPage { namespace: "pam", class: "pam-app",
                ToolPageToolbar {
                    class: "pam-page-toolbar",
                    actions: rsx! { PageActions {} },
                }

                TabStrip {}
                workspace::EditorToolbar { properties_open: inspector_open(), on_properties: move |_| inspector_open.toggle() }

                div { class: if inspector_open() { "pam-editor-workspace is-inspector-open" } else { "pam-editor-workspace" },
                    div { class: "pam-editor-center",
                        WorkspaceCard { class: "pam-preview-card pam-editor-canvas", aria_label: tr(locale, "stage"),
                            if let Some(tab) = tab.as_ref() {
                                header { class: "pam-editor-canvas-heading",
                                    span { {tr(locale, "stage")} }
                                    span { "{tab.document.pam.size[0]} × {tab.document.pam.size[1]} · {tab.document.pam.frame_rate} FPS" }
                                }
                            }
                            div { class: "pam-stage-frame", Stage {} }
                            div { class: "pam-editor-canvas-status", InlineStatus {} }
                        }
                        if tab.is_some() {
                            workspace::EditorTimeline {}
                        } else {
                            div { class: "pam-preview-controls", PlaybackDock {} }
                        }
                    }
                    if tab.is_some() {
                        workspace::EditorInspector { on_close: move |_| inspector_open.set(false) }
                    }
                }

                ContextSheet {
                    open: images_sheet_open,
                    side: "left",
                    title: tr(locale, "images"),
                    close_label: tr(locale, "close_menu"),
                    on_close: move |_| set_resource_sheet_open(context, true, false),
                    if tab.is_some() {
                        ImagePanel {}
                    } else {
                        EmptyResourceSheet { title: tr(locale, "images"), images: true }
                    }
                }
                ContextSheet {
                    open: sprites_sheet_open,
                    title: tr(locale, "sprites"),
                    close_label: tr(locale, "close_menu"),
                    on_close: move |_| set_resource_sheet_open(context, false, false),
                    if tab.is_some() {
                        SpritePanel {}
                    } else {
                        EmptyResourceSheet { title: tr(locale, "sprites"), images: false }
                    }
                }
                ExportOverlay {}
                editor::CloseConfirmation {}
            }
        }
    }
}

fn finish_pointer_gestures(mut context: AppContext) {
    crate::actions::finish_edit_gesture(context);
    context.stage_drag.set(None);
    context.dragged_tab.set(None);
}

fn cancel_pointer_gestures(mut context: AppContext) {
    crate::actions::finish_edit_gesture(context);
    context.stage_drag.set(None);
    context.dragged_tab.set(None);
}

#[component]
fn EmptyResourceSheet(title: String, images: bool) -> Element {
    let panel_icon = if images {
        icon(LdImage)
    } else {
        icon(LdShapes)
    };
    rsx! {
        div { class: "pam-resource-sheet empty",
            header { class: "pam-panel-header",
                div { class: "pam-panel-title",
                    {panel_icon}
                    h2 { "{title}" }
                }
            }
        }
    }
}
