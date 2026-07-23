use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdFilePlus2, LdFileUp, LdFolderOpen};

use crate::app_constants::LOADABLE_FILE_ACCEPT;
use crate::components::{FileSelection, button_class, lucide_icon};
use crate::domain::{Status, Tone, is_loadable_display_name};
use crate::file_import::{
    LoadedFileState, file_data_display_name, loaded_file_draft_from_file_data,
    stage_loaded_file_drafts,
};
use crate::i18n::I18n;

#[component]
pub(crate) fn WebFileOpenControl(
    i18n: I18n,
    class_name: String,
    compact: bool,
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    mut status: Signal<Status>,
    on_files_staged: EventHandler<()>,
) -> Element {
    rsx! {
        label {
            class: "{class_name}",
            title: i18n.t("toolbar-open"),
            aria_label: i18n.t("toolbar-open"),
            input {
                class: "file-input",
                r#type: "file",
                multiple: true,
                accept: LOADABLE_FILE_ACCEPT,
                onchange: move |event| async move {
                    let files = event.files();
                    if files.is_empty() {
                        return;
                    }

                    let mut drafts = Vec::new();
                    for file in files {
                        let name = file.name();
                        match loaded_file_draft_from_file_data(name.clone(), file).await {
                            Ok(draft) => drafts.push(draft),
                            Err(error) => status.set(Status::new(
                                i18n.t_args(
                                    "status-file-read-error",
                                    &[("name", name), ("error", error)],
                                ),
                                Tone::Error,
                            )),
                        }
                    }

                    if drafts.is_empty() {
                        status.set(Status::new(i18n.t("status-no-loadable-files"), Tone::Warn));
                    } else {
                        let indexed = stage_loaded_file_drafts(
                            loaded_files,
                            next_loaded_file_id,
                            file_selection,
                            drafts,
                        );
                        status.set(Status::new(
                            i18n.t_args("status-indexed-files", &[("count", indexed.to_string())]),
                            Tone::Ok,
                        ));
                        on_files_staged.call(());
                    }
                }
            }
            if compact {
                {lucide_icon(LdFilePlus2)}
            } else {
                span { class: "button-icon", {lucide_icon(LdFileUp)} }
                span { {i18n.t("toolbar-open")} }
            }
        }
    }
}

#[component]
pub(super) fn FileToolbarGroup(
    i18n: I18n,
    active_file_label: String,
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    mut status: Signal<Status>,
    open_native_files: EventHandler<()>,
    open_native_folder: EventHandler<()>,
    on_files_staged: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "rton-toolbar-group",
            if cfg!(target_arch = "wasm32") {
                WebFileOpenControl {
                    i18n,
                    class_name: button_class("primary"),
                    compact: false,
                    loaded_files,
                    next_loaded_file_id,
                    file_selection,
                    status,
                    on_files_staged
                }
                label { class: button_class("secondary"),
                    input {
                        class: "file-input",
                        r#type: "file",
                        multiple: true,
                        directory: true,
                        accept: LOADABLE_FILE_ACCEPT,
                        onchange: move |event| async move {
                            let files = event.files();
                            if files.is_empty() {
                                return;
                            }

                            let mut drafts = Vec::new();
                            let mut skipped = 0usize;
                            for file in files {
                                let name = file_data_display_name(&file);
                                if !is_loadable_display_name(&name) {
                                    skipped += 1;
                                    continue;
                                }
                                match loaded_file_draft_from_file_data(name.clone(), file).await {
                                    Ok(draft) => drafts.push(draft),
                                    Err(error) => status.set(Status::new(
                                        i18n.t_args(
                                            "status-file-read-error",
                                            &[("name", name), ("error", error)],
                                        ),
                                        Tone::Error,
                                    )),
                                }
                            }

                            if drafts.is_empty() {
                                if skipped > 0 {
                                    status.set(Status::new(i18n.t("status-no-loadable-files"), Tone::Warn));
                                }
                            } else {
                                let indexed = stage_loaded_file_drafts(
                                    loaded_files,
                                    next_loaded_file_id,
                                    file_selection,
                                    drafts,
                                );
                                status.set(Status::new(
                                    i18n.t_args("status-indexed-files", &[("count", indexed.to_string())]),
                                    Tone::Ok,
                                ));
                            }
                        }
                    }
                    span { class: "button-icon", {lucide_icon(LdFolderOpen)} }
                    span { {i18n.t("toolbar-folder")} }
                }
            } else {
                button {
                    class: button_class("primary"),
                    onclick: move |_| open_native_files.call(()),
                    span { class: "button-icon", {lucide_icon(LdFileUp)} }
                    span { {i18n.t("toolbar-open")} }
                }
                button {
                    class: button_class("secondary"),
                    onclick: move |_| open_native_folder.call(()),
                    span { class: "button-icon", {lucide_icon(LdFolderOpen)} }
                    {i18n.t("toolbar-folder")}
                }
            }
            span { class: "current-file",
                "{active_file_label}"
            }
        }
    }
}
