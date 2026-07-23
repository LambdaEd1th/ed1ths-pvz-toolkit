use dioxus::prelude::*;
use pam_viewer_core::SpriteKey;

use crate::actions::{activate_sprite, select_label};
use crate::i18n::tr;
use crate::state::AppContext;

use super::super::primitives::{SelectControl, SelectOption};
use super::FieldLabel;

#[component]
pub(super) fn SelectorGroup() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let Some(tab) = context.active_tab_snapshot() else {
        return rsx! {
            FieldLabel { text: tr(locale, "sprite"), SelectControl {
                value: String::new(), options: vec![], disabled: true, onchange: move |_| {}
            } }
            FieldLabel { text: tr(locale, "label"), SelectControl {
                value: String::new(), options: vec![], disabled: true, onchange: move |_| {}
            } }
        };
    };

    let mut sprites = Vec::new();
    if tab.document.pam.main_sprite.is_some() {
        sprites.push(SelectOption::new("main", "MainSprite"));
    }
    sprites.extend(
        tab.document
            .pam
            .sprite
            .iter()
            .enumerate()
            .map(|(index, sprite)| {
                SelectOption::new(
                    index.to_string(),
                    sprite
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("sprite_{index}")),
                )
            }),
    );
    let sprite_value = match tab.active_sprite {
        SpriteKey::Main => "main".to_string(),
        SpriteKey::Sprite(index) => index.to_string(),
    };
    let mut labels = vec![SelectOption::new("all", tr(locale, "all_frames"))];
    labels.extend(
        tab.labels()
            .iter()
            .enumerate()
            .map(|(index, label)| SelectOption::new(index.to_string(), label.name.clone())),
    );
    let label_value = tab
        .selected_label
        .map(|index| index.to_string())
        .unwrap_or_else(|| "all".into());

    rsx! {
        FieldLabel { text: tr(locale, "sprite"),
            SelectControl {
                value: sprite_value,
                options: sprites,
                onchange: move |value: String| {
                    let key = if value == "main" {
                        Some(SpriteKey::Main)
                    } else {
                        value.parse::<usize>().ok().map(SpriteKey::Sprite)
                    };
                    if let Some(key) = key { activate_sprite(context, key); }
                },
            }
        }
        FieldLabel { text: tr(locale, "label"),
            SelectControl {
                value: label_value,
                options: labels,
                onchange: move |value: String| {
                    select_label(context, value.parse::<usize>().ok());
                },
            }
        }
    }
}
