use dioxus::prelude::*;

use crate::domain::{
    EditorTabState, TextSearchMatch, next_text_search_index, previous_text_search_index,
    replace_all_text_query, replace_text_span,
};

use super::{editing::update_active_text, tabs::active_tab};

pub(crate) fn go_to_previous_text_match(
    match_count: usize,
    mut editor_search_match_index: Signal<usize>,
) {
    if match_count == 0 {
        return;
    }
    let current = *editor_search_match_index.read();
    editor_search_match_index.set(previous_text_search_index(current, match_count));
}

pub(crate) fn go_to_next_text_match(
    match_count: usize,
    mut editor_search_match_index: Signal<usize>,
) {
    if match_count == 0 {
        return;
    }
    let current = *editor_search_match_index.read();
    editor_search_match_index.set(next_text_search_index(current, match_count));
}

pub(crate) fn replace_current_text_match(
    matches: &[TextSearchMatch],
    replacement: &str,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    mut editor_search_match_index: Signal<usize>,
) {
    if matches.is_empty() {
        return;
    }
    let Some(current_text) = active_tab_text(tabs, active_tab_id) else {
        return;
    };
    let index = (*editor_search_match_index.read()).min(matches.len() - 1);
    let match_ = matches[index];
    let next_text = replace_text_span(&current_text, match_.start, match_.end, replacement);
    if next_text != current_text {
        update_active_text(next_text, tabs, active_tab_id);
    }
    editor_search_match_index.set(index);
}

pub(crate) fn replace_all_text_matches_in_editor(
    query: &str,
    replacement: &str,
    case_sensitive: bool,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    mut editor_search_match_index: Signal<usize>,
) {
    if query.is_empty() {
        return;
    }
    let Some(current_text) = active_tab_text(tabs, active_tab_id) else {
        return;
    };
    let next_text = replace_all_text_query(&current_text, query, replacement, case_sensitive);
    if next_text != current_text {
        update_active_text(next_text, tabs, active_tab_id);
    }
    editor_search_match_index.set(0);
}

pub(crate) fn handle_text_find_key(
    event: KeyboardEvent,
    match_count: usize,
    mut editor_search_panel_visible: Signal<bool>,
    editor_search_match_index: Signal<usize>,
) -> bool {
    let key = event.key().to_string();
    if key == "Escape" {
        event.prevent_default();
        editor_search_panel_visible.set(false);
        false
    } else if key == "Enter" && match_count > 0 {
        event.prevent_default();
        if event.modifiers().shift() {
            go_to_previous_text_match(match_count, editor_search_match_index);
        } else {
            go_to_next_text_match(match_count, editor_search_match_index);
        }
        true
    } else {
        false
    }
}

pub(crate) fn handle_text_replace_key(
    event: KeyboardEvent,
    matches: &[TextSearchMatch],
    replacement: &str,
    mut editor_search_panel_visible: Signal<bool>,
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
    editor_search_match_index: Signal<usize>,
) {
    let key = event.key().to_string();
    if key == "Escape" {
        event.prevent_default();
        editor_search_panel_visible.set(false);
    } else if key == "Enter" && !matches.is_empty() {
        event.prevent_default();
        replace_current_text_match(
            matches,
            replacement,
            tabs,
            active_tab_id,
            editor_search_match_index,
        );
    }
}

fn active_tab_text(
    tabs: Signal<Vec<EditorTabState>>,
    active_tab_id: Signal<usize>,
) -> Option<String> {
    let active = active_tab(tabs, active_tab_id)?;
    active.mode.text_format().is_some().then(|| {
        active
            .text_buffer
            .as_ref()
            .map(|buffer| buffer.materialize())
            .unwrap_or_else(|| active.editor_text.to_string())
    })
}
