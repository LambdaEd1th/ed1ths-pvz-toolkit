use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdPlus, LdX};

use crate::actions::{activate_tab, close_tab, reorder_tab};
use crate::i18n::tr;
use crate::state::AppContext;

use super::primitives::icon;

#[component]
pub fn TabStrip() -> Element {
    let mut context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let active = *context.active_tab.read();
    let dragged = *context.dragged_tab.read();
    let tabs = context.tabs.read().clone();

    rsx! {
        nav { class: "pam-tab-strip",
            div {
                class: "pam-tab-viewport",
                role: "tablist",
                aria_label: tr(locale, "animations"),
                for tab in tabs {
                    {
                        let id = tab.id;
                        let name = tab.display_name();
                        let class_name = match (active == Some(id), dragged == Some(id)) {
                            (true, true) => "pam-tab active dragging",
                            (true, false) => "pam-tab active",
                            (false, true) => "pam-tab dragging",
                            (false, false) => "pam-tab",
                        };
                        rsx! {
                            div {
                                key: "{id}",
                                class: "{class_name}",
                                onmousedown: move |event| {
                                    event.prevent_default();
                                    context.dragged_tab.set(Some(id));
                                },
                                onmouseenter: move |_| {
                                    if let Some(source) = *context.dragged_tab.read() {
                                        reorder_tab(context, source, id);
                                    }
                                },
                                button {
                                    r#type: "button",
                                    class: "pam-tab-label",
                                    role: "tab",
                                    aria_selected: active == Some(id),
                                    title: "{name}",
                                    onclick: move |_| activate_tab(context, id),
                                    span { "{name}" }
                                }
                                button {
                                    r#type: "button",
                                    class: "pam-tab-close",
                                    title: tr(locale, "close_tab"),
                                    aria_label: tr(locale, "close_tab"),
                                    onmousedown: move |event| event.stop_propagation(),
                                    onclick: move |event| {
                                        event.stop_propagation();
                                        close_tab(context, id);
                                    },
                                    {icon(LdX)}
                                }
                            }
                        }
                    }
                }
                NewTabButton {}
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn NewTabButton() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    rsx! {
        label {
            class: "pam-new-tab",
            title: tr(locale, "load"),
            input {
                class: "pam-file-input",
                r#type: "file",
                multiple: true,
                directory: true,
                onchange: move |event| async move {
                    match crate::actions::input_files_from_dioxus(event.files()).await {
                        Ok(files) if !files.is_empty() => crate::actions::load_inputs(context, files),
                        Ok(_) => {}
                        Err(error) => context.set_status(crate::state::Status::new(
                            error,
                            crate::state::Tone::Error,
                        )),
                    }
                }
            }
            {icon(LdPlus)}
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn NewTabButton() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    rsx! {
        button {
            r#type: "button",
            class: "pam-new-tab",
            title: tr(locale, "load"),
            onclick: move |_| {
                if let Some(root) = crate::platform::pick_animation_folder() {
                    crate::actions::load_folder(context, root);
                }
            },
            {icon(LdPlus)}
        }
    }
}
