use crate::i18n::{self, I18n};
use dioxus::prelude::*;
use rton_editor_core::{BinaryEncoding, EncodeOptions, TextFormat};
use std::cell::RefCell;
use std::rc::Rc;
use toolkit_ui::{ContextSheet, ToolPage, ToolPageHeader, WorkspaceCard};

use crate::app_actions::*;
#[cfg(not(target_arch = "wasm32"))]
use crate::app_constants::{DESKTOP_WINDOW_MIN_HEIGHT, DESKTOP_WINDOW_MIN_WIDTH};
use crate::app_i18n::*;
use crate::components::{TabStrip, UnsavedChangesDialog, file_path_matches_scope};
use crate::domain::{
    BatchExportMode, ByteDocument, EditorMode, HexEdit, IdentityArc, Status, TextBuffer,
    TextRangeReplacement, TextSearchMatch, ThemePreference, Tone, default_expanded_paths,
    empty_tree_rows,
};
use crate::file_import::*;

use super::editor_stage::{EditorStage, EditorStageTab};
use super::effects::{RtonOutputSize, RtonOutputSourceKey, use_rton_output_size_effect};
#[cfg(target_arch = "wasm32")]
use super::effects::{
    use_web_file_drop_guard, use_web_i18n_loader, use_web_visual_viewport_effect,
};
use super::file_panel::FilePanel;
use super::handlers::{start_tab_drag_if_needed, update_tab_drop_marker_for_drag};
use super::index_panel::IndexPanel;
use super::layout::DocumentNotice;
use super::page_actions::PageActions;
use super::signals::{self, AppSignals, use_app_signals};
use super::snapshot::{
    FilePanelCacheKey, FilePanelSnapshot, editor_search_result_from_core, editor_search_snapshot,
    empty_editor_search_result_snapshot, empty_editor_search_snapshot, file_panel_snapshot,
    loaded_files_fingerprint, tab_headers_for_tabs,
};
use super::workspace_drop::{handle_workspace_file_drop, workspace_drag_has_files};
const _: Asset = asset!("/assets/rton", AssetOptions::folder());
const TAILWIND_CSS: Asset = asset!("/assets/rton/tailwind.css");
const TOKENS_CSS: Asset = asset!("/assets/rton/tokens.css");
const APP_CSS: Asset = asset!("/assets/rton/style.css");
const PAGE_CSS: Asset = asset!("/assets/rton/page.css");

#[component]
pub(crate) fn App(theme: ThemePreference) -> Element {
    #[cfg(not(target_arch = "wasm32"))]
    let _loaded_i18n_count = use_hook(load_i18n_sources);
    let initial_locale_snapshot = initial_locale();
    let signals = use_app_signals(initial_locale_snapshot, theme);
    use_context_provider(|| signals);
    let mut theme_preference = signals.preferences.theme_preference;
    use_effect(use_reactive(&theme, move |theme| {
        if *theme_preference.peek() != theme {
            theme_preference.set(theme);
        }
    }));

    #[cfg(target_arch = "wasm32")]
    use_web_i18n_loader(
        signals.preferences.locale,
        signals.status,
        signals.preferences.i18n_loaded,
        signals.preferences.i18n_revision,
        initial_locale_snapshot,
    );
    #[cfg(target_arch = "wasm32")]
    use_web_visual_viewport_effect();
    #[cfg(target_arch = "wasm32")]
    use_web_file_drop_guard();

    #[cfg(target_arch = "wasm32")]
    if !*signals.preferences.i18n_loaded.read() {
        return rsx! {
            document::Stylesheet { href: TAILWIND_CSS }
            document::Stylesheet { href: TOKENS_CSS }
            document::Stylesheet { href: APP_CSS }
            document::Stylesheet { href: PAGE_CSS }
            div { class: "rton-app rton-loading-screen", "Loading localization..." }
        };
    }

    rsx! {
        RtonPage {}
    }
}

