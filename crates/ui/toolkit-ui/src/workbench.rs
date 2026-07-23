use dioxus::prelude::*;

#[component]
pub fn WorkbenchSurface(
    namespace: &'static str,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "ui-workbench ui-workbench--{namespace} {class}",
            "data-tool": namespace,
            {children}
        }
    }
}

#[component]
pub fn WorkbenchPanel(
    #[props(default)] class: String,
    #[props(default)] aria_label: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        section {
            class: "ui-workbench-panel {class}",
            aria_label,
            {children}
        }
    }
}
