use dioxus::prelude::*;
use rton_editor_core::{BinaryEncoding, EncodeOptions, TextFormat, format_bytes};
#[cfg(not(target_arch = "wasm32"))]
use rton_editor_core::{encode_rton_bytes, value_to_text};

#[cfg(target_arch = "wasm32")]
use crate::domain::BatchExportMode;
#[cfg(not(target_arch = "wasm32"))]
use crate::domain::document_for_owned_tab;
use crate::domain::{EditorTabState, Status, Tone, export_rton_name, export_text_name};
use crate::i18n::I18n;
use crate::platform;
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::run_cpu_task;

#[cfg(not(target_arch = "wasm32"))]
use super::document::cache_parsed_document;
use super::tabs::active_tab;

pub(crate) fn export_active_rton(
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    compact_output: Signal<bool>,
    encrypt_output: Signal<bool>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    let Some(active_tab) = active_tab(tabs, active_tab_id) else {
        status.set(Status::new(i18n.t("status-no-document-export"), Tone::Warn));
        return;
    };
    #[cfg(not(target_arch = "wasm32"))]
    let tab_id = active_tab.id;
    let file_name = active_tab.file_name.clone();
    #[cfg(not(target_arch = "wasm32"))]
    let selected_path = active_tab.selected_path.clone();
    let options = EncodeOptions {
        encoding: if *compact_output.read() {
            BinaryEncoding::Compact
        } else {
            BinaryEncoding::Standard
        },
        encrypted: *encrypt_output.read(),
    };

    spawn(async move {
        #[cfg(target_arch = "wasm32")]
        {
            match crate::batch_export_runner::export_tab_in_web_worker(
                active_tab,
                BatchExportMode::Rton,
                options,
            )
            .await
            {
                Ok(bytes) => {
                    let default_name = export_rton_name(&file_name);
                    match platform::save_bytes(&default_name, &bytes) {
                        Ok(true) => status.set(Status::new(
                            i18n.t_args(
                                "status-exported-file-bytes",
                                &[("name", default_name), ("bytes", format_bytes(bytes.len()))],
                            ),
                            Tone::Ok,
                        )),
                        Ok(false) => {
                            status.set(Status::new(i18n.t("status-export-cancelled"), Tone::Info))
                        }
                        Err(error) => status.set(Status::new(error, Tone::Error)),
                    }
                }
                Err(error) => status.set(Status::new(error, Tone::Error)),
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        let doc = match run_cpu_task(move || {
            document_for_owned_tab(active_tab).map_err(|error| error.to_string())
        })
        .await
        {
            Ok(doc) => doc,
            Err(error) => {
                status.set(Status::new(error, Tone::Error));
                return;
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        match run_cpu_task({
            let doc = doc.clone();
            move || encode_rton_bytes(&doc.value, options).map_err(|error| error.to_string())
        })
        .await
        {
            Ok(bytes) => {
                let default_name = export_rton_name(&file_name);
                match platform::save_bytes(&default_name, &bytes) {
                    Ok(true) => {
                        cache_parsed_document(tabs, tab_id, selected_path, doc, false);
                        status.set(Status::new(
                            i18n.t_args(
                                "status-exported-file-bytes",
                                &[("name", default_name), ("bytes", format_bytes(bytes.len()))],
                            ),
                            Tone::Ok,
                        ));
                    }
                    Ok(false) => {
                        status.set(Status::new(i18n.t("status-export-cancelled"), Tone::Info))
                    }
                    Err(error) => status.set(Status::new(error, Tone::Error)),
                }
            }
            Err(error) => status.set(Status::new(error, Tone::Error)),
        }
    });
}

pub(crate) fn export_active_text(
    format: TextFormat,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    let Some(active_tab) = active_tab(tabs, active_tab_id) else {
        status.set(Status::new(i18n.t("status-no-document-export"), Tone::Warn));
        return;
    };
    #[cfg(not(target_arch = "wasm32"))]
    let tab_id = active_tab.id;
    let file_name = active_tab.file_name.clone();
    #[cfg(not(target_arch = "wasm32"))]
    let selected_path = active_tab.selected_path.clone();

    spawn(async move {
        #[cfg(target_arch = "wasm32")]
        {
            let mode = match format {
                TextFormat::Json => BatchExportMode::Json,
                TextFormat::Yaml => BatchExportMode::Yaml,
                TextFormat::Toml => BatchExportMode::Toml,
            };
            match crate::batch_export_runner::export_tab_in_web_worker(
                active_tab,
                mode,
                EncodeOptions::default(),
            )
            .await
            {
                Ok(bytes) => {
                    let default_name = export_text_name(&file_name, format);
                    let text = String::from_utf8_lossy(&bytes);
                    match platform::save_text(&default_name, &text) {
                        Ok(true) => status.set(Status::new(
                            i18n.t_args("status-exported-file", &[("name", default_name)]),
                            Tone::Ok,
                        )),
                        Ok(false) => {
                            status.set(Status::new(i18n.t("status-export-cancelled"), Tone::Info))
                        }
                        Err(error) => status.set(Status::new(error, Tone::Error)),
                    }
                }
                Err(error) => status.set(Status::new(error, Tone::Error)),
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        let doc = match run_cpu_task(move || {
            document_for_owned_tab(active_tab).map_err(|error| error.to_string())
        })
        .await
        {
            Ok(doc) => doc,
            Err(error) => {
                status.set(Status::new(error, Tone::Error));
                return;
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        match run_cpu_task({
            let doc = doc.clone();
            move || value_to_text(&doc.value, format).map_err(|error| error.to_string())
        })
        .await
        {
            Ok(text) => {
                let default_name = export_text_name(&file_name, format);
                match platform::save_text(&default_name, &text) {
                    Ok(true) => {
                        cache_parsed_document(tabs, tab_id, selected_path, doc, false);
                        status.set(Status::new(
                            i18n.t_args("status-exported-file", &[("name", default_name)]),
                            Tone::Ok,
                        ));
                    }
                    Ok(false) => {
                        status.set(Status::new(i18n.t("status-export-cancelled"), Tone::Info))
                    }
                    Err(error) => status.set(Status::new(error, Tone::Error)),
                }
            }
            Err(error) => status.set(Status::new(error, Tone::Error)),
        }
    });
}
