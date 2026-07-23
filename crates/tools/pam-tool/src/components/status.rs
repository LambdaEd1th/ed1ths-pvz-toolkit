use dioxus::prelude::*;

use crate::i18n::tr;
use crate::state::{AppContext, Tone};

#[component]
pub fn StatusBar() -> Element {
    let context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let status = context.status.read().clone();
    let tab = context.active_tab_snapshot();
    let pointer = *context.pointer_coord.read();
    let tone = match status.tone {
        Tone::Neutral => "neutral",
        Tone::Ok => "ok",
        Tone::Warning => "warning",
        Tone::Error => "error",
    };
    let message = if status.message.is_empty() {
        tr(locale, "status_hint")
    } else {
        &status.message
    };
    let pointer_text = pointer.map(|pointer| format!("x {:.1}  y {:.1}", pointer[0], pointer[1]));
    let size_text = tab
        .as_ref()
        .map(|tab| format!("{} x {}", tab.export_size[0], tab.export_size[1]));
    let zoom_text = tab.as_ref().map(|tab| format!("{:.0}%", tab.zoom * 100.0));
    rsx! {
        footer { class: "pam-statusbar",
            span { class: "pam-status-message {tone}", "{message}" }
            span { class: "pam-status-spacer" }
            if let Some(pointer_text) = pointer_text {
                span { class: "pam-status-chip", "{pointer_text}" }
            }
            if let Some(size_text) = size_text {
                span { class: "pam-status-chip", "{size_text}" }
            }
            if let Some(zoom_text) = zoom_text {
                span { class: "pam-status-chip", "{zoom_text}" }
            }
        }
    }
}

#[component]
pub fn ExportOverlay() -> Element {
    let mut context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let Some(progress) = context.export.read().clone() else {
        return rsx! {};
    };
    let progress_style = format!("width:{:.1}%", progress.progress * 100.0);
    rsx! {
        div { class: "pam-modal-backdrop",
            div { class: "pam-export-dialog", role: "dialog", aria_modal: "true",
                div { class: "pam-export-title", "{progress.title}" }
                div { class: "pam-export-detail", "{progress.detail}" }
                div { class: "pam-progress-track",
                    div { class: "pam-progress-fill", style: "{progress_style}" }
                }
                button {
                    r#type: "button",
                    class: "pam-button",
                    onclick: move |_| {
                        if let Some(progress) = context.export.write().as_mut() {
                            progress.cancel_requested = true;
                            progress.detail = tr(locale, "cancel").into();
                            let document_id = progress.document_id;
                            let operation_id = progress.operation_id;
                            spawn(async move {
                                let _ = crate::platform::processing::perform(
                                    pam_viewer_core::WorkerRequest::CancelExport {
                                        document_id,
                                        operation_id,
                                    },
                                )
                                .await;
                            });
                        }
                    },
                    {tr(locale, "cancel")}
                }
            }
        }
    }
}
