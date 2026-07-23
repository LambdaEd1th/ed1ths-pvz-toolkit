use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdTriangleAlert, LdX};

use crate::components::lucide_icon;
use crate::i18n::I18n;

const DIALOG_EXIT_DURATION_MS: u64 = 200;

fn close_dialog(mut closing: Signal<bool>, action: EventHandler<()>) {
    if *closing.peek() {
        return;
    }
    closing.set(true);
    spawn(async move {
        crate::platform::sleep_ms(DIALOG_EXIT_DURATION_MS).await;
        action.call(());
    });
}

#[component]
pub(crate) fn UnsavedChangesDialog(
    file_name: String,
    i18n: I18n,
    on_cancel: EventHandler<()>,
    on_discard: EventHandler<()>,
) -> Element {
    let closing = use_signal(|| false);
    let closing_snapshot = *closing.read();
    let title = i18n.t("unsaved-changes-title");
    let close_label = i18n.t("about-close");

    rsx! {
        div {
            class: if closing_snapshot { "rton-unsaved-backdrop closing" } else { "rton-unsaved-backdrop" },
            tabindex: "-1",
            onmounted: move |event| async move {
                let _ = event.set_focus(true).await;
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    close_dialog(closing, on_cancel);
                }
            },
            onclick: move |_| close_dialog(closing, on_cancel),
            section {
                class: "rton-unsaved-dialog",
                role: "alertdialog",
                aria_modal: "true",
                aria_labelledby: "rton-unsaved-title",
                aria_describedby: "rton-unsaved-description",
                onclick: move |event| event.stop_propagation(),
                header { class: "rton-unsaved-header",
                    div { class: "rton-unsaved-title-row",
                        span { class: "rton-unsaved-icon", aria_hidden: "true", {lucide_icon(LdTriangleAlert)} }
                        h2 { id: "rton-unsaved-title", "{title}" }
                    }
                    button {
                        r#type: "button",
                        class: "rton-unsaved-close",
                        disabled: closing_snapshot,
                        title: "{close_label}",
                        aria_label: "{close_label}",
                        onclick: move |_| close_dialog(closing, on_cancel),
                        {lucide_icon(LdX)}
                    }
                }
                div { class: "rton-unsaved-content",
                    p {
                        id: "rton-unsaved-description",
                        {i18n.t_args("unsaved-changes-description", &[("name", file_name)])}
                    }
                }
                footer { class: "rton-unsaved-actions",
                    button {
                        r#type: "button",
                        class: "rton-button secondary",
                        disabled: closing_snapshot,
                        onclick: move |_| close_dialog(closing, on_cancel),
                        {i18n.t("unsaved-changes-cancel")}
                    }
                    button {
                        r#type: "button",
                        class: "rton-button danger",
                        disabled: closing_snapshot,
                        onclick: move |_| close_dialog(closing, on_discard),
                        {i18n.t("unsaved-changes-discard")}
                    }
                }
            }
        }
    }
}
