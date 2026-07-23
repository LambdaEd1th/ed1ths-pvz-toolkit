use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdDownload, LdScrollText, LdTrash2, LdX};

use crate::components::lucide_icon;
use crate::domain::{Status, Tone};
use crate::i18n::I18n;
use crate::platform;

const DIALOG_EXIT_DURATION_MS: u64 = 200;

fn close_dialog(mut closing: Signal<bool>, on_close: EventHandler<()>) {
    if *closing.peek() {
        return;
    }
    closing.set(true);
    spawn(async move {
        platform::sleep_ms(DIALOG_EXIT_DURATION_MS).await;
        on_close.call(());
    });
}

#[component]
pub(crate) fn LogViewerDialog(
    i18n: I18n,
    mut status: Signal<Status>,
    on_close: EventHandler<()>,
) -> Element {
    let closing = use_signal(|| false);
    let closing_snapshot = *closing.read();
    let mut logs = use_signal(platform::log_buffer::snapshot);
    let log_text = logs.read().clone();
    let title = i18n.t("settings-logs");
    let close_label = i18n.t("about-close");

    use_future(move || async move {
        loop {
            platform::sleep_ms(500).await;
            let next = platform::log_buffer::snapshot();
            if logs.peek().as_str() != next.as_str() {
                logs.set(next);
            }
        }
    });

    rsx! {
        div {
            class: if closing_snapshot { "rton-logs-backdrop closing" } else { "rton-logs-backdrop" },
            tabindex: "-1",
            onmounted: move |event| async move {
                let _ = event.set_focus(true).await;
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    close_dialog(closing, on_close);
                }
            },
            onclick: move |_| close_dialog(closing, on_close),
            section {
                class: "rton-logs-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{title}",
                onclick: move |event| event.stop_propagation(),
                header { class: "rton-settings-header",
                    div { class: "rton-settings-title-row",
                        span { class: "rton-settings-title-icon", {lucide_icon(LdScrollText)} }
                        h2 { "{title}" }
                    }
                    button {
                        r#type: "button",
                        class: "rton-settings-close",
                        disabled: closing_snapshot,
                        title: "{close_label}",
                        aria_label: "{close_label}",
                        onclick: move |_| close_dialog(closing, on_close),
                        {lucide_icon(LdX)}
                    }
                }
                div { class: "rton-logs-content",
                    textarea {
                        class: "rton-log-viewer",
                        readonly: true,
                        value: "{log_text}",
                        spellcheck: "false",
                        wrap: "soft",
                        aria_label: "{title}",
                        placeholder: i18n.t("logs-empty"),
                        onfocus: move |_| logs.set(platform::log_buffer::snapshot()),
                    }
                }
                footer { class: "rton-logs-actions",
                    button {
                        r#type: "button",
                        class: "rton-button secondary",
                        disabled: closing_snapshot,
                        onclick: move |_| {
                            platform::log_buffer::clear();
                            logs.set(String::new());
                            status.set(Status::new(i18n.t("status-logs-cleared"), Tone::Info));
                        },
                        span { class: "button-icon", {lucide_icon(LdTrash2)} }
                        span { {i18n.t("logs-clear")} }
                    }
                    button {
                        r#type: "button",
                        class: "rton-button primary",
                        disabled: closing_snapshot,
                        onclick: move |_| {
                            let snapshot = platform::log_buffer::snapshot();
                            match platform::save_text("rton-editor.log", &snapshot) {
                                Ok(true) => status.set(Status::new(
                                    i18n.t("status-logs-exported"),
                                    Tone::Ok,
                                )),
                                Ok(false) => status.set(Status::new(
                                    i18n.t("status-export-cancelled"),
                                    Tone::Info,
                                )),
                                Err(error) => status.set(Status::new(error, Tone::Error)),
                            }
                        },
                        span { class: "button-icon", {lucide_icon(LdDownload)} }
                        span { {i18n.t("logs-export")} }
                    }
                }
            }
        }
    }
}
