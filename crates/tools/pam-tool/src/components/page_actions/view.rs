use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::LdRotateCcw;

use crate::actions::{reset_view, set_export_dimension, set_export_scale};
use crate::i18n::tr;
use crate::state::AppContext;

use super::super::primitives::{NumberControl, SelectControl, SelectOption, icon};

#[component]
pub(super) fn ViewGroup() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    rsx! {
        button { r#type: "button", class: "pam-button", onclick: move |_| reset_view(context),
            {icon(LdRotateCcw)} span { {tr(locale, "reset_view")} }
        }
    }
}

#[component]
pub(super) fn SizeGroup() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let tab = context.active_tab_snapshot();
    let disabled = tab.is_none();
    let size = tab.as_ref().map(|tab| tab.export_size).unwrap_or([0, 0]);
    let scale = tab
        .as_ref()
        .and_then(|tab| tab.export_scale)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "custom".into());
    let scales = [
        ("custom", tr(locale, "custom")),
        ("1", "1x"),
        ("2", "2x"),
        ("3", "3x"),
        ("4", "4x"),
    ]
    .into_iter()
    .map(|(value, label)| SelectOption::new(value, label))
    .collect();
    rsx! {
        span { class: "pam-field-label",
            span { {tr(locale, "export_size")} }
            NumberControl { value: size[0], min: 1, max: 99_999, disabled, onchange: move |value| set_export_dimension(context, 0, value) }
            span { class: "pam-range-separator", "x" }
            NumberControl { value: size[1], min: 1, max: 99_999, disabled, onchange: move |value| set_export_dimension(context, 1, value) }
            SelectControl {
                value: scale, options: scales, compact: true, disabled,
                onchange: move |value: String| set_export_scale(context, value.parse::<u32>().ok()),
            }
        }
    }
}
