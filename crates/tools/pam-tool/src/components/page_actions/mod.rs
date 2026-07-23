mod export;
mod layers;
mod playback;
mod selector;
mod view;

use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{
    LdEllipsis, LdFolderOpen, LdMenu, LdPanelRight, LdSettings, LdX,
};

use crate::actions::{clear_tabs, set_resource_sheet_open};
#[cfg(target_arch = "wasm32")]
use crate::actions::{input_files_from_dioxus, load_inputs};
use crate::i18n::tr;
use crate::state::AppContext;

use super::logs::LogViewerDialog;
use super::primitives::icon;
use export::{ConvertGroup, ExportGroup};
use layers::LayerGroup;
use playback::{PlaybackGroup, SpeedGroup};
use selector::SelectorGroup;
use view::{PreferenceGroup, SizeGroup, ViewGroup};

const DIALOG_EXIT_MS: u64 = 200;

fn close_settings_dialog(
    mut mounted: Signal<bool>,
    mut closing: Signal<bool>,
    logs_open: Signal<bool>,
) {
    if *logs_open.peek() || !*mounted.peek() || *closing.peek() {
        return;
    }
    closing.set(true);
    spawn(async move {
        crate::platform::sleep_ms(DIALOG_EXIT_MS).await;
        mounted.set(false);
        closing.set(false);
    });
}

#[component]
pub fn PageActions() -> Element {
    let context = use_context::<AppContext>();
    let active_tab = context.active_tab_snapshot();
    let has_active_tab = active_tab.is_some();
    let preferences = context.preferences.read().clone();
    let images_sheet_open = *context.images_sheet_open.read();
    let sprites_sheet_open = *context.sprites_sheet_open.read();
    let mut more_open = use_signal(|| false);
    let mut settings_mounted = use_signal(|| false);
    let mut settings_closing = use_signal(|| false);
    let mut logs_open = use_signal(|| false);
    let more_open_snapshot = *more_open.read();
    let settings_mounted_snapshot = *settings_mounted.read();
    let settings_closing_snapshot = *settings_closing.read();
    let settings_open = settings_mounted_snapshot && !settings_closing_snapshot;
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
            class: "pam-page-actions",
            FileGroup {}
            div { class: "pam-document-pill", title: "{active_name}",
                span { class: "pam-document-dot" }
                span { "{active_name}" }
            }
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
            button {
                r#type: "button",
                class: if more_open_snapshot {
                    "pam-page-icon-button active"
                } else {
                    "pam-page-icon-button"
                },
                title: tr(locale, "more"),
                aria_label: tr(locale, "more"),
                aria_expanded: more_open_snapshot,
                onclick: move |_| more_open.toggle(),
                {icon(LdEllipsis)}
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
            button {
                r#type: "button",
                class: if settings_open {
                    "pam-page-icon-button active"
                } else {
                    "pam-page-icon-button"
                },
                title: tr(locale, "settings"),
                aria_label: tr(locale, "settings"),
                aria_expanded: settings_open,
                onclick: move |_| {
                    settings_closing.set(false);
                    settings_mounted.set(true);
                },
                {icon(LdSettings)}
            }
            if more_open_snapshot {
                div { class: "pam-action-sheet-layer",
                    button {
                        r#type: "button",
                        class: "pam-action-sheet-backdrop",
                        aria_label: tr(locale, "close_menu"),
                        onclick: move |_| more_open.set(false),
                    }
                    section { class: "pam-action-sheet",
                        header { class: "pam-action-sheet-header",
                            strong { {tr(locale, "more")} }
                            button {
                                r#type: "button",
                                class: "pam-page-icon-button",
                                title: tr(locale, "close_menu"),
                                aria_label: tr(locale, "close_menu"),
                                onclick: move |_| more_open.set(false),
                                {icon(LdX)}
                            }
                        }
                        div { class: "pam-action-sheet-groups",
                            for group in action_groups {
                                ActionGroup {
                                    key: "sheet-{group}",
                                    id: group.to_string(),
                                    {action_group_content(group)}
                                }
                            }
                        }
                    }
                }
            }
            if settings_mounted_snapshot {
                div {
                    class: if settings_closing_snapshot { "pam-settings-layer closing" } else { "pam-settings-layer" },
                    tabindex: "-1",
                    onmounted: move |event| {
                        spawn(async move {
                            let _ = event.set_focus(true).await;
                        });
                    },
                    onkeydown: move |event| {
                        if event.key() == Key::Escape {
                            event.prevent_default();
                            close_settings_dialog(settings_mounted, settings_closing, logs_open);
                        }
                    },
                    button {
                        r#type: "button",
                        class: "pam-settings-backdrop",
                        aria_label: tr(locale, "close_menu"),
                        onclick: move |_| {
                            close_settings_dialog(settings_mounted, settings_closing, logs_open)
                        },
                    }
                    section {
                        class: "pam-settings-dialog",
                        role: "dialog",
                        aria_modal: "true",
                        aria_label: tr(locale, "settings"),
                        header { class: "pam-settings-header",
                            div { class: "pam-settings-title-row",
                                span { class: "pam-settings-title-icon", {icon(LdSettings)} }
                                h2 { {tr(locale, "settings")} }
                            }
                            button {
                                r#type: "button",
                                class: "pam-settings-close",
                                title: tr(locale, "close_menu"),
                                aria_label: tr(locale, "close_menu"),
                                onclick: move |_| {
                                    close_settings_dialog(
                                        settings_mounted,
                                        settings_closing,
                                        logs_open,
                                    )
                                },
                                {icon(LdX)}
                            }
                        }
                        div { class: "pam-settings-content",
                            PreferenceGroup {
                                on_open_logs: move |_| {
                                    settings_closing.set(false);
                                    settings_mounted.set(true);
                                    logs_open.set(true);
                                }
                            }
                        }
                    }
                }
            }
            if *logs_open.read() {
                LogViewerDialog {
                    on_close: move |_| {
                        logs_open.set(false);
                        settings_closing.set(false);
                        settings_mounted.set(true);
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
            class: "pam-button quiet",
            disabled,
            title: tr(locale, "clear"),
            onclick: move |_| clear_tabs(context),
            {icon(LdX)}
            span { {tr(locale, "clear")} }
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
        "pam-button primary"
    };
    rsx! {
        label { class,
            title: tr(locale, "load"),
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
            span { {tr(locale, "load")} }
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
        "pam-button primary"
    };
    rsx! {
        button {
            r#type: "button",
            class,
            title: tr(locale, "load"),
            onclick: move |_| {
                if let Some(root) = crate::platform::pick_animation_folder() {
                    crate::actions::load_folder(context, root);
                }
            },
            {icon(LdFolderOpen)}
            span { {tr(locale, "load")} }
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
