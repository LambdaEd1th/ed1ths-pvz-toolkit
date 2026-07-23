use dioxus::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rton_editor_core::encode_rton_bytes;
use rton_editor_core::{BinaryEncoding, EncodeOptions};
#[cfg(target_arch = "wasm32")]
use rton_editor_core::{TextFormat, WorkerDocumentSource, WorkerRtonSizeRequest};

#[cfg(target_arch = "wasm32")]
use crate::app_i18n::{bump_i18n_revision, initial_locale, load_i18n_sources_async};
#[cfg(not(target_arch = "wasm32"))]
use crate::domain::document_for_owned_tab;
use crate::domain::{EditorMode, EditorTabState};
#[cfg(target_arch = "wasm32")]
use crate::domain::{Status, Tone};
#[cfg(target_arch = "wasm32")]
use crate::i18n::{I18n, Locale};
#[cfg(not(target_arch = "wasm32"))]
use crate::platform::run_cpu_task;
#[cfg(target_arch = "wasm32")]
use crate::platform::run_rton_size_worker;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RtonOutputSize {
    pub(super) tab_id: usize,
    pub(super) compact: bool,
    pub(super) encrypted: bool,
    pub(super) byte_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RtonOutputSourceKey {
    pub(super) tab_id: usize,
    pub(super) content_revision: u64,
    pub(super) mode: EditorMode,
}

pub(super) fn use_rton_output_size_effect(
    source_key: Option<RtonOutputSourceKey>,
    tabs: Signal<Vec<EditorTabState>>,
    compact: bool,
    encrypted: bool,
    mut output_size: Signal<Option<RtonOutputSize>>,
    mut output_size_generation: Signal<u64>,
) {
    use_effect(use_reactive(
        &(source_key, compact, encrypted),
        move |(source_key, compact, encrypted)| {
            let generation = output_size_generation.peek().saturating_add(1);
            output_size_generation.set(generation);

            let Some(source_key) = source_key else {
                output_size.set(None);
                return;
            };
            if source_key.mode != EditorMode::RtonHex || !encrypted {
                output_size.set(None);
                return;
            }

            let tab_id = source_key.tab_id;
            let tab = tabs
                .peek()
                .iter()
                .find(|tab| {
                    tab.id == source_key.tab_id
                        && tab.content_revision == source_key.content_revision
                })
                .cloned();
            let Some(tab) = tab else {
                output_size.set(None);
                return;
            };
            output_size.set(None);
            spawn(async move {
                let encode_options = EncodeOptions {
                    encoding: if compact {
                        BinaryEncoding::Compact
                    } else {
                        BinaryEncoding::Standard
                    },
                    encrypted,
                };
                let Ok(byte_count) = rton_output_size_for_tab(tab, encode_options).await else {
                    return;
                };
                if *output_size_generation.peek() == generation {
                    output_size.set(Some(RtonOutputSize {
                        tab_id,
                        compact,
                        encrypted,
                        byte_count,
                    }));
                }
            });
        },
    ));
}

#[cfg(not(target_arch = "wasm32"))]
async fn rton_output_size_for_tab(
    tab: EditorTabState,
    encode_options: EncodeOptions,
) -> Result<usize, String> {
    let doc = run_cpu_task(move || document_for_owned_tab(tab).map_err(|error| error.to_string()))
        .await?;
    run_cpu_task(move || {
        encode_rton_bytes(&doc.value, encode_options)
            .map(|bytes| bytes.len())
            .map_err(|error| error.to_string())
    })
    .await
}

#[cfg(target_arch = "wasm32")]
async fn rton_output_size_for_tab(
    tab: EditorTabState,
    encode_options: EncodeOptions,
) -> Result<usize, String> {
    let document_id = (!tab.dirty).then_some(tab.worker_document_id).flatten();
    let request = WorkerRtonSizeRequest {
        document_id,
        source: if document_id.is_some() {
            None
        } else {
            Some(worker_document_source(&tab)?)
        },
        encode_options,
    };
    let response = match run_rton_size_worker(request).await {
        Ok(response) => response,
        Err(_) if document_id.is_some() => {
            run_rton_size_worker(WorkerRtonSizeRequest {
                document_id: None,
                source: Some(worker_document_source(&tab)?),
                encode_options,
            })
            .await?
        }
        Err(error) => return Err(error),
    };
    Ok(response.byte_len)
}

#[cfg(target_arch = "wasm32")]
fn worker_document_source(tab: &EditorTabState) -> Result<WorkerDocumentSource, String> {
    match tab.mode {
        EditorMode::RtonHex => {
            let bytes = tab
                .byte_doc
                .as_ref()
                .map(|byte_doc| byte_doc.as_cow().into_owned())
                .ok_or_else(|| "Missing RTON bytes".to_string())?;
            Ok(WorkerDocumentSource::RtonBytes(bytes))
        }
        EditorMode::Json | EditorMode::Yaml | EditorMode::Toml => {
            let format = match tab.mode {
                EditorMode::Json => TextFormat::Json,
                EditorMode::Yaml => TextFormat::Yaml,
                EditorMode::Toml => TextFormat::Toml,
                EditorMode::RtonHex => unreachable!("rton handled above"),
            };
            let text = tab
                .text_buffer
                .as_ref()
                .map(|buffer| buffer.materialize())
                .unwrap_or_else(|| tab.editor_text.to_string());
            Ok(WorkerDocumentSource::Text { text, format })
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) fn use_web_i18n_loader(
    mut locale: Signal<Locale>,
    mut status: Signal<Status>,
    mut i18n_loaded: Signal<bool>,
    i18n_revision: Signal<u64>,
    initial_locale_snapshot: Locale,
) {
    use_future(move || async move {
        let loaded = load_i18n_sources_async().await;
        i18n_loaded.set(true);
        if loaded == 0 {
            return;
        }

        bump_i18n_revision(i18n_revision);
        if *locale.peek() == initial_locale_snapshot {
            let next_locale = initial_locale();
            locale.set(next_locale);
            status.set(Status::new(
                I18n::new(next_locale).t("status-ready"),
                Tone::Ok,
            ));
        }
    });
}

#[cfg(target_arch = "wasm32")]
pub(super) fn use_web_visual_viewport_effect() {
    use_effect(|| {
        dioxus::document::eval(
            r#"
            (() => {
                const viewport = window.visualViewport;
                if (!viewport) return;

                const applyViewport = () => {
                    const height = Math.max(1, Math.round(viewport.height));
                    const keyboardOpen = window.innerWidth <= 900
                        && height < window.innerHeight - 120;
                    document.documentElement.style.setProperty(
                        '--rton-visual-viewport-height',
                        `${height}px`,
                    );
                    document.documentElement.dataset.rtonKeyboard = keyboardOpen
                        ? 'open'
                        : 'closed';
                };

                if (!window.__rtonVisualViewportHandler) {
                    window.__rtonVisualViewportHandler = applyViewport;
                    viewport.addEventListener('resize', applyViewport, { passive: true });
                    viewport.addEventListener('scroll', applyViewport, { passive: true });
                    window.addEventListener('resize', applyViewport, { passive: true });
                }
                applyViewport();
            })();
            "#,
        );
    });
}

#[cfg(target_arch = "wasm32")]
pub(super) fn use_web_file_drop_guard() {
    use_effect(|| {
        dioxus::document::eval(
            r#"
            (() => {
                if (window.__rtonFileDropGuard) return;
                const blocksNativeFileDrop = (event) => {
                    const transfer = event.dataTransfer;
                    if (!transfer) return;
                    const hasFiles = Array.from(transfer.types || []).includes('Files')
                        || Array.from(transfer.items || []).some((item) => item.kind === 'file');
                    if (hasFiles) {
                        event.preventDefault();
                    }
                };
                window.__rtonFileDropGuard = blocksNativeFileDrop;
                document.addEventListener('dragover', blocksNativeFileDrop, true);
                document.addEventListener('drop', blocksNativeFileDrop, true);
            })();
            "#,
        );
    });
}