#[component]
fn RtonPage() -> Element {
    let signals = use_context::<AppSignals>();
    let AppSignals {
        workspace:
            signals::WorkspaceSignals {
                tabs,
                loaded_files,
                active_tab_id,
                next_tab_id,
                next_loaded_file_id,
                mut file_search_query,
                file_selection,
            },
        preferences:
            signals::PreferenceSignals {
                compact_output,
                encrypt_output,
                mut preferred_editor_mode,
                theme_preference,
                locale,
                i18n_revision,
                #[cfg(target_arch = "wasm32")]
                    i18n_loaded: _,
                line_wrapping,
            },
        editor:
            signals::EditorSessionSignals {
                mut editor_search_panel_visible,
                mut editor_search_text,
                mut editor_replace_text,
                mut editor_search_case_sensitive,
                mut editor_search_match_index,
                editor_search_focus_token,
                next_jump_id,
                text_jump_target,
                hex_jump_target,
            },
        layout:
            signals::LayoutSignals {
                dragging_files,
                file_sheet_open,
                inspector_sheet_open,
                dragged_tab_id,
                tab_drop_marker,
            },
        mut status,
    } = signals;

    let active_id_snapshot = *active_tab_id.read();
    let mut pending_close_tab_id = use_signal(|| None::<usize>);
    let rton_output_size = use_signal(|| None::<RtonOutputSize>);
    let rton_output_size_generation = use_signal(|| 0_u64);
    #[cfg(not(target_arch = "wasm32"))]
    let last_window_size_save = use_signal(|| None::<(u32, u32)>);
    let (
        active_output_source_key,
        active_stage_tab_snapshot,
        active_stats_snapshot,
        has_parsed_document_snapshot,
        active_byte_doc_snapshot,
        active_file_name_snapshot,
        active_mode_snapshot,
        active_search,
        value_search_result,
        selected_path_snapshot,
        tree_rows,
        expanded_paths_snapshot,
        can_undo_snapshot,
        can_redo_snapshot,
        active_text_buffer_snapshot,
        tab_headers,
    ) = {
        let tabs_snapshot = tabs.read();
        let active_tab_snapshot = tabs_snapshot
            .iter()
            .find(|tab| tab.id == active_id_snapshot)
            .or_else(|| tabs_snapshot.first());
        let active_search = active_tab_snapshot
            .map(|tab| tab.search_query.clone())
            .unwrap_or_default();
        let value_search_result = if active_search.trim().is_empty() {
            None
        } else {
            active_tab_snapshot
                .and_then(|tab| tab.search_result.clone())
                .map(IdentityArc::from)
        };
        (
            active_tab_snapshot.map(|tab| RtonOutputSourceKey {
                tab_id: tab.id,
                content_revision: tab.content_revision,
                mode: tab.mode,
            }),
            active_tab_snapshot.map(|tab| EditorStageTab {
                id: tab.id,
                mode: tab.mode,
                worker_document_id: (!tab.dirty && tab.worker_surface_mode == Some(tab.mode))
                    .then_some(tab.worker_document_id)
                    .flatten(),
                text_buffer: tab.text_buffer.clone().map(IdentityArc::from),
                text_state: tab.text_state.clone(),
                task_state: tab.task_state.clone(),
            }),
            active_tab_snapshot.and_then(|tab| tab.stats.clone()),
            active_tab_snapshot
                .is_some_and(|tab| tab.doc.is_some() || tab.worker_document_id.is_some()),
            active_tab_snapshot.and_then(|tab| tab.byte_doc.clone()),
            active_tab_snapshot.map(|tab| tab.file_name.clone()),
            active_tab_snapshot.map(|tab| tab.mode),
            active_search,
            value_search_result,
            active_tab_snapshot
                .map(|tab| tab.selected_path.clone())
                .unwrap_or_else(|| "$".to_string()),
            active_tab_snapshot
                .map(|tab| IdentityArc::from(tab.tree_rows.clone()))
                .unwrap_or_else(|| IdentityArc::from(empty_tree_rows())),
            active_tab_snapshot
                .map(|tab| IdentityArc::from(tab.expanded_paths.clone()))
                .unwrap_or_else(|| IdentityArc::from(default_expanded_paths())),
            active_tab_snapshot.is_some_and(tab_can_undo),
            active_tab_snapshot.is_some_and(tab_can_redo),
            active_tab_snapshot
                .filter(|tab| tab.mode.text_format().is_some())
                .and_then(|tab| tab.text_buffer.clone())
                .map(IdentityArc::from),
            tab_headers_for_tabs(&tabs_snapshot),
        )
    };
    let pending_close_file_name = (*pending_close_tab_id.read()).and_then(|id| {
        tabs.read()
            .iter()
            .find(|tab| tab.id == id)
            .map(|tab| tab.file_name.clone())
    });
    let compact_snapshot = *compact_output.read();
    let encrypt_snapshot = *encrypt_output.read();
    use_rton_output_size_effect(
        active_output_source_key,
        tabs,
        compact_snapshot,
        encrypt_snapshot,
        rton_output_size,
        rton_output_size_generation,
    );
    let rton_output_size_snapshot = *rton_output_size.read();
    let preferred_editor_mode_snapshot = *preferred_editor_mode.read();
    let theme_preference_snapshot = *theme_preference.read();
    let locale_snapshot = *locale.read();
    let _i18n_revision_snapshot = *i18n_revision.read();
    let i18n = I18n::new(locale_snapshot);
    let language_options = i18n::language_options(i18n);
    let line_wrapping_snapshot = *line_wrapping.read();
    let editor_search_panel_visible_snapshot = *editor_search_panel_visible.read();
    let editor_search_text_snapshot = editor_search_text.read().clone();
    let editor_replace_text_snapshot = editor_replace_text.read().clone();
    let editor_search_case_sensitive_snapshot = *editor_search_case_sensitive.read();
    let editor_search_match_index_snapshot = *editor_search_match_index.read();
    let editor_search_focus_token_snapshot = *editor_search_focus_token.read();
    let should_search_editor_text =
        editor_search_panel_visible_snapshot && !editor_search_text_snapshot.is_empty();
    let editor_search_result = {
        let mut result = use_signal(empty_editor_search_result_snapshot);
        let mut generation = use_signal(|| 0_u64);
        let worker_document_id = active_stage_tab_snapshot
            .as_ref()
            .and_then(|tab| tab.worker_document_id);
        use_effect(use_reactive(
            &(
                active_text_buffer_snapshot.clone(),
                worker_document_id,
                should_search_editor_text,
                editor_search_text_snapshot.clone(),
                editor_search_case_sensitive_snapshot,
            ),
            move |(text_buffer, worker_document_id, should_search, query, case_sensitive)| {
                let next_generation = (*generation.peek()).saturating_add(1);
                generation.set(next_generation);
                result.set(empty_editor_search_result_snapshot());
                if !should_search {
                    return;
                }
                let Some(text_buffer) = text_buffer else {
                    return;
                };
                spawn(async move {
                    crate::platform::sleep_ms(120).await;
                    if *generation.peek() != next_generation {
                        return;
                    }
                    let response = crate::application::SearchService::search_text(
                        text_buffer.arc(),
                        worker_document_id,
                        query,
                        case_sensitive,
                    )
                    .await;
                    if *generation.peek() == next_generation
                        && let Ok(response) = response
                    {
                        result.set(editor_search_result_from_core(response));
                    }
                });
            },
        ));
        result
    };
    let editor_search = if should_search_editor_text {
        editor_search_snapshot(
            &editor_search_result.read(),
            &editor_search_text_snapshot,
            editor_search_match_index_snapshot,
            i18n,
        )
    } else {
        empty_editor_search_snapshot(&editor_search_text_snapshot, i18n)
    };
    let text_search_jump = if should_search_editor_text {
        active_text_buffer_snapshot
            .as_deref()
            .and_then(|text_buffer| {
                current_text_search_jump(
                    text_buffer,
                    &editor_search.matches,
                    editor_search_match_index_snapshot,
                    editor_search_focus_token_snapshot,
                )
            })
    } else {
        None
    };
    let active_text_jump_target = text_search_jump.or(*text_jump_target.read());
    let file_search_snapshot = file_search_query.read().clone();
    let file_selection_snapshot = file_selection.read().clone();
    let file_panel_cache =
        use_hook(|| Rc::new(RefCell::new(None::<(FilePanelCacheKey, FilePanelSnapshot)>)));
    let file_panel_memo = use_memo(move || {
        let loaded_files_snapshot = loaded_files.read();
        let tabs_snapshot = tabs.read();
        let tab_headers = tab_headers_for_tabs(&tabs_snapshot);
        let active_id = *active_tab_id.read();
        let file_search = file_search_query.read().clone();
        let selection = file_selection.read().clone();
        let locale_snapshot = *locale.read();
        let i18n_revision_snapshot = *i18n_revision.read();
        let key = FilePanelCacheKey {
            loaded_files_fingerprint: loaded_files_fingerprint(&loaded_files_snapshot),
            tab_headers: tab_headers.clone(),
            active_tab_id: active_id,
            search_query: file_search.clone(),
            selection: selection.clone(),
            locale_code: locale_snapshot.code(),
            i18n_revision: i18n_revision_snapshot,
        };
        if let Some((cached_key, snapshot)) = file_panel_cache.borrow().as_ref()
            && cached_key == &key
        {
            return snapshot.clone();
        }
        let snapshot = file_panel_snapshot(
            &loaded_files_snapshot,
            &tab_headers,
            active_id,
            &file_search,
            &selection,
            I18n::new(locale_snapshot),
        );
        *file_panel_cache.borrow_mut() = Some((key, snapshot.clone()));
        snapshot
    });
    let file_panel = file_panel_memo.read().clone();
    let dragged_tab_id_snapshot = *dragged_tab_id.read();
    let tab_drop_marker_snapshot = *tab_drop_marker.read();
    let status_snapshot = status.read().clone();
    let no_file_label = i18n.t("common-no-file");
    let active_file_label = active_file_name_snapshot.unwrap_or_else(|| no_file_label.clone());
    let active_text_byte_count_snapshot = active_stage_tab_snapshot
        .as_ref()
        .and_then(|tab| tab.text_buffer.as_ref())
        .map(|buffer| buffer.byte_count());
    let input_value_label = file_input_summary(
        active_stage_tab_snapshot.is_some(),
        active_byte_doc_snapshot.as_ref(),
        active_text_byte_count_snapshot,
        i18n,
    );
    let output_value_label = file_output_summary(FileOutputSummaryInput {
        has_active_file: active_stage_tab_snapshot.is_some(),
        active_tab_id: active_output_source_key.map(|source| source.tab_id),
        mode: active_mode_snapshot,
        byte_doc: active_byte_doc_snapshot.as_ref(),
        text_byte_count: active_text_byte_count_snapshot,
        compact: compact_snapshot,
        encrypted: encrypt_snapshot,
        rton_output_size: rton_output_size_snapshot,
        i18n,
    });
    let parse_current = move |_| {
        validate_active_tab(tabs, active_tab_id, status, i18n);
    };

    let new_tab_mode_snapshot = active_mode_snapshot
        .or(preferred_editor_mode_snapshot)
        .unwrap_or(EditorMode::RtonHex);
    let create_blank_tab = move |_| {
        open_blank_tab(
            next_tab_id,
            tabs,
            active_tab_id,
            new_tab_mode_snapshot,
            rton_display_encode_options(*compact_output.read()),
            status,
            i18n,
        );
    };

    let activate_tab = move |id: usize| {
        activate_tab_by_id(id, tabs, active_tab_id, status, i18n);
    };

    let open_loaded_file = move |file_id: usize| {
        open_loaded_file_by_id(
            file_id,
            loaded_files,
            next_tab_id,
            tabs,
            active_tab_id,
            compact_output,
            encrypt_output,
            *preferred_editor_mode.read(),
            status,
            i18n,
        );
    };

    let request_close_tab = move |id: usize| {
        if tab_requires_close_confirmation(&tabs.read(), id) {
            pending_close_tab_id.set(Some(id));
            return;
        }
        close_tab_by_id(id, tabs, active_tab_id, status, i18n);
        unlink_loaded_file_tab(loaded_files, id);
    };

    let cancel_pending_close = move |_| pending_close_tab_id.set(None);
    let discard_pending_close = move |_| {
        let pending_id = *pending_close_tab_id.peek();
        pending_close_tab_id.set(None);
        if let Some(id) = pending_id {
            close_tab_by_id(id, tabs, active_tab_id, status, i18n);
            unlink_loaded_file_tab(loaded_files, id);
        }
    };

    let remove_file_list_item = move |key: String| {
        if let Some(name) = remove_file_list_item_by_key(key, loaded_files, tabs, active_tab_id) {
            status.set(Status::new(
                i18n.t_args("status-removed-file", &[("name", name)]),
                Tone::Info,
            ));
        }
    };

    let remove_file_list_path = {
        let file_list_items = file_panel.filtered_items.clone();
        move |path: String| {
            let keys = file_list_items
                .iter()
                .filter(|item| file_path_matches_scope(&item.path, &path))
                .map(|item| item.key.clone())
                .collect::<Vec<_>>();
            let count = remove_file_list_items_by_keys(keys, loaded_files, tabs, active_tab_id);
            if count > 0 {
                status.set(Status::new(
                    i18n.t_args(
                        "status-removed-path",
                        &[("name", path), ("count", count.to_string())],
                    ),
                    Tone::Info,
                ));
            }
        }
    };

    let switch_mode = move |next_mode: EditorMode| {
        let _ = crate::platform::save_editor_mode_preference(next_mode);
        preferred_editor_mode.set(Some(next_mode));
        switch_active_mode(
            next_mode,
            rton_display_encode_options(*compact_output.read()),
            tabs,
            active_tab_id,
            status,
            i18n,
        );
    };

    let handle_compact_change = move |compact: bool| {
        update_compact_output(compact, compact_output, tabs, active_tab_id, status, i18n);
    };

    let select_all_visible_files = {
        let file_search_snapshot = file_search_snapshot.clone();
        move |_| {
            select_visible_files(&file_search_snapshot, file_selection, status, i18n);
        }
    };

    let clear_selected_files = {
        let file_search_snapshot = file_search_snapshot.clone();
        move |_| {
            clear_visible_file_selection(&file_search_snapshot, file_selection, status, i18n);
        }
    };

    let toggle_selected_file = move |(key, checked): (String, bool)| {
        toggle_selected_file_key(key, checked, file_selection);
    };

    let toggle_selected_path = move |(path, checked): (String, bool)| {
        toggle_selected_file_path(path, checked, file_selection);
    };

    let batch_export_selected: EventHandler<BatchExportMode> = {
        let file_list_items = file_panel.all_items.clone();
        EventHandler::new(move |mode: BatchExportMode| {
            batch_export_selected_file_items(
                mode,
                file_list_items.arc(),
                file_selection,
                tabs,
                loaded_files,
                compact_output,
                encrypt_output,
                status,
                i18n,
            );
        })
    };

    let update_hex_edit = move |edits: Vec<HexEdit>| {
        update_active_hex_edits(edits, tabs, active_tab_id);
    };

    let update_virtual_text_range = move |replacement: TextRangeReplacement| {
        update_active_text_range(replacement, tabs, active_tab_id);
    };

    let undo_edit: EventHandler<()> = EventHandler::new(move |_| {
        undo_active_edit(tabs, active_tab_id);
    });

    let redo_edit: EventHandler<()> = EventHandler::new(move |_| {
        redo_active_edit(tabs, active_tab_id);
    });

    let go_to_previous_editor_match = move |_| {
        go_to_previous_text_match(editor_search.match_count, editor_search_match_index);
        request_text_search_focus(editor_search.match_count, editor_search_focus_token);
    };

    let go_to_next_editor_match = move |_| {
        go_to_next_text_match(editor_search.match_count, editor_search_match_index);
        request_text_search_focus(editor_search.match_count, editor_search_focus_token);
    };

    let replace_current_editor_match = {
        let matches = editor_search.matches.clone();
        let replacement = editor_replace_text_snapshot.clone();
        move |_| {
            replace_current_text_match(
                &matches,
                &replacement,
                tabs,
                active_tab_id,
                editor_search_match_index,
            );
        }
    };

    let replace_all_editor_matches = {
        let query = editor_search_text_snapshot.clone();
        let replacement = editor_replace_text_snapshot.clone();
        let case_sensitive = editor_search_case_sensitive_snapshot;
        move |_| {
            replace_all_text_matches_in_editor(
                &query,
                &replacement,
                case_sensitive,
                tabs,
                active_tab_id,
                editor_search_match_index,
            );
        }
    };

    let handle_editor_search_key = move |event: KeyboardEvent| {
        if handle_text_find_key(
            event,
            editor_search.match_count,
            editor_search_panel_visible,
            editor_search_match_index,
        ) {
            request_text_search_focus(editor_search.match_count, editor_search_focus_token);
        }
    };

    let handle_editor_replace_key = {
        let matches = editor_search.matches.clone();
        let replacement = editor_replace_text_snapshot.clone();
        move |event: KeyboardEvent| {
            handle_text_replace_key(
                event,
                &matches,
                &replacement,
                editor_search_panel_visible,
                tabs,
                active_tab_id,
                editor_search_match_index,
            );
        }
    };

    let start_tab_drag = move |id: usize| {
        start_tab_drag_if_needed(id, tabs, dragged_tab_id, tab_drop_marker);
    };

    let update_tab_drop_marker = move |marker| {
        update_tab_drop_marker_for_drag(marker, dragged_tab_id, tab_drop_marker);
    };

    let finish_tab_drag = move |_| {
        finish_tab_drag_state(tabs, dragged_tab_id, tab_drop_marker);
    };

    let export_rton = move |_| {
        export_active_rton(
            tabs,
            active_tab_id,
            compact_output,
            encrypt_output,
            status,
            i18n,
        );
    };

    let export_text = move |format: TextFormat| {
        export_active_text(format, tabs, active_tab_id, status, i18n);
    };

    let open_native_files = move |_| {
        if open_native_files_dialog(
            loaded_files,
            next_loaded_file_id,
            file_selection,
            status,
            i18n,
        ) {
            reveal_file_sheet_after_commit(file_sheet_open, inspector_sheet_open);
        }
    };

    let open_native_folder = move |_| {
        if open_native_folder_dialog(
            loaded_files,
            next_loaded_file_id,
            file_selection,
            status,
            i18n,
        ) {
            reveal_file_sheet_after_commit(file_sheet_open, inspector_sheet_open);
        }
    };

    let select_value_path = move |path: String| {
        navigate_to_value_path(
            path,
            tabs,
            active_tab_id,
            next_jump_id,
            text_jump_target,
            hex_jump_target,
            status,
            i18n,
        );
    };

    rsx! {
        document::Stylesheet { href: TAILWIND_CSS }
        document::Stylesheet { href: TOKENS_CSS }
        document::Stylesheet { href: APP_CSS }
        document::Stylesheet { href: PAGE_CSS }
        div {
            class: "{theme_preference_snapshot.shell_class()} rton-page-host",
            onresize: move |event| {
                if let Ok(size) = event.get_content_box_size() {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        save_desktop_viewport_size(
                            size.width,
                            size.height,
                            last_window_size_save,
                        );
                    }
                    #[cfg(target_arch = "wasm32")]
                    let _ = size;
                }
            },
            onmouseup: move |_| finish_tab_drag_state(tabs, dragged_tab_id, tab_drop_marker),
            ondragover: move |event| {
                if workspace_drag_has_files(&event) {
                    event.prevent_default();
                    let mut dragging_files = dragging_files;
                    dragging_files.set(true);
                }
            },
            ondragleave: move |_| {
                let mut dragging_files = dragging_files;
                dragging_files.set(false);
            },
            ondrop: move |event| {
                if !workspace_drag_has_files(&event) {
                    return;
                }
                // Chrome requires this during event dispatch; doing it in the async reader is too late.
                event.prevent_default();
                spawn(async move {
                    let imported = handle_workspace_file_drop(
                        event,
                        dragging_files,
                        loaded_files,
                        next_loaded_file_id,
                        file_selection,
                        status,
                        i18n,
                    )
                    .await;
                    if imported {
                        reveal_file_sheet_after_commit(file_sheet_open, inspector_sheet_open);
                    }
                });
            },
            ToolPage { namespace: "rton", class: "rton-app",
                ToolPageHeader {
                    class: "rton-page-header",
                    eyebrow: i18n.t("page-eyebrow"),
                    title: "RTON Editor",
                    description: i18n.t("page-description"),
                    actions: rsx! {
                        PageActions {
                            i18n,
                            active_mode_snapshot,
                            preferred_mode_snapshot: preferred_editor_mode_snapshot,
                            active_file_label: active_file_label.clone(),
                            compact_snapshot,
                            encrypt_snapshot,
                            locale_snapshot,
                            language_options: language_options.clone(),
                            line_wrapping_snapshot,
                            editor_search_panel_visible_snapshot,
                            can_undo_snapshot,
                            can_redo_snapshot,
                            file_sheet_open,
                            inspector_sheet_open,
                            loaded_files,
                            next_loaded_file_id,
                            file_selection,
                            encrypt_output,
                            locale,
                            line_wrapping,
                            editor_search_panel_visible,
                            status,
                            open_native_files: EventHandler::new(move |_| open_native_files(())),
                            open_native_folder: EventHandler::new(move |_| open_native_folder(())),
                            on_files_staged: EventHandler::new(move |_| {
                                reveal_file_sheet_after_commit(file_sheet_open, inspector_sheet_open)
                            }),
                            undo_edit,
                            redo_edit,
                            on_switch_mode: EventHandler::new(switch_mode),
                            on_compact_change: EventHandler::new(handle_compact_change),
                            export_text: EventHandler::new(export_text),
                            parse_current: EventHandler::new(move |_| parse_current(())),
                            export_rton: EventHandler::new(move |_| export_rton(()))
                        }
                    },
                }

                WorkspaceCard { class: "rton-editor-card", aria_label: "RTON Editor",
                    TabStrip {
                        tabs: tab_headers.clone(),
                        active_tab_id: active_id_snapshot,
                        dragged_tab_id: dragged_tab_id_snapshot,
                        drop_marker: tab_drop_marker_snapshot,
                        new_tab_mode: new_tab_mode_snapshot,
                        i18n,
                        on_activate: activate_tab,
                        on_close: request_close_tab,
                        on_new: create_blank_tab,
                        on_drag_start: start_tab_drag,
                        on_drop_marker: update_tab_drop_marker,
                        on_drag_end: finish_tab_drag
                    }

                    EditorStage {
                        i18n,
                        active_tab: active_stage_tab_snapshot.clone(),
                        active_byte_doc: active_byte_doc_snapshot.clone(),
                        hex_jump_target: *hex_jump_target.read(),
                        text_jump_target: active_text_jump_target,
                        line_wrapping: line_wrapping_snapshot,
                        editor_search_panel_visible: editor_search_panel_visible_snapshot,
                        editor_search_text: editor_search_text_snapshot.clone(),
                        editor_replace_text: editor_replace_text_snapshot.clone(),
                        editor_search_case_sensitive: editor_search_case_sensitive_snapshot,
                        editor_search_controls_disabled: editor_search.controls_disabled,
                        editor_search_status_text: editor_search.status_text.clone(),
                        on_hex_change: EventHandler::new(update_hex_edit),
                        on_virtual_text_range_replace: EventHandler::new(update_virtual_text_range),
                        on_undo: undo_edit,
                        on_redo: redo_edit,
                        on_search_visible_change: EventHandler::new(move |visible| editor_search_panel_visible.set(visible)),
                        on_find_input: EventHandler::new(move |value: String| {
                            editor_search_text.set(value);
                            editor_search_match_index.set(0);
                        }),
                        on_case_sensitive_change: EventHandler::new(move |checked: bool| {
                            editor_search_case_sensitive.set(checked);
                            editor_search_match_index.set(0);
                        }),
                        on_replace_input: EventHandler::new(move |value: String| editor_replace_text.set(value)),
                        on_previous_match: EventHandler::new(go_to_previous_editor_match),
                        on_next_match: EventHandler::new(go_to_next_editor_match),
                        on_replace_current: EventHandler::new(replace_current_editor_match),
                        on_replace_all: EventHandler::new(replace_all_editor_matches),
                        on_find_key: EventHandler::new(handle_editor_search_key),
                        on_replace_key: EventHandler::new(handle_editor_replace_key),
                        suppress_resize_observer: false
                    }

                    DocumentNotice {
                        i18n,
                        active_file_label: active_file_label.clone(),
                        output_value: output_value_label.clone(),
                        status: status_snapshot
                    }
                }

                if *dragging_files.read() {
                    div { class: "rton-page-drop-indicator",
                        strong { {i18n.t("drop-title")} }
                    }
                }

                ContextSheet {
                    open: *file_sheet_open.read(),
                    side: "left",
                    title: i18n.t("file-list-title"),
                    close_label: i18n.t("toolbar-close-menu"),
                    on_close: move |_| set_file_sheet_open(false, file_sheet_open, inspector_sheet_open),
                    FilePanel {
                        i18n,
                        file_list_subtitle: file_panel.subtitle.clone(),
                        file_search: file_search_snapshot.clone(),
                        file_list_empty_message: file_panel.empty_message.clone(),
                        file_list_items_empty: file_panel.all_items.is_empty(),
                        filtered_file_list_items: file_panel.filtered_items.clone(),
                        file_selection: file_selection_snapshot.clone(),
                        selected_file_count: file_panel.selected_count,
                        selected_visible_file_count: file_panel.selected_visible_count,
                        on_select_all: EventHandler::new(move |_| select_all_visible_files(())),
                        on_clear_selected: EventHandler::new(move |_| clear_selected_files(())),
                        on_search_input: EventHandler::new(move |query: String| file_search_query.set(query)),
                        on_batch_export: batch_export_selected,
                        on_open_file: EventHandler::new(open_loaded_file),
                        on_activate: EventHandler::new(activate_tab),
                        on_remove: EventHandler::new(remove_file_list_item),
                        on_remove_path: EventHandler::new(remove_file_list_path),
                        on_toggle_selected: EventHandler::new(toggle_selected_file),
                        on_toggle_path: EventHandler::new(toggle_selected_path),
                        suppress_resize_observer: false
                    }
                }

                ContextSheet {
                    open: *inspector_sheet_open.read(),
                    title: i18n.t("panel-inspector-tabs"),
                    close_label: i18n.t("toolbar-close-menu"),
                    on_close: move |_| set_inspector_sheet_open(false, file_sheet_open, inspector_sheet_open),
                    IndexPanel {
                        i18n,
                        active_file_id: active_output_source_key.map(|source| source.tab_id),
                        active_file_label: active_file_label.clone(),
                        input_value: input_value_label,
                        output_value: output_value_label.clone(),
                        active_stats: active_stats_snapshot.clone(),
                        has_parsed_document: has_parsed_document_snapshot,
                        active_search: active_search.clone(),
                        selected_path: selected_path_snapshot.clone(),
                        tree_rows: tree_rows.clone(),
                        expanded_paths: expanded_paths_snapshot.clone(),
                        value_search_result,
                        on_search_change: EventHandler::new(move |query: String| update_active_search(query, tabs, active_tab_id)),
                        on_toggle_path: EventHandler::new(move |path: String| toggle_active_tree_path(path, tabs, active_tab_id)),
                        on_select_path: EventHandler::new(select_value_path),
                        suppress_resize_observer: false
                    }
                }

                if let Some(file_name) = pending_close_file_name {
                    UnsavedChangesDialog {
                        file_name,
                        i18n,
                        on_cancel: cancel_pending_close,
                        on_discard: discard_pending_close
                    }
                }
            }
        }
    }
}

