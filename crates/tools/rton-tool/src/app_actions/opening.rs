use dioxus::prelude::*;

use rton_editor_core::{
    BinaryEncoding, CoreError, EncodeOptions, RtonValue, encode_rton_bytes, value_to_text,
};

use crate::components::FileSelection;
use crate::domain::{
    ByteDocument, EditorMode, EditorTabState, OpenTabError, Status, Tone,
    create_tab_from_byte_document, create_text_tab,
};
use crate::file_import::{
    LoadedFileState, create_tab_from_loaded_file, loaded_file_drafts_from_native,
    stage_loaded_file_drafts,
};
use crate::i18n::I18n;
use crate::platform;

use super::document::{parse_tab_by_id, switch_active_mode};

pub(crate) fn create_blank_tab_state(
    id: usize,
    mode: EditorMode,
    encode_options: EncodeOptions,
) -> Result<EditorTabState, CoreError> {
    let value = RtonValue::Object(Vec::new());
    let file_name = format!("untitled-{id}.{}", mode.code());

    if let Some(format) = mode.text_format() {
        return create_text_tab(id, file_name, value_to_text(&value, format)?, format);
    }

    let bytes = encode_rton_bytes(
        &value,
        EncodeOptions {
            encrypted: false,
            ..encode_options
        },
    )?;
    create_tab_from_byte_document(id, file_name, ByteDocument::from_vec(bytes))
}

pub(crate) fn open_blank_tab(
    mut next_tab_id: Signal<usize>,
    mut tabs: Signal<Vec<EditorTabState>>,
    mut active_tab_id: Signal<usize>,
    mode: EditorMode,
    encode_options: EncodeOptions,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    let id = *next_tab_id.read();
    next_tab_id.set(id + 1);
    match create_blank_tab_state(id, mode, encode_options) {
        Ok(tab) => {
            let name = tab.file_name.clone();
            tabs.write().push(tab);
            active_tab_id.set(id);
            status.set(Status::new(
                i18n.t_args("status-blank-created", &[("name", name)]),
                Tone::Ok,
            ));
            parse_tab_by_id(id, tabs, status, i18n);
        }
        Err(error) => status.set(Status::new(error.to_string(), Tone::Error)),
    }
}

