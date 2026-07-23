use dioxus::prelude::*;
use dioxus_html::HasFileData;

use crate::components::FileSelection;
use crate::domain::{Status, Tone, is_loadable_display_name};
use crate::file_import::{
    LoadedFileState, dropped_directory_files, file_data_display_name,
    loaded_file_draft_from_file_data, loaded_file_drafts_from_native, stage_loaded_file_drafts,
};
#[cfg(target_arch = "wasm32")]
use crate::file_import::{collect_web_dropped_directory_files, loaded_file_draft_from_web_dropped};
use crate::i18n::I18n;

pub(super) async fn handle_workspace_file_drop(
    event: DragEvent,
    mut dragging_files: Signal<bool>,
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    mut status: Signal<Status>,
    i18n: I18n,
) -> bool {
    event.prevent_default();
    dragging_files.set(false);

    #[cfg(target_arch = "wasm32")]
    {
        match collect_web_dropped_directory_files(&event).await {
            Ok(Some(web_files)) => {
                let drafts = web_files
                    .into_iter()
                    .map(loaded_file_draft_from_web_dropped)
                    .collect::<Vec<_>>();
                if drafts.is_empty() {
                    status.set(Status::new(i18n.t("status-no-loadable-files"), Tone::Warn));
                    return false;
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
                    return indexed > 0;
                }
            }
            Ok(None) => {}
            Err(error) => {
                status.set(Status::new(error, Tone::Error));
                return false;
            }
        }
    }

    let files = event.files();
    if files.is_empty() {
        return false;
    }

    let mut drafts = Vec::new();
    let mut skipped = 0usize;
    for file in files {
        match dropped_directory_files(&file) {
            Ok(Some(folder_files)) => {
                if folder_files.is_empty() {
                    skipped += 1;
                    continue;
                }
                drafts.extend(loaded_file_drafts_from_native(folder_files));
            }
            Ok(None) => {
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
            Err(error) => status.set(Status::new(error, Tone::Error)),
        }
    }

    if drafts.is_empty() {
        if skipped > 0 {
            status.set(Status::new(i18n.t("status-no-loadable-files"), Tone::Warn));
        }
        false
    } else {
        let indexed =
            stage_loaded_file_drafts(loaded_files, next_loaded_file_id, file_selection, drafts);
        status.set(Status::new(
            i18n.t_args("status-indexed-files", &[("count", indexed.to_string())]),
            Tone::Ok,
        ));
        indexed > 0
    }
}

pub(super) fn workspace_drag_has_files(event: &DragEvent) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(web_event) = event.data().downcast::<web_sys::DragEvent>()
            && let Some(data_transfer) = web_event.data_transfer()
        {
            if data_transfer
                .files()
                .is_some_and(|files| files.length() > 0)
            {
                return true;
            }
            let items = data_transfer.items();
            for index in 0..items.length() {
                if items.get(index).is_some_and(|item| item.kind() == "file") {
                    return true;
                }
            }
        }
    }

    !event.files().is_empty()
}
