use dioxus::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use rton_editor_core::DecodedDocument;
use rton_editor_core::{BinaryEncoding, EncodeOptions};
use std::sync::Arc;

use crate::application::{DocumentService, ModeSwitchPayload, ParsePayload};
use crate::components::{HexJumpTarget, TextJumpTarget};
use crate::domain::{
    EditorMode, EditorTabState, HexHistory, RtonSurfaceCache, Status, TabTaskState, TextHistory,
    TextSurfaceCache, Tone, default_expanded_paths, empty_editor_text, locate_rton_value_offset,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::domain::{value_search_result_for_doc, value_tree_rows_for_doc_with_expansion};
use crate::i18n::I18n;
use crate::platform::sleep_ms;

use super::tabs::{active_tab, update_tab};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseReport {
    Foreground,
    Background,
}

pub(crate) fn validate_active_tab(
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    let Some(tab) = active_tab(tabs, active_tab_id) else {
        status.set(Status::new(i18n.t("status-no-active-tab"), Tone::Warn));
        return;
    };

    parse_tab(tab, tabs, status, i18n, ParseReport::Foreground);
}

pub(crate) fn parse_tab_by_id(
    tab_id: usize,
    tabs: Signal<Vec<EditorTabState>>,
    status: Signal<Status>,
    i18n: I18n,
) {
    let tab = {
        let tabs_snapshot = tabs.read();
        let Some(tab) = tabs_snapshot.iter().find(|tab| tab.id == tab_id) else {
            return;
        };
        if tab.doc.is_some() || tab.worker_document_id.is_some() || tab.dirty {
            return;
        }
        tab.clone()
    };

    parse_tab(tab, tabs, status, i18n, ParseReport::Background);
}

fn parse_tab(
    tab: EditorTabState,
    tabs: Signal<Vec<EditorTabState>>,
    mut status: Signal<Status>,
    i18n: I18n,
    report: ParseReport,
) {
    let tab_id = tab.id;
    let file_name = tab.file_name.clone();
    let was_dirty = tab.dirty;
    let task_id = tab.task_generation.saturating_add(1);
    let target_mode = tab.mode;
    update_tab(tabs, tab_id, |active| {
        active.task_generation = task_id;
        if report == ParseReport::Foreground {
            active.task_state = Some(TabTaskState {
                id: task_id,
                target_mode,
            });
        }
    });

    spawn(async move {
        let result = DocumentService::parse(tab).await;
        match result {
            Ok(payload) => {
                if !parse_task_is_current(tabs, tab_id, task_id, was_dirty, report) {
                    return;
                }
                apply_parse_payload(tabs, tab_id, payload);
                if report == ParseReport::Foreground {
                    status.set(Status::new(i18n.t("status-parsed"), Tone::Ok));
                }
            }
            Err(error) => {
                if parse_task_is_current(tabs, tab_id, task_id, was_dirty, report) {
                    if report == ParseReport::Foreground {
                        update_tab(tabs, tab_id, |active| active.task_state = None);
                        status.set(Status::new(error, Tone::Error));
                    } else {
                        status.set(Status::new(
                            i18n.t_args(
                                "status-file-error",
                                &[("name", file_name), ("error", error)],
                            ),
                            Tone::Warn,
                        ));
                    }
                }
            }
        }
    });
}

pub(crate) fn switch_active_mode(
    next_mode: EditorMode,
    encode_options: EncodeOptions,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    let Some(tab) = active_tab(tabs, active_tab_id) else {
        return;
    };
    if tab.mode == next_mode {
        return;
    }

    let tab_id = tab.id;
    if let Some(surface) = cached_mode_surface(&tab, next_mode, encode_options) {
        let task_id = tab.task_generation.saturating_add(1);
        update_tab(tabs, tab_id, |active| {
            cache_current_surface(active, encode_options);
            active.task_generation = task_id;
            apply_cached_mode_surface(active, next_mode, surface);
        });
        status.set(Status::new(
            i18n.t_args(
                "status-switched-cached",
                &[("mode", next_mode.label().to_string())],
            ),
            Tone::Info,
        ));
        return;
    }

    let task_id = tab.task_generation.saturating_add(1);
    update_tab(tabs, tab_id, |active| {
        cache_current_surface(active, encode_options);
        active.task_generation = task_id;
        active.task_state = Some(TabTaskState {
            id: task_id,
            target_mode: next_mode,
        });
    });
    status.set(Status::new(
        i18n.t_args(
            "status-switching",
            &[("mode", next_mode.label().to_string())],
        ),
        Tone::Warn,
    ));

    spawn(async move {
        let result = DocumentService::switch_mode(tab, next_mode, encode_options).await;
        match result {
            Ok(payload) => {
                if !task_is_current(tabs, tab_id, task_id) {
                    return;
                }
                apply_mode_switch_payload(tabs, tab_id, next_mode, payload);
                status.set(Status::new(
                    i18n.t_args(
                        "status-switched",
                        &[("mode", next_mode.label().to_string())],
                    ),
                    Tone::Info,
                ));
            }
            Err(error) => {
                if task_is_current(tabs, tab_id, task_id) {
                    update_tab(tabs, tab_id, |active| active.task_state = None);
                    status.set(Status::new(
                        i18n.t_args("status-cannot-switch", &[("error", error)]),
                        Tone::Error,
                    ));
                }
            }
        }
    });
}

pub(crate) fn update_compact_output(
    compact: bool,
    mut compact_output: Signal<bool>,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    status: Signal<Status>,
    i18n: I18n,
) {
    if *compact_output.peek() == compact {
        return;
    }
    compact_output.set(compact);
    refresh_active_rton_encoding(
        EncodeOptions {
            encoding: if compact {
                BinaryEncoding::Compact
            } else {
                BinaryEncoding::Standard
            },
            encrypted: false,
        },
        tabs,
        active_tab_id,
        status,
        i18n,
    );
}

pub(crate) fn refresh_active_rton_encoding(
    encode_options: EncodeOptions,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    let Some(tab) = active_tab(tabs, active_tab_id) else {
        return;
    };
    if tab.mode != EditorMode::RtonHex {
        return;
    }

    let tab_id = tab.id;
    let task_id = tab.task_generation.saturating_add(1);
    let selected_path = tab.selected_path.clone();
    update_tab(tabs, tab_id, |active| {
        active.task_generation = task_id;
        active.task_state = Some(TabTaskState {
            id: task_id,
            target_mode: EditorMode::RtonHex,
        });
    });
    status.set(Status::new(
        i18n.t_args(
            "status-switching",
            &[("mode", EditorMode::RtonHex.label().to_string())],
        ),
        Tone::Warn,
    ));

    spawn(async move {
        let result = DocumentService::switch_mode(tab, EditorMode::RtonHex, encode_options).await;
        match result {
            Ok(payload) => {
                if !task_is_current(tabs, tab_id, task_id) {
                    return;
                }
                apply_mode_switch_payload_with_selection(
                    tabs,
                    tab_id,
                    EditorMode::RtonHex,
                    payload,
                    selected_path,
                );
                status.set(Status::new(
                    i18n.t_args(
                        "status-switched",
                        &[("mode", encode_options.encoding.label().to_string())],
                    ),
                    Tone::Info,
                ));
            }
            Err(error) => {
                if task_is_current(tabs, tab_id, task_id) {
                    update_tab(tabs, tab_id, |active| active.task_state = None);
                    status.set(Status::new(
                        i18n.t_args("status-cannot-switch", &[("error", error)]),
                        Tone::Error,
                    ));
                }
            }
        }
    });
}

pub(crate) fn update_active_search(
    query: String,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
) {
    let active_id = *active_tab_id.read();
    let (doc, worker_document_id, generation) = {
        let tabs_snapshot = tabs.read();
        let Some(tab) = tabs_snapshot.iter().find(|tab| tab.id == active_id) else {
            return;
        };
        (
            tab.doc.clone(),
            tab.worker_document_id,
            tab.search_generation.saturating_add(1),
        )
    };
    update_tab(tabs, active_id, |tab| {
        tab.search_query = query;
        tab.search_result = None;
        tab.search_generation = generation;
    });

    let query = {
        let tabs_snapshot = tabs.read();
        tabs_snapshot
            .iter()
            .find(|tab| tab.id == active_id)
            .map(|tab| tab.search_query.clone())
            .unwrap_or_default()
    };
    if doc.is_none() && worker_document_id.is_none() {
        return;
    }
    if query.trim().is_empty() {
        return;
    }

    spawn(async move {
        sleep_ms(160).await;
        if !search_task_is_current(tabs, active_id, generation, &query) {
            return;
        }
        let result = DocumentService::search_values(doc, worker_document_id, query.clone()).await;
        update_tab(tabs, active_id, |tab| {
            if tab.search_generation == generation && tab.search_query == query {
                tab.search_result = result.map(Arc::new);
            }
        });
    });
}

pub(crate) fn update_selected_path(
    path: String,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
) {
    let active_id = *active_tab_id.read();
    update_tab(tabs, active_id, |tab| {
        tab.selected_path = path;
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn navigate_to_value_path(
    path: String,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    mut next_jump_id: Signal<u64>,
    mut text_jump_target: Signal<Option<TextJumpTarget>>,
    mut hex_jump_target: Signal<Option<HexJumpTarget>>,
    mut status: Signal<Status>,
    i18n: I18n,
) {
    update_selected_path(path.clone(), tabs, active_tab_id);

    let Some(tab) = active_tab(tabs, active_tab_id) else {
        status.set(Status::new(i18n.t("status-no-active-tab"), Tone::Warn));
        return;
    };
    if tab.doc.is_none() && tab.worker_document_id.is_none() {
        status.set(Status::new(i18n.t("status-no-parsed-document"), Tone::Warn));
        return;
    }

    match tab.mode {
        EditorMode::RtonHex => {
            let Some(byte_doc) = tab.byte_doc.as_ref() else {
                status.set(Status::new(i18n.t("status-no-jump-bytes"), Tone::Warn));
                return;
            };

            let Some(offset) = locate_rton_value_offset(byte_doc, &path) else {
                status.set(Status::new(i18n.t("status-offset-not-found"), Tone::Warn));
                return;
            };
            let id = *next_jump_id.read();
            next_jump_id.set(id.saturating_add(1));
            hex_jump_target.set(Some(HexJumpTarget { id, offset }));
            status.set(Status::new(
                i18n.t_args("status-jumped-offset", &[("offset", format!("{offset:X}"))]),
                Tone::Ok,
            ));
        }
        EditorMode::Json | EditorMode::Yaml | EditorMode::Toml => {
            let text_buffer = tab.text_buffer.clone();
            let line_count = text_buffer
                .as_ref()
                .map(|buffer| buffer.line_count())
                .unwrap_or_else(|| text_line_count(tab.editor_text.as_ref()));
            let Some(format) = tab.mode.text_format() else {
                return;
            };
            let id = *next_jump_id.read();
            next_jump_id.set(id.saturating_add(1));
            let doc = tab.doc.clone();
            let worker_document_id = tab.worker_document_id;
            let editor_text = tab.editor_text.clone();
            spawn(async move {
                let position = DocumentService::locate_text(
                    doc,
                    worker_document_id,
                    path,
                    text_buffer,
                    editor_text,
                    format,
                )
                .await;
                let Some(position) = position else {
                    status.set(Status::new(
                        i18n.t("status-text-line-not-found"),
                        Tone::Warn,
                    ));
                    return;
                };
                text_jump_target.set(Some(TextJumpTarget {
                    id,
                    line: position.line,
                    column: position.column,
                    selection_end_column: position.column,
                    line_count,
                    focus: true,
                }));
                status.set(Status::new(
                    i18n.t_args("status-jumped-line", &[("line", position.line.to_string())]),
                    Tone::Ok,
                ));
            });
        }
    }
}

fn text_line_count(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    text.bytes()
        .enumerate()
        .filter(|(index, byte)| *byte == b'\n' && index + 1 < text.len())
        .count()
        + 1
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn cache_parsed_document(
    tabs: Signal<Vec<EditorTabState>>,
    tab_id: usize,
    selected_path: String,
    doc: Arc<DecodedDocument>,
    clear_dirty: bool,
) {
    update_tab(tabs, tab_id, |active| {
        active.tree_rows = value_tree_rows_for_doc_with_expansion(&doc, &active.expanded_paths);
        active.search_result = value_search_result_for_doc(&doc, &active.search_query);
        active.stats = Some(doc.stats.clone());
        active.doc = Some(doc);
        active.worker_document_id = None;
        active.worker_surface_mode = None;
        active.selected_path = selected_path;
        active.task_state = None;
        if clear_dirty {
            active.dirty = false;
        }
    });
}

pub(crate) fn toggle_active_tree_path(
    path: String,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
) {
    let active_id = *active_tab_id.read();
    let payload = {
        let tabs_snapshot = tabs.read();
        let Some(tab) = tabs_snapshot.iter().find(|tab| tab.id == active_id) else {
            return;
        };
        let mut expanded = (*tab.expanded_paths).clone();
        if !expanded.insert(path.clone()) {
            expanded.remove(&path);
        }
        let expanded_paths = Arc::new(expanded);
        let generation = tab.tree_generation.saturating_add(1);
        (
            tab.doc.clone(),
            tab.worker_document_id,
            expanded_paths,
            generation,
        )
    };

    let (doc, worker_document_id, expanded_paths, generation) = payload;
    update_tab(tabs, active_id, |tab| {
        tab.expanded_paths = expanded_paths.clone();
        tab.tree_generation = generation;
    });

    if doc.is_none() && worker_document_id.is_none() {
        return;
    }
    spawn(async move {
        let Some(tree_rows) =
            DocumentService::tree_rows(doc, worker_document_id, expanded_paths.clone()).await
        else {
            return;
        };
        update_tab(tabs, active_id, |tab| {
            if tab.tree_generation == generation {
                tab.tree_rows = tree_rows;
            }
        });
    });
}

fn cached_mode_surface(
    tab: &EditorTabState,
    next_mode: EditorMode,
    encode_options: EncodeOptions,
) -> Option<crate::domain::editor_tab::TabSurface> {
    if tab.doc.is_none() && tab.worker_document_id.is_none() {
        return None;
    }
    match next_mode {
        EditorMode::RtonHex => {
            let cache = tab.rton_cache.as_ref()?;
            if cache.encode_options != encode_options {
                return None;
            }
            Some(crate::domain::editor_tab::TabSurface {
                byte_doc: Some(cache.byte_doc.clone()),
                editor_text: empty_editor_text(),
                text_buffer: None,
                text_state: crate::domain::TextContentState::None,
            })
        }
        EditorMode::Json | EditorMode::Yaml | EditorMode::Toml => {
            let cache = tab
                .text_cache
                .iter()
                .find(|cache| cache.mode == next_mode)?;
            Some(crate::domain::editor_tab::TabSurface {
                byte_doc: None,
                editor_text: cache.editor_text.clone(),
                text_buffer: cache.text_buffer.clone(),
                text_state: cache.text_state.clone(),
            })
        }
    }
}

fn cache_current_surface(tab: &mut EditorTabState, encode_options: EncodeOptions) {
    match tab.mode {
        EditorMode::RtonHex => {
            if let Some(byte_doc) = tab.byte_doc.clone() {
                tab.rton_cache = Some(RtonSurfaceCache {
                    encode_options,
                    byte_doc,
                });
            }
        }
        EditorMode::Json | EditorMode::Yaml | EditorMode::Toml => {
            tab.text_cache.retain(|cache| cache.mode != tab.mode);
            tab.text_cache.push(TextSurfaceCache {
                mode: tab.mode,
                editor_text: tab.editor_text.clone(),
                text_buffer: tab.text_buffer.clone(),
                text_state: tab.text_state.clone(),
            });
        }
    }
}

fn apply_cached_mode_surface(
    tab: &mut EditorTabState,
    next_mode: EditorMode,
    surface: crate::domain::editor_tab::TabSurface,
) {
    tab.byte_doc = surface.byte_doc;
    tab.editor_text = surface.editor_text;
    tab.text_buffer = surface.text_buffer;
    tab.text_state = surface.text_state;
    tab.mode = next_mode;
    tab.text_history = TextHistory::default();
    tab.hex_history = HexHistory::default();
    tab.task_state = None;
}

fn task_is_current(tabs: Signal<Vec<EditorTabState>>, tab_id: usize, task_id: u64) -> bool {
    tabs.read().iter().any(|tab| {
        tab.id == tab_id
            && tab
                .task_state
                .as_ref()
                .is_some_and(|state| state.id == task_id)
    })
}

fn parse_task_is_current(
    tabs: Signal<Vec<EditorTabState>>,
    tab_id: usize,
    task_id: u64,
    was_dirty: bool,
    report: ParseReport,
) -> bool {
    match report {
        ParseReport::Foreground => task_is_current(tabs, tab_id, task_id),
        ParseReport::Background => tabs.read().iter().any(|tab| {
            tab.id == tab_id && tab.task_generation == task_id && tab.dirty == was_dirty
        }),
    }
}

fn search_task_is_current(
    tabs: Signal<Vec<EditorTabState>>,
    tab_id: usize,
    generation: u64,
    query: &str,
) -> bool {
    tabs.read().iter().any(|tab| {
        tab.id == tab_id && tab.search_generation == generation && tab.search_query == query
    })
}

fn apply_parse_payload(tabs: Signal<Vec<EditorTabState>>, tab_id: usize, payload: ParsePayload) {
    update_tab(tabs, tab_id, |active| {
        active.expanded_paths = default_expanded_paths();
        active.tree_rows = payload.tree_rows;
        active.stats = Some(payload.stats);
        active.search_result = if active.search_query == payload.search_query {
            payload.search_result
        } else {
            None
        };
        active.doc = payload.doc;
        active.worker_document_id = payload.worker_document_id;
        active.worker_surface_mode = payload.worker_surface_mode;
        active.selected_path = "$".to_string();
        active.task_state = None;
    });
}

fn apply_mode_switch_payload(
    tabs: Signal<Vec<EditorTabState>>,
    tab_id: usize,
    next_mode: EditorMode,
    payload: ModeSwitchPayload,
) {
    apply_mode_switch_payload_with_selection(tabs, tab_id, next_mode, payload, "$".to_string());
}

fn apply_mode_switch_payload_with_selection(
    tabs: Signal<Vec<EditorTabState>>,
    tab_id: usize,
    next_mode: EditorMode,
    payload: ModeSwitchPayload,
    selected_path: String,
) {
    update_tab(tabs, tab_id, |active| {
        active.expanded_paths = default_expanded_paths();
        active.tree_rows = payload.tree_rows;
        active.stats = Some(payload.stats);
        active.search_result = if active.search_query == payload.search_query {
            payload.search_result
        } else {
            None
        };
        active.doc = payload.doc;
        active.worker_document_id = payload.worker_document_id;
        active.worker_surface_mode = payload.worker_document_id.map(|_| next_mode);
        active.byte_doc = payload.surface.byte_doc;
        active.editor_text = payload.surface.editor_text;
        active.text_buffer = payload.surface.text_buffer;
        active.text_state = payload.surface.text_state;
        active.mode = next_mode;
        active.selected_path = selected_path;
        active.text_history = TextHistory::default();
        active.hex_history = HexHistory::default();
        active.task_state = None;
        active.dirty = payload.was_dirty;
        if next_mode.text_format().is_some() {
            active.text_cache.retain(|cache| cache.mode != next_mode);
            active.text_cache.push(TextSurfaceCache {
                mode: next_mode,
                editor_text: active.editor_text.clone(),
                text_buffer: active.text_buffer.clone(),
                text_state: active.text_state.clone(),
            });
        }
    });
}

#[cfg(test)]
mod surface_cache_tests {
    use super::*;
    use crate::domain::{ByteDocument, create_text_tab};
    use rton_editor_core::{BinaryEncoding, TextFormat, parse_text};

    #[test]
    fn rton_surface_cache_restores_only_matching_encoding() {
        let mut tab = create_text_tab(
            1,
            "sample.json".to_string(),
            r#"{"value":1}"#.to_string(),
            TextFormat::Json,
        )
        .expect("text tab");
        tab.doc = Some(Arc::new(
            parse_text(
                &tab.text_buffer.as_ref().expect("text buffer").materialize(),
                TextFormat::Json,
            )
            .expect("parsed document"),
        ));
        tab.mode = EditorMode::RtonHex;
        tab.byte_doc = Some(ByteDocument::from_vec(b"RTON".to_vec()));
        tab.editor_text = empty_editor_text();
        tab.text_buffer = None;
        tab.text_state = crate::domain::TextContentState::None;

        let standard = EncodeOptions {
            encoding: BinaryEncoding::Standard,
            encrypted: false,
        };
        cache_current_surface(&mut tab, standard);

        let restored =
            cached_mode_surface(&tab, EditorMode::RtonHex, standard).expect("standard RTON cache");
        assert_eq!(restored.byte_doc.expect("cached bytes").to_vec(), b"RTON");

        let compact = EncodeOptions {
            encoding: BinaryEncoding::Compact,
            encrypted: false,
        };
        assert!(cached_mode_surface(&tab, EditorMode::RtonHex, compact).is_none());
    }
}
