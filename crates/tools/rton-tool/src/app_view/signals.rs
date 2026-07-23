use dioxus::prelude::*;

use crate::components::{FileSelection, HexJumpTarget, TextJumpTarget};
use crate::domain::{DropMarker, EditorMode, EditorTabState, Status, ThemePreference, Tone};
use crate::file_import::LoadedFileState;
use crate::i18n::{I18n, Locale};
use crate::platform;

#[derive(Clone, Copy, PartialEq)]
pub(super) struct WorkspaceSignals {
    pub(super) tabs: Signal<Vec<EditorTabState>>,
    pub(super) loaded_files: Signal<Vec<LoadedFileState>>,
    pub(super) active_tab_id: Signal<usize>,
    pub(super) next_tab_id: Signal<usize>,
    pub(super) next_loaded_file_id: Signal<usize>,
    pub(super) file_search_query: Signal<String>,
    pub(super) file_selection: Signal<FileSelection>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct PreferenceSignals {
    pub(super) compact_output: Signal<bool>,
    pub(super) encrypt_output: Signal<bool>,
    pub(super) preferred_editor_mode: Signal<Option<EditorMode>>,
    pub(super) theme_preference: Signal<ThemePreference>,
    pub(super) locale: Signal<Locale>,
    pub(super) i18n_revision: Signal<u64>,
    #[cfg(target_arch = "wasm32")]
    pub(super) i18n_loaded: Signal<bool>,
    pub(super) line_wrapping: Signal<bool>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct EditorSessionSignals {
    pub(super) editor_search_panel_visible: Signal<bool>,
    pub(super) editor_search_text: Signal<String>,
    pub(super) editor_replace_text: Signal<String>,
    pub(super) editor_search_case_sensitive: Signal<bool>,
    pub(super) editor_search_match_index: Signal<usize>,
    pub(super) editor_search_focus_token: Signal<u64>,
    pub(super) next_jump_id: Signal<u64>,
    pub(super) text_jump_target: Signal<Option<TextJumpTarget>>,
    pub(super) hex_jump_target: Signal<Option<HexJumpTarget>>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct LayoutSignals {
    pub(super) dragging_files: Signal<bool>,
    pub(super) file_sheet_open: Signal<bool>,
    pub(super) inspector_sheet_open: Signal<bool>,
    pub(super) dragged_tab_id: Signal<Option<usize>>,
    pub(super) tab_drop_marker: Signal<Option<DropMarker<usize>>>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) struct AppSignals {
    pub(super) workspace: WorkspaceSignals,
    pub(super) preferences: PreferenceSignals,
    pub(super) editor: EditorSessionSignals,
    pub(super) layout: LayoutSignals,
    pub(super) status: Signal<Status>,
}

pub(super) fn use_app_signals(
    initial_locale_snapshot: Locale,
    theme: ThemePreference,
) -> AppSignals {
    AppSignals {
        workspace: WorkspaceSignals {
            tabs: use_signal(Vec::<EditorTabState>::new),
            loaded_files: use_signal(Vec::<LoadedFileState>::new),
            active_tab_id: use_signal(|| 0_usize),
            next_tab_id: use_signal(|| 1_usize),
            next_loaded_file_id: use_signal(|| 1_usize),
            file_search_query: use_signal(String::new),
            file_selection: use_signal(FileSelection::default),
        },
        preferences: PreferenceSignals {
            compact_output: use_signal(|| false),
            encrypt_output: use_signal(|| false),
            preferred_editor_mode: use_signal(platform::read_editor_mode_preference),
            theme_preference: use_signal(move || theme),
            locale: use_signal(move || initial_locale_snapshot),
            i18n_revision: use_signal(|| 0_u64),
            #[cfg(target_arch = "wasm32")]
            i18n_loaded: use_signal(|| false),
            line_wrapping: use_signal(platform::read_line_wrapping_preference),
        },
        editor: EditorSessionSignals {
            editor_search_panel_visible: use_signal(|| false),
            editor_search_text: use_signal(String::new),
            editor_replace_text: use_signal(String::new),
            editor_search_case_sensitive: use_signal(|| false),
            editor_search_match_index: use_signal(|| 0_usize),
            editor_search_focus_token: use_signal(|| 0_u64),
            next_jump_id: use_signal(|| 1_u64),
            text_jump_target: use_signal(|| None::<TextJumpTarget>),
            hex_jump_target: use_signal(|| None::<HexJumpTarget>),
        },
        layout: LayoutSignals {
            dragging_files: use_signal(|| false),
            file_sheet_open: use_signal(|| false),
            inspector_sheet_open: use_signal(|| false),
            dragged_tab_id: use_signal(|| None::<usize>),
            tab_drop_marker: use_signal(|| None::<DropMarker<usize>>),
        },
        status: use_signal(move || {
            Status::new(
                I18n::new(initial_locale_snapshot).t("status-ready"),
                Tone::Ok,
            )
        }),
    }
}