pub(crate) fn activate_tab_by_id(
    id: usize,
    tabs: Signal<Vec<EditorTabState>>,
    mut active_tab_id: Signal<usize>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    if tabs.read().iter().any(|tab| tab.id == id) {
        active_tab_id.set(id);
        status.set(Status::new(i18n.t("status-tab-activated"), Tone::Info));
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_loaded_file_by_id(
    file_id: usize,
    loaded_files: Signal<Vec<LoadedFileState>>,
    mut next_tab_id: Signal<usize>,
    mut tabs: Signal<Vec<EditorTabState>>,
    mut active_tab_id: Signal<usize>,
    compact_output: Signal<bool>,
    encrypt_output: Signal<bool>,
    preferred_mode: Option<EditorMode>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    spawn(async move {
        let Some(entry) = loaded_files
            .read()
            .iter()
            .find(|file| file.id == file_id)
            .cloned()
        else {
            return;
        };

        if let Some(tab_id) = entry.tab_id
            && tabs.read().iter().any(|tab| tab.id == tab_id)
        {
            active_tab_id.set(tab_id);
            if let Some(tab) = tabs.read().iter().find(|tab| tab.id == tab_id) {
                sync_output_encoding_from_tab(tab, compact_output, encrypt_output);
            }
            status.set(Status::new(i18n.t("status-tab-activated"), Tone::Info));
            return;
        }

        let id = *next_tab_id.read();
        next_tab_id.set(id + 1);
        let name = entry.display_name.clone();
        match create_tab_from_loaded_file(id, &entry).await {
            Ok(tab) => {
                let source_mode = tab.mode;
                let source_options = tab.source_encode_options;
                tabs.write().push(tab);
                crate::file_import::set_loaded_file_tab_id(loaded_files, file_id, Some(id));
                active_tab_id.set(id);
                sync_output_encoding_from_options(source_options, compact_output, encrypt_output);
                status.set(Status::new(
                    i18n.t_args("status-opened-file", &[("name", name)]),
                    Tone::Ok,
                ));
                apply_preferred_mode_or_parse(
                    id,
                    source_mode,
                    preferred_mode,
                    EncodeOptions {
                        encoding: source_options.encoding,
                        encrypted: false,
                    },
                    tabs,
                    active_tab_id,
                    status,
                    i18n,
                );
            }
            Err(OpenTabError::Read(error)) => status.set(Status::new(
                i18n.t_args(
                    "status-file-read-error",
                    &[("name", name), ("error", error)],
                ),
                Tone::Error,
            )),
            Err(error) => status.set(Status::new(
                i18n.t_args(
                    "status-file-error",
                    &[("name", name), ("error", error.message())],
                ),
                Tone::Error,
            )),
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn apply_preferred_mode_or_parse(
    tab_id: usize,
    source_mode: EditorMode,
    preferred_mode: Option<EditorMode>,
    encode_options: EncodeOptions,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    status: Signal<Status>,
    i18n: I18n,
) {
    if let Some(preferred_mode) = preferred_mode
        && preferred_mode != source_mode
    {
        switch_active_mode(
            preferred_mode,
            encode_options,
            tabs,
            active_tab_id,
            status,
            i18n,
        );
    } else {
        parse_tab_by_id(tab_id, tabs, status, i18n);
    }
}

fn sync_output_encoding_from_tab(
    tab: &EditorTabState,
    compact_output: Signal<bool>,
    encrypt_output: Signal<bool>,
) {
    sync_output_encoding_from_options(tab.source_encode_options, compact_output, encrypt_output);
}

fn sync_output_encoding_from_options(
    options: rton_editor_core::EncodeOptions,
    mut compact_output: Signal<bool>,
    mut encrypt_output: Signal<bool>,
) {
    compact_output.set(options.encoding == BinaryEncoding::Compact);
    encrypt_output.set(options.encrypted);
}

pub(crate) fn open_native_files_dialog(
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    mut status: Signal<Status>,
    i18n: I18n,
) -> bool {
    match platform::open_files() {
        Ok(Some(files)) => stage_native_open_files(
            files,
            loaded_files,
            next_loaded_file_id,
            file_selection,
            status,
            i18n,
        ),
        Ok(None) => false,
        Err(error) => {
            status.set(Status::new(error, Tone::Error));
            false
        }
    }
}

pub(crate) fn open_native_folder_dialog(
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    mut status: Signal<Status>,
    i18n: I18n,
) -> bool {
    match platform::open_folder() {
        Ok(Some(files)) => stage_native_open_files(
            files,
            loaded_files,
            next_loaded_file_id,
            file_selection,
            status,
            i18n,
        ),
        Ok(None) => false,
        Err(error) => {
            status.set(Status::new(error, Tone::Error));
            false
        }
    }
}

fn stage_native_open_files(
    files: Vec<platform::NativeOpenFile>,
    loaded_files: Signal<Vec<LoadedFileState>>,
    next_loaded_file_id: Signal<usize>,
    file_selection: Signal<FileSelection>,
    mut status: Signal<Status>,
    i18n: I18n,
) -> bool {
    if files.is_empty() {
        status.set(Status::new(i18n.t("status-no-loadable-files"), Tone::Warn));
        return false;
    }

    let drafts = loaded_file_drafts_from_native(files);
    let indexed =
        stage_loaded_file_drafts(loaded_files, next_loaded_file_id, file_selection, drafts);
    status.set(Status::new(
        i18n.t_args("status-indexed-files", &[("count", indexed.to_string())]),
        Tone::Ok,
    ));
    indexed > 0
}