fn reveal_file_sheet_after_commit(
    file_sheet_open: Signal<bool>,
    inspector_sheet_open: Signal<bool>,
) {
    if *file_sheet_open.peek() {
        set_file_sheet_open(true, file_sheet_open, inspector_sheet_open);
        return;
    }
    spawn(async move {
        crate::platform::sleep_ms(34).await;
        if !*file_sheet_open.peek() {
            set_file_sheet_open(true, file_sheet_open, inspector_sheet_open);
        }
    });
}

#[derive(Clone, Copy)]
enum SheetSide {
    File,
    Inspector,
}

fn sheet_state_after_change(
    side: SheetSide,
    open: bool,
    file_open: bool,
    inspector_open: bool,
) -> (bool, bool) {
    match side {
        SheetSide::File => (open, if open { false } else { inspector_open }),
        SheetSide::Inspector => (if open { false } else { file_open }, open),
    }
}

fn set_sheet_visibility(
    side: SheetSide,
    open: bool,
    mut file_sheet_open: Signal<bool>,
    mut inspector_sheet_open: Signal<bool>,
) {
    let (next_file_open, next_inspector_open) = sheet_state_after_change(
        side,
        open,
        *file_sheet_open.peek(),
        *inspector_sheet_open.peek(),
    );
    if next_file_open != *file_sheet_open.peek() {
        file_sheet_open.set(next_file_open);
    }
    if next_inspector_open != *inspector_sheet_open.peek() {
        inspector_sheet_open.set(next_inspector_open);
    }
}

