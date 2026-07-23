use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdDownload, LdScrollText, LdTrash2, LdX};

use crate::i18n::tr;
use crate::state::{AppContext, Status, Tone};

use super::primitives::icon;

const DIALOG_EXIT_MS: u64 = 200;

fn close_dialog(mut closing: Signal<bool>, on_close: EventHandler<()>) {
    if *closing.peek() {
        return;
    }
    closing.set(true);
    spawn(async move {
        crate::platform::sleep_ms(DIALOG_EXIT_MS).await;
        on_close.call(());
    });
}

#[component]
pub(super) fn LogViewerDialog(on_close: EventHandler<()>) -> Element {
    let mut context = use_context::<AppContext>();
    let locale = context.preferences.read().locale;
    let closing = use_signal(|| false);
    let closing_snapshot = *closing.read();
    let mut logs = use_signal(crate::platform::log_buffer::snapshot);
    let log_text = logs.read().clone();
    let title = tr(locale, "settings_logs");

    use_future(move || async move {
        loop {
            crate::platform::sleep_ms(500).await;
            let next = crate::platform::log_buffer::snapshot();
            if logs.peek().as_str() != next.as_str() {
                logs.set(next);
            }
        }
    });

    rsx! {
        div {
            class: if closing_snapshot { "pam-logs-backdrop closing" } else { "pam-logs-backdrop" },
            tabindex: "-1",
            onmounted: move |event| {
                spawn(async move {
                    let _ = event.set_focus(true).await;
                });
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    close_dialog(closing, on_close);
                }
            },
            onclick: move |_| close_dialog(closing, on_close),
            section {
                class: "pam-logs-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{title}",
                onclick: move |event| event.stop_propagation(),
                header { class: "pam-settings-header",
                    div { class: "pam-settings-title-row",
                        span { class: "pam-settings-title-icon", {icon(LdScrollText)} }
                        h2 { "{title}" }
                    }
                    button {
                        r#type: "button",
                        class: "pam-settings-close",
                        disabled: closing_snapshot,
                        title: tr(locale, "close_menu"),
                        aria_label: tr(locale, "close_menu"),
                        onclick: move |_| close_dialog(closing, on_close),
                        {icon(LdX)}
                    }
                }
                div { class: "pam-logs-content",
                    textarea {
                        class: "pam-log-viewer",
                        readonly: true,
                        value: "{log_text}",
                        spellcheck: "false",
                        wrap: "soft",
                        aria_label: "{title}",
                        placeholder: tr(locale, "logs_empty"),
                        onfocus: move |_| logs.set(crate::platform::log_buffer::snapshot()),
                    }
                }
                footer { class: "pam-logs-actions",
                    button {
                        r#type: "button",
                        class: "pam-button secondary",
                        disabled: closing_snapshot,
                        onclick: move |_| {
                            crate::platform::log_buffer::clear();
                            logs.set(String::new());
                            context.status.set(Status::new(tr(locale, "logs_cleared"), Tone::Ok));
                        },
                        {icon(LdTrash2)}
                        span { {tr(locale, "logs_clear")} }
                    }
                    button {
                        r#type: "button",
                        class: "pam-button primary",
                        disabled: closing_snapshot,
                        onclick: move |_| {
                            let snapshot = crate::platform::log_buffer::snapshot();
                            match crate::platform::save_bytes("pam-viewer.log", snapshot.as_bytes()) {
                                Ok(true) => context.status.set(Status::new(tr(locale, "logs_exported"), Tone::Ok)),
                                Ok(false) => context.status.set(Status::new(tr(locale, "export_cancelled"), Tone::Neutral)),
                                Err(error) => context.set_status(Status::new(error, Tone::Error)),
                            }
                        },
                        {icon(LdDownload)}
                        span { {tr(locale, "logs_export")} }
                    }
                }
            }
        }
    }
}
