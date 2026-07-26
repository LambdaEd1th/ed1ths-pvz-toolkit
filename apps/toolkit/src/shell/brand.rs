use dioxus::prelude::*;

#[component]
pub(crate) fn BrandLabel(compact: bool) -> Element {
    rsx! {
        div { class: if compact { "tk-brand tk-brand--compact" } else { "tk-brand" },
            div { class: "tk-brand-copy",
                strong { "Ed1th's PvZ Toolkit" }
                if !compact {
                    span { "Resource workspace" }
                }
            }
        }
    }
}
