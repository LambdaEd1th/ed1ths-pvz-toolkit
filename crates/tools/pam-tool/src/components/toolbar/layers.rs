use dioxus::prelude::*;

use crate::actions::{select_exclusive_special_layer, set_ground_swatch_visible};
use crate::i18n::tr;
use crate::state::{AppContext, Locale, ViewerTab};

use super::super::primitives::{SelectControl, SelectOption, SwitchControl};
use super::FieldLabel;

#[component]
pub(super) fn LayerGroup() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let Some(tab) = context.active_tab_snapshot() else {
        return rsx! {};
    };
    let plant_indices = tab.special_layers.plant_custom_layers.clone();
    let zombie_indices = tab.special_layers.zombie_state_layers.clone();
    let ground_indices = tab.special_layers.ground_swatch_layers.clone();
    let plant_options = special_layer_options(&tab, &plant_indices, locale);
    let zombie_options = special_layer_options(&tab, &zombie_indices, locale);
    let plant_value = selected_special_layer(&tab, &plant_indices);
    let zombie_value = selected_special_layer(&tab, &zombie_indices);
    let ground_checked = ground_indices
        .iter()
        .any(|index| tab.sprite_filter.get(*index).copied().unwrap_or(false));
    rsx! {
        FieldLabel { text: tr(locale, "plant_layer"),
            SelectControl {
                value: plant_value,
                options: plant_options,
                disabled: plant_indices.is_empty(),
                onchange: move |value: String| {
                    select_exclusive_special_layer(
                        context,
                        plant_indices.clone(),
                        value.parse::<usize>().ok(),
                    );
                },
            }
        }
        FieldLabel { text: tr(locale, "zombie_state"),
            SelectControl {
                value: zombie_value,
                options: zombie_options,
                disabled: zombie_indices.is_empty(),
                onchange: move |value: String| {
                    select_exclusive_special_layer(
                        context,
                        zombie_indices.clone(),
                        value.parse::<usize>().ok(),
                    );
                },
            }
        }
        SwitchControl {
            label: tr(locale, "ground_swatch"),
            checked: ground_checked,
            disabled: ground_indices.is_empty(),
            onchange: move |value| set_ground_swatch_visible(context, value),
        }
    }
}

fn special_layer_options(tab: &ViewerTab, indices: &[usize], locale: Locale) -> Vec<SelectOption> {
    let mut options = vec![SelectOption::new("none", tr(locale, "none"))];
    options.extend(indices.iter().filter_map(|index| {
        tab.document.pam.sprite.get(*index).map(|sprite| {
            SelectOption::new(
                index.to_string(),
                sprite
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("sprite_{index}")),
            )
        })
    }));
    options
}

fn selected_special_layer(tab: &ViewerTab, indices: &[usize]) -> String {
    indices
        .iter()
        .find(|index| tab.sprite_filter.get(**index).copied().unwrap_or(false))
        .map(ToString::to_string)
        .unwrap_or_else(|| "none".into())
}
