use dioxus::prelude::*;

#[component]
pub(crate) fn BrandMark(compact: bool) -> Element {
    rsx! {
        div { class: if compact { "tk-brand tk-brand--compact" } else { "tk-brand" },
            div { class: "tk-brand-mark", aria_hidden: "true",
                svg { view_box: "0 0 64 64",
                    path {
                        d: "M15 36C6 23 14 8 28 17C30 7 45 5 46 20C58 20 61 35 49 40C51 53 35 60 28 49C18 58 5 47 15 36Z",
                        fill: "currentColor",
                    }
                    path {
                        d: "M22 40C29 37 35 32 42 23M31 34C29 28 26 25 22 22M36 30C40 31 44 32 48 31",
                        fill: "none",
                        stroke: "white",
                        stroke_width: "4",
                        stroke_linecap: "round",
                    }
                }
            }
            div { class: "tk-brand-copy",
                strong { "Ed1th's PvZ Toolkit" }
                if !compact {
                    span { "Resource workspace" }
                }
            }
        }
    }
}