pub(super) fn set_file_sheet_open(
    open: bool,
    file_sheet_open: Signal<bool>,
    inspector_sheet_open: Signal<bool>,
) {
    set_sheet_visibility(SheetSide::File, open, file_sheet_open, inspector_sheet_open);
}

pub(super) fn set_inspector_sheet_open(
    open: bool,
    file_sheet_open: Signal<bool>,
    inspector_sheet_open: Signal<bool>,
) {
    set_sheet_visibility(
        SheetSide::Inspector,
        open,
        file_sheet_open,
        inspector_sheet_open,
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn save_desktop_viewport_size(width: f64, height: f64, mut last_saved: Signal<Option<(u32, u32)>>) {
    if !width.is_finite() || !height.is_finite() {
        return;
    }

    let width = width.round().max(0.0) as u32;
    let height = height.round().max(0.0) as u32;
    if width < DESKTOP_WINDOW_MIN_WIDTH || height < DESKTOP_WINDOW_MIN_HEIGHT {
        return;
    }

    let next_size = (width, height);
    if *last_saved.peek() == Some(next_size) {
        return;
    }
    last_saved.set(Some(next_size));

    if let Err(error) = crate::platform::save_window_size_preference(width, height) {
        log::warn!(target: "rton_editor::preferences", "Failed to save window size preference: {error}");
    }
}

fn rton_display_encode_options(compact: bool) -> EncodeOptions {
    EncodeOptions {
        encoding: if compact {
            BinaryEncoding::Compact
        } else {
            BinaryEncoding::Standard
        },
        encrypted: false,
    }
}

fn file_input_summary(
    has_active_file: bool,
    byte_doc: Option<&ByteDocument>,
    text_byte_count: Option<usize>,
    i18n: I18n,
) -> String {
    if !has_active_file {
        return i18n.t("summary-no-output");
    }

    if let Some(byte_doc) = byte_doc {
        return format_panel_bytes(byte_doc.len());
    }

    if text_byte_count.is_some() {
        return i18n.t("summary-text-input");
    }

    i18n.t("summary-not-generated")
}

struct FileOutputSummaryInput<'a> {
    has_active_file: bool,
    active_tab_id: Option<usize>,
    mode: Option<EditorMode>,
    byte_doc: Option<&'a ByteDocument>,
    text_byte_count: Option<usize>,
    compact: bool,
    encrypted: bool,
    rton_output_size: Option<RtonOutputSize>,
    i18n: I18n,
}

fn file_output_summary(input: FileOutputSummaryInput<'_>) -> String {
    let FileOutputSummaryInput {
        has_active_file,
        active_tab_id,
        mode,
        byte_doc,
        text_byte_count,
        compact,
        encrypted,
        rton_output_size,
        i18n,
    } = input;
    if !has_active_file {
        return i18n.t("summary-no-output");
    }

    match mode {
        Some(EditorMode::RtonHex) if encrypted => {
            let encoding = rton_encoding_summary(compact, encrypted, i18n);
            rton_output_size
                .filter(|size| {
                    Some(size.tab_id) == active_tab_id
                        && size.compact == compact
                        && size.encrypted == encrypted
                })
                .map(|size| format!("{} · {encoding} RTON", format_panel_bytes(size.byte_count)))
                .unwrap_or_else(|| format!("{encoding} RTON"))
        }
        Some(EditorMode::RtonHex) => byte_doc
            .map(|bytes| {
                format!(
                    "{} · {} RTON",
                    format_panel_bytes(bytes.len()),
                    rton_encoding_summary(compact, false, i18n)
                )
            })
            .unwrap_or_else(|| i18n.t("summary-not-generated")),
        Some(mode @ (EditorMode::Json | EditorMode::Yaml | EditorMode::Toml)) => text_byte_count
            .filter(|byte_count| *byte_count > 0)
            .map(|byte_count| format!("{} · {}", format_panel_bytes(byte_count), mode.label()))
            .unwrap_or_else(|| i18n.t("summary-not-generated")),
        None => i18n.t("summary-no-output"),
    }
}

fn rton_encoding_summary(compact: bool, encrypted: bool, i18n: I18n) -> String {
    let base = if compact { "Compact" } else { "Standard" };
    if encrypted {
        format!("{base} · {}", i18n.t("toolbar-encrypted"))
    } else {
        base.to_string()
    }
}

fn format_panel_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / 1024.0 / 1024.0)
    }
}

