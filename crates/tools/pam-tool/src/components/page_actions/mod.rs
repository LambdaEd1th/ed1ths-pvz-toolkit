mod export;
mod layers;
mod playback;
mod selector;
mod view;

use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdFolderOpen, LdMenu, LdPanelRight, LdX};

use crate::actions::{clear_tabs, set_resource_sheet_open};
#[cfg(target_arch = "wasm32")]
use crate::actions::{input_files_from_dioxus, load_inputs};
use crate::i18n::tr;
use crate::state::AppContext;

use super::primitives::icon;
use export::{ConvertGroup, ExportGroup};
use layers::LayerGroup;
use playback::{PlaybackGroup, SpeedGroup};
use selector::SelectorGroup;
use view::{SizeGroup, ViewGroup};

#[component]
pub fn PageActions() -> Element {
    let context = use_context::<AppContext>();
    let active_tab = context.active_tab_snapshot();
    let has_active_tab = active_tab.is_some();
    let preferences = context.preferences.read().clone();
    let images_sheet_open = *context.images_sheet_open.read();
    let sprites_sheet_open = *context.sprites_sheet_open.read();
    let locale = preferences.locale;
    let active_name = active_tab
        .map(|tab| tab.display_name())
        .unwrap_or_else(|| tr(locale, "no_animation").into());
    let mut action_groups = vec!["selectors", "view", "size", "export", "convert"];
    if has_active_tab {
        action_groups.insert(1, "layers");
    }

    rsx! {
        div {
            class: "ui-island ui-tool-page-actions pam-page-actions",
            button {
                r#type: "button",
                class: if images_sheet_open {
                    "pam-page-icon-button active"
                } else {
                    "pam-page-icon-button"
                },
                title: tr(locale, "images"),
                aria_label: tr(locale, "images"),
                aria_pressed: images_sheet_open,
                onclick: move |_| {
                    set_resource_sheet_open(context, true, !images_sheet_open);
                },
                {icon(LdMenu)}
            }
            FileGroup {}
            div { class: "pam-document-pill", title: "{active_name}",
                span { class: "pam-document-dot" }
                span { "{active_name}" }
            }
            button {
                r#type: "button",
                class: if sprites_sheet_open {
                    "pam-page-icon-button active"
                } else {
                    "pam-page-icon-button"
                },
                title: tr(locale, "sprites"),
                aria_label: tr(locale, "sprites"),
                aria_pressed: sprites_sheet_open,
                onclick: move |_| {
                    set_resource_sheet_open(context, false, !sprites_sheet_open);
                },
                {icon(LdPanelRight)}
            }
            div { class: "pam-toolbar-action-groups",
                for group in action_groups {
                    ActionGroup {
                        key: "toolbar-{group}",
                        id: group.to_string(),
                        {action_group_content(group)}
                    }
                }
            }
        }
    }
}

#[component]
fn ActionGroup(id: String, children: Element) -> Element {
    rsx! {
        div { class: "pam-action-group-shell", "data-group": "{id}",
            div { class: "pam-action-group", {children} }
        }
    }
}

fn action_group_content(id: &str) -> Element {
    match id {
        "selectors" => rsx! { SelectorGroup {} },
        "layers" => rsx! { LayerGroup {} },
        "view" => rsx! { ViewGroup {} },
        "size" => rsx! { SizeGroup {} },
        "export" => rsx! { ExportGroup {} },
        "convert" => rsx! { ConvertGroup {} },
        _ => rsx! {},
    }
}

#[component]
pub fn PlaybackDock() -> Element {
    rsx! {
        div { class: "pam-playback-dock",
            PlaybackGroup {}
            SpeedGroup {}
        }
    }
}

#[component]
fn FileGroup() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let disabled = context.tabs.read().is_empty();
    rsx! {
        LoadButton {}
        button {
            r#type: "button",
            class: "pam-icon-button quiet",
            disabled,
            title: tr(locale, "clear"),
            aria_label: tr(locale, "clear"),
            onclick: move |_| clear_tabs(context),
            {icon(LdX)}
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
pub fn LoadButton(#[props(default)] large: bool) -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let class = if large {
        "pam-button primary large"
    } else {
        "pam-icon-button primary"
    };
    rsx! {
        label { class,
            title: tr(locale, "load"),
            aria_label: tr(locale, "load"),
            input {
                class: "pam-file-input",
                r#type: "file",
                multiple: true,
                directory: true,
                onchange: move |event| async move {
                    match input_files_from_dioxus(event.files()).await {
                        Ok(files) if !files.is_empty() => load_inputs(context, files),
                        Ok(_) => {}
                        Err(error) => context.set_status(crate::state::Status::new(
                            error,
                            crate::state::Tone::Error,
                        )),
                    }
                },
            }
            {icon(LdFolderOpen)}
            if large {
                span { {tr(locale, "load")} }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn LoadButton(#[props(default)] large: bool) -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let class = if large {
        "pam-button primary large"
    } else {
        "pam-icon-button primary"
    };
    rsx! {
        button {
            r#type: "button",
            class,
            title: tr(locale, "load"),
            aria_label: tr(locale, "load"),
            onclick: move |_| {
                if let Some(root) = crate::platform::pick_animation_folder() {
                    crate::actions::load_folder(context, root);
                }
            },
            {icon(LdFolderOpen)}
            if large {
                span { {tr(locale, "load")} }
            }
        }
    }
}

#[component]
pub(super) fn FieldLabel(text: String, children: Element) -> Element {
    rsx! {
        span { class: "pam-field-label",
            span { class: "pam-field-caption", "{text}" }
            {children}
        }
    }
}
