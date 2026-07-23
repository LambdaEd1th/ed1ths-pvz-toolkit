use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{
    LdCheckCheck, LdImage, LdPlay, LdRotateCcw, LdShapes, LdSquare,
};
use pam_viewer_core::SpriteKey;
use regex::{Regex, RegexBuilder};

use crate::actions::{
    activate_sprite, restore_default_sprite_visibility, set_all_images_visible,
    set_all_sprites_visible, set_image_visible, set_sprite_visible,
};
use crate::i18n::tr;
use crate::state::{AppContext, PanelResize, PanelSide, ViewerTab};

use super::primitives::icon;

fn filter_regex(value: &str) -> Result<Option<Regex>, ()> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    RegexBuilder::new(value.trim())
        .case_insensitive(true)
        .build()
        .map(Some)
        .map_err(|_| ())
}

#[component]
pub fn ImagePanel() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let Some(tab) = context.active_tab_snapshot() else {
        return rsx! {};
    };
    let regex = filter_regex(&tab.image_regex);
    let invalid = regex.is_err();
    let query = regex.ok().flatten();

    rsx! {
        aside { class: "pam-side-panel pam-images-panel",
            PanelHeader {
                title: tr(locale, "images").to_string(),
                actions: rsx! {
                    button {
                        r#type: "button",
                        class: "pam-button compact",
                        title: tr(locale, "select_all"),
                        onclick: move |_| set_all_images_visible(context, true),
                        {icon(LdCheckCheck)}
                        span { {tr(locale, "select_all")} }
                    }
                    button {
                        r#type: "button",
                        class: "pam-button compact",
                        title: tr(locale, "select_none"),
                        onclick: move |_| set_all_images_visible(context, false),
                        {icon(LdSquare)}
                        span { {tr(locale, "select_none")} }
                    }
                },
                {icon(LdImage)}
            }
            input {
                class: if invalid { "pam-filter-input invalid" } else { "pam-filter-input" },
                r#type: "text",
                value: "{tab.image_regex}",
                placeholder: tr(locale, "regex"),
                oninput: move |event| {
                    let value = event.value();
                    context.update_active_tab(|tab| tab.image_regex = value);
                },
            }
            ul { class: "pam-filter-list",
                for (index, definition) in tab.document.pam.image.iter().enumerate() {
                    if query.as_ref().is_none_or(|regex| regex.is_match(&definition.name)) {
                        {
                            let checked = tab.image_filter.get(index).copied().unwrap_or(true);
                            let thumbnail = tab.image_thumbnails.get(index).cloned().flatten();
                            let dimensions = definition
                                .size
                                .map(|size| format!("{}x{}", size[0], size[1]))
                                .unwrap_or_default();
                            rsx! {
                                li { key: "image-{index}", class: "pam-filter-row",
                                    div { class: "pam-item-thumb",
                                        if let Some(source) = thumbnail {
                                            img { src: "{source}", alt: "" }
                                        } else {
                                            {icon(LdImage)}
                                        }
                                    }
                                    span {
                                        class: "pam-item-name",
                                        title: "{definition.name}",
                                            "{pam_viewer_core::parse_image_file_name(&definition.name)}"
                                    }
                                    span { class: "pam-item-meta", "{dimensions}" }
                                    FilterCheckbox {
                                        label: definition.name.clone(),
                                        checked,
                                        onchange: move |value| set_image_visible(context, index, value),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn SpritePanel() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let Some(tab) = context.active_tab_snapshot() else {
        return rsx! {};
    };
    let regex = filter_regex(&tab.sprite_regex);
    let invalid = regex.is_err();
    let query = regex.ok().flatten();

    rsx! {
        aside { class: "pam-side-panel pam-sprites-panel",
            PanelHeader {
                title: tr(locale, "sprites").to_string(),
                actions: rsx! {
                    button {
                        r#type: "button",
                        class: "pam-button compact",
                        title: tr(locale, "select_all"),
                        onclick: move |_| set_all_sprites_visible(context, true),
                        {icon(LdCheckCheck)}
                        span { {tr(locale, "select_all")} }
                    }
                    button {
                        r#type: "button",
                        class: "pam-button compact",
                        title: tr(locale, "select_none"),
                        onclick: move |_| set_all_sprites_visible(context, false),
                        {icon(LdSquare)}
                        span { {tr(locale, "select_none")} }
                    }
                    button {
                        r#type: "button",
                        class: "pam-button compact",
                        title: tr(locale, "default"),
                        onclick: move |_| restore_default_sprite_visibility(context),
                        {icon(LdRotateCcw)}
                        span { {tr(locale, "default")} }
                    }
                },
                {icon(LdShapes)}
            }
            input {
                class: if invalid { "pam-filter-input invalid" } else { "pam-filter-input" },
                r#type: "text",
                value: "{tab.sprite_regex}",
                placeholder: tr(locale, "regex"),
                oninput: move |event| {
                    let value = event.value();
                    context.update_active_tab(|tab| tab.sprite_regex = value);
                },
            }
            ul { class: "pam-filter-list",
                for (index, sprite) in tab.document.pam.sprite.iter().enumerate() {
                    {
                        let name = sprite.name.clone().unwrap_or_else(|| format!("sprite_{index}"));
                        let visible_by_query = query.as_ref().is_none_or(|regex| regex.is_match(&name));
                        if visible_by_query {
                            let checked = tab.sprite_filter.get(index).copied().unwrap_or(true);
                            let active = tab.active_sprite == SpriteKey::Sprite(index);
                            let thumbnail = sprite_thumbnail(&tab, index);
                            let row_class = if active { "pam-filter-row active" } else { "pam-filter-row" };
                            rsx! {
                                li { key: "sprite-{index}", class: "{row_class}",
                                    div { class: "pam-item-thumb",
                                        if let Some(source) = thumbnail {
                                            img { src: "{source}", alt: "" }
                                        } else {
                                            {icon(LdShapes)}
                                        }
                                    }
                                    span { class: "pam-item-name", title: "{name}", "{name}" }
                                    span { class: "pam-item-meta", "{sprite.frame.len()}f" }
                                    FilterCheckbox {
                                        label: name.clone(),
                                        checked,
                                        onchange: move |value| set_sprite_visible(context, index, value),
                                    }
                                    button {
                                        r#type: "button",
                                        class: "pam-activate-button",
                                        title: tr(locale, "activate"),
                                        onclick: move |_| activate_sprite(context, SpriteKey::Sprite(index)),
                                        {icon(LdPlay)}
                                    }
                                }
                            }
                        } else {
                            rsx! {}
                        }
                    }
                }
                if let Some(main_sprite) = tab.document.pam.main_sprite.as_ref() {
                    if query.as_ref().is_none_or(|regex| regex.is_match("MainSprite")) {
                        li {
                            class: if tab.active_sprite == SpriteKey::Main { "pam-filter-row active" } else { "pam-filter-row" },
                            div { class: "pam-item-thumb", {icon(LdShapes)} }
                            span { class: "pam-item-name", "MainSprite" }
                            span { class: "pam-item-meta", "{main_sprite.frame.len()}f" }
                            span { class: "pam-filter-checkbox-placeholder" }
                            button {
                                r#type: "button",
                                class: "pam-activate-button",
                                title: tr(locale, "activate"),
                                onclick: move |_| activate_sprite(context, SpriteKey::Main),
                                {icon(LdPlay)}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FilterCheckbox(label: String, checked: bool, onchange: EventHandler<bool>) -> Element {
    rsx! {
        label { class: "pam-filter-checkbox", title: "{label}",
            input {
                r#type: "checkbox",
                checked,
                aria_label: "{label}",
                onchange: move |event| onchange.call(event.checked()),
            }
        }
    }
}

#[component]
fn PanelHeader(title: String, actions: Element, children: Element) -> Element {
    rsx! {
        header { class: "pam-panel-header",
            div { class: "pam-panel-header-top",
                div { class: "pam-panel-title",
                    {children}
                    h2 { "{title}" }
                }
                div { class: "pam-panel-actions", {actions} }
            }
        }
    }
}

#[component]
pub fn PanelResizeHandle(side: PanelSide) -> Element {
    let mut context = use_context::<AppContext>();
    let width = match side {
        PanelSide::Images => context.preferences.read().image_panel_width,
        PanelSide::Sprites => context.preferences.read().sprite_panel_width,
    };
    rsx! {
        div {
            class: "pam-panel-resize-handle",
            role: "separator",
            aria_orientation: "vertical",
            onmousedown: move |event| {
                event.prevent_default();
                context.panel_resize.set(Some(PanelResize {
                    side,
                    start_x: event.client_coordinates().x,
                    start_width: width,
                }));
            },
        }
    }
}

fn sprite_thumbnail(tab: &ViewerTab, index: usize) -> Option<String> {
    let sprite = tab.document.pam.sprite.get(index)?;
    if sprite.frame.len() != 1 {
        return None;
    }
    let image_index = sprite.frame[0]
        .append
        .iter()
        .find(|append| !append.sprite)
        .map(|append| append.resource as usize)?;
    tab.image_thumbnails.get(image_index).cloned().flatten()
}
