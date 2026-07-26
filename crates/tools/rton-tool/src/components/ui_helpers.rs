use dioxus::prelude::*;
use dioxus_free_icons::{Icon, IconShape};

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
