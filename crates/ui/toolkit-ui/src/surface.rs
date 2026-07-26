use dioxus::prelude::*;

#[component]
pub fn ToolSurface(
    namespace: &'static str,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "ui-tool-surface ui-tool-surface--{namespace} {class}",
            "data-tool": namespace,
            {children}
        }
    }
}
