use dioxus::prelude::*;
use serde_json::json;

/// Generic chrome labels shared by every tool drawer. The application shell
/// provides a localized value; standalone tool mounts keep the Chinese
/// fallbacks used before localization was wired through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolbarLabels {
    pub label: String,
    pub open_label: String,
    pub close_label: String,
}

impl Default for ToolbarLabels {
    fn default() -> Self {
        Self {
            label: "工具".to_string(),
            open_label: "展开工具栏".to_string(),
            close_label: "收起工具栏".to_string(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct ToolbarLabelsContext {
    labels: Signal<ToolbarLabels>,
}

pub fn use_toolbar_labels() -> ToolbarLabels {
    try_consume_context::<ToolbarLabelsContext>()
        .map(|context| (context.labels)())
        .unwrap_or_default()
}

pub fn use_provide_toolbar_labels(labels: Signal<ToolbarLabels>) {
    use_context_provider(|| ToolbarLabelsContext { labels });
}

/// Contain focus inside an open overlay. The layer element must exist while
/// `active` is true; its focusable content is `[data-modal-content]` when
/// present, otherwise the layer itself. Background siblings are made inert
/// and the previously focused element is restored on deactivation.
///
/// `modal_selector` limits activation to layouts where a marker element is
/// actually interactive (used by the responsive tool drawer). `exclude_selector`
/// keeps a sibling click target such as the compact sidebar backdrop usable.
pub fn use_modal_focus(
    id: &str,
    active: bool,
    selector: &str,
    modal_selector: Option<&str>,
    exclude_selector: Option<&str>,
) {
    let id = id.to_string();
    let selector = selector.to_string();
    let modal_selector = modal_selector.map(str::to_string);
    let exclude_selector = exclude_selector.map(str::to_string);

    let effect_id = id.clone();
    use_effect(use_reactive(&active, move |active| {
        let script = if active {
            let options = json!({
                "id": effect_id.clone(),
                "selector": selector.clone(),
                "modalSelector": modal_selector.clone(),
                "excludeSelector": exclude_selector.clone(),
            });
            format!("window.toolkitModalFocus?.activate({options});")
        } else {
            let id = json!(&effect_id);
            format!("window.toolkitModalFocus?.deactivate({id});")
        };
        let _ = document::eval(&script);
    }));

    let drop_id = id;
    use_drop(move || {
        let id = json!(&drop_id);
        let _ = document::eval(&format!("window.toolkitModalFocus?.deactivate({id});"));
    });
}