fn current_text_search_jump(
    buffer: &TextBuffer,
    matches: &[TextSearchMatch],
    match_index: usize,
    focus_token: u64,
) -> Option<crate::components::TextJumpTarget> {
    let match_ = matches.get(match_index.min(matches.len().saturating_sub(1)))?;
    let position = offset_to_text_buffer_position(buffer, match_.start);
    let end_position = offset_to_text_buffer_position(buffer, match_.end);
    let selection_end_column = if end_position.line == position.line {
        end_position.column
    } else {
        position.column
    };
    Some(crate::components::TextJumpTarget {
        id: focus_token,
        line: position.line,
        column: position.column,
        selection_end_column,
        line_count: buffer.line_count(),
        focus: focus_token > 0,
    })
}

fn request_text_search_focus(match_count: usize, mut focus_token: Signal<u64>) {
    if match_count > 0 {
        let next = focus_token.read().saturating_add(1);
        focus_token.set(next);
    }
}

fn offset_to_text_buffer_position(
    buffer: &TextBuffer,
    offset: usize,
) -> crate::domain::TextPosition {
    let bounded_offset = offset.min(buffer.byte_count());
    let line_index = buffer.byte_to_line(bounded_offset);
    let line_start = buffer.line_start_byte(line_index).unwrap_or(0);
    crate::domain::TextPosition {
        line: line_index + 1,
        column: bounded_offset.saturating_sub(line_start),
    }
}

#[cfg(test)]
mod sheet_layout_tests {
    use super::{SheetSide, sheet_state_after_change};

    #[test]
    fn context_sheets_are_mutually_exclusive() {
        assert_eq!(
            sheet_state_after_change(SheetSide::File, true, false, true),
            (true, false)
        );
        assert_eq!(
            sheet_state_after_change(SheetSide::Inspector, true, true, false),
            (false, true)
        );
    }

    #[test]
    fn closing_one_sheet_preserves_the_other() {
        assert_eq!(
            sheet_state_after_change(SheetSide::File, false, true, true),
            (false, true)
        );
        assert_eq!(
            sheet_state_after_change(SheetSide::Inspector, false, true, true),
            (true, false)
        );
    }
}
