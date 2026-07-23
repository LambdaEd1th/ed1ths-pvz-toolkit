use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdRedo2, LdUndo2};

use crate::components::{button_class, lucide_icon};
use crate::i18n::I18n;

#[component]
pub(super) fn EditToolbarGroup(
    i18n: I18n,
    can_undo_snapshot: bool,
    can_redo_snapshot: bool,
    undo_edit: EventHandler<()>,
    redo_edit: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "rton-toolbar-group",
            button {
                class: button_class("secondary"),
                disabled: !can_undo_snapshot,
                title: i18n.t("toolbar-undo"),
                onclick: move |_| undo_edit.call(()),
                span { class: "button-icon", {lucide_icon(LdUndo2)} }
                {i18n.t("toolbar-undo")}
            }
            button {
                class: button_class("secondary"),
                disabled: !can_redo_snapshot,
                title: i18n.t("toolbar-redo"),
                onclick: move |_| redo_edit.call(()),
                span { class: "button-icon", {lucide_icon(LdRedo2)} }
                {i18n.t("toolbar-redo")}
            }
        }
    }
}
