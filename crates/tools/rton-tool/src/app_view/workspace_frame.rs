use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::LdFileUp;

use crate::components::lucide_icon;
use crate::i18n::I18n;

#[component]
pub(super) fn WorkspaceFrame(
    i18n: I18n,
    dragging_files: Signal<bool>,
    file_drawer_open: Signal<bool>,
    inspector_drawer_open: Signal<bool>,
    workspace_style: String,
    children: Element,
) -> Element {
    let dragging_snapshot = *dragging_files.read();
    let file_drawer_open_snapshot = *file_drawer_open.read();
    let inspector_drawer_open_snapshot = *inspector_drawer_open.read();
    let workspace_class = format!(
        "rton-workspace-shell {} {} {} {} {}",
        if dragging_snapshot {
            "dragging-files"
        } else {
            ""
        },
        if file_drawer_open_snapshot {
            "file-drawer-mounted"
        } else {
            "file-drawer-unmounted"
        },
        if file_drawer_open_snapshot {
            "file-drawer-open"
        } else {
            "file-drawer-closed"
        },
        if inspector_drawer_open_snapshot {
            "inspector-drawer-mounted"
        } else {
            "inspector-drawer-unmounted"
        },
        if inspector_drawer_open_snapshot {
            "inspector-drawer-open"
        } else {
            "inspector-drawer-closed"
        },
    );

    rsx! {
        div {
            class: "{workspace_class}",
            style: "{workspace_style}",
            if dragging_snapshot {
                div {
                    class: "rton-workspace-drop-indicator",
                    aria_hidden: "true",
                    span { class: "rton-workspace-drop-indicator-icon", {lucide_icon(LdFileUp)} }
                    strong { {i18n.t("drop-title")} }
                }
            }
            if file_drawer_open_snapshot || inspector_drawer_open_snapshot {
                div {
                    class: "rton-file-drawer-backdrop",
                    aria_hidden: "true"
                }
            }
            {children}
        }
    }
}
