use dioxus::prelude::*;

#[component]
pub fn ToolPage(
    namespace: &'static str,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "ui-tool-page ui-tool-page--{namespace} {class}",
            "data-tool-page": namespace,
            {children}
        }
    }
}

#[component]
pub fn ToolPageHeader(
    eyebrow: String,
    title: String,
    description: String,
    actions: Element,
    #[props(default)] class: String,
) -> Element {
    rsx! {
        section { class: "ui-island ui-tool-page-header {class}",
            div { class: "ui-tool-page-copy",
                span { class: "ui-tool-page-eyebrow", "{eyebrow}" }
                div { class: "ui-tool-page-title-row",
                    h1 { "{title}" }
                    span { class: "ui-tool-page-pulse", aria_hidden: "true" }
                }
                p { "{description}" }
            }
            div { class: "ui-tool-page-actions", {actions} }
        }
    }
}

#[component]
pub fn WorkspaceCard(
    #[props(default)] class: String,
    #[props(default)] aria_label: Option<String>,
    children: Element,
) -> Element {
    rsx! {
        article {
            class: "ui-workspace-card {class}",
            aria_label,
            {children}
        }
    }
}

#[component]
pub fn ContextSheet(
    open: bool,
    title: String,
    close_label: String,
    on_close: EventHandler<()>,
    #[props(default = "right".to_string())] side: String,
    #[props(default)] class: String,
    children: Element,
) -> Element {
    if !open {
        return rsx! {};
    }

    rsx! {
        div { class: "ui-context-sheet-layer ui-context-sheet-layer--{side} {class}",
            button {
                r#type: "button",
                class: "ui-context-sheet-backdrop",
                aria_label: "{close_label}",
                onclick: move |_| on_close.call(()),
            }
            aside {
                class: "ui-context-sheet",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{title}",
                div { class: "ui-context-sheet-grabber", aria_hidden: "true" }
                {children}
            }
        }
    }
}

#[component]
pub fn InlineNotice(
    #[props(default)] class: String,
    #[props(default = "neutral".to_string())] tone: String,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "ui-inline-notice ui-inline-notice--{tone} {class}",
            role: "status",
            {children}
        }
    }
}
