use dioxus::prelude::*;
use dioxus_free_icons::{Icon, IconShape};

use crate::app_constants::{PANEL_MAX_WIDTH, PANEL_MIN_WIDTH};

pub(crate) fn clamp_panel_width(width: f64) -> i32 {
    width
        .round()
        .clamp(PANEL_MIN_WIDTH as f64, PANEL_MAX_WIDTH as f64) as i32
}

pub(crate) fn lucide_icon<T>(icon: T) -> Element
where
    T: IconShape + Clone + PartialEq + 'static,
{
    rsx! {
        Icon {
            class: "rton-lucide-icon",
            width: 16,
            height: 16,
            fill: "currentColor",
            icon
        }
    }
}

pub(crate) fn button_class(variant: &'static str) -> &'static str {
    match variant {
        "primary" => "rton-button primary",
        _ => "rton-button secondary",
    }
}
