use std::sync::Arc;

use super::FileListItem;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct FileSelection {
    rules: Vec<FileSelectionRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileSelectionRule {
    effect: SelectionEffect,
    scope: SelectionScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionEffect {
    Include,
    Exclude,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SelectionScope {
    Key(Arc<str>),
    Path(Arc<str>),
    Search(Arc<str>),
}

impl FileSelection {
    pub(crate) fn clear(&mut self) {
        self.rules.clear();
    }

    pub(crate) fn is_selected(&self, item: &FileListItem) -> bool {
        self.rules
            .iter()
            .rev()
            .find_map(|rule| {
                rule.scope.matches(item).then_some(match rule.effect {
                    SelectionEffect::Include => true,
                    SelectionEffect::Exclude => false,
                })
            })
            .unwrap_or(false)
    }

    pub(crate) fn selected_count(&self, items: &[FileListItem]) -> usize {
        items.iter().filter(|item| self.is_selected(item)).count()
    }

    pub(crate) fn selected_items(&self, items: &[FileListItem]) -> Vec<FileListItem> {
        items
            .iter()
            .filter(|item| self.is_selected(item))
            .cloned()
            .collect()
    }

    pub(crate) fn select_visible(&mut self, search_query: &str) {
        self.push(
            SelectionEffect::Include,
            SelectionScope::search(search_query),
        );
    }

    pub(crate) fn clear_visible(&mut self, search_query: &str) {
        self.push(
            SelectionEffect::Exclude,
            SelectionScope::search(search_query),
        );
    }

    pub(crate) fn toggle_key(&mut self, key: String, checked: bool) {
        self.push(
            effect_for_checked(checked),
            SelectionScope::Key(Arc::from(key)),
        );
    }

    pub(crate) fn toggle_path(&mut self, path: String, checked: bool) {
        self.push(
            effect_for_checked(checked),
            SelectionScope::Path(Arc::from(normalize_display_path(&path))),
        );
    }

    fn push(&mut self, effect: SelectionEffect, scope: SelectionScope) {
        self.rules.push(FileSelectionRule { effect, scope });
        if self.rules.len() > 256 {
            compact_tail_rules(&mut self.rules);
        }
    }
}

impl SelectionScope {
    fn search(search_query: &str) -> Self {
        Self::Search(Arc::from(normalize_search_query(search_query)))
    }

    fn matches(&self, item: &FileListItem) -> bool {
        match self {
            Self::Key(key) => item.key.as_str() == key.as_ref(),
            Self::Path(path) => file_path_matches_scope(&item.path, path),
            Self::Search(query) => file_item_matches_search(item, query),
        }
    }
}

fn effect_for_checked(checked: bool) -> SelectionEffect {
    if checked {
        SelectionEffect::Include
    } else {
        SelectionEffect::Exclude
    }
}

fn compact_tail_rules(rules: &mut Vec<FileSelectionRule>) {
    let keep_from = rules.len().saturating_sub(128);
    rules.drain(0..keep_from);
}

pub(crate) fn file_item_matches_search(item: &FileListItem, search_query: &str) -> bool {
    let needle = normalize_search_query(search_query);
    if needle.is_empty() {
        return true;
    }

    item.path.to_ascii_lowercase().contains(&needle)
        || item.name.to_ascii_lowercase().contains(&needle)
        || item.detail.to_ascii_lowercase().contains(&needle)
}

pub(crate) fn file_path_matches_scope(path: &str, scope_path: &str) -> bool {
    let path = normalize_display_path(path);
    let scope = normalize_display_path(scope_path);
    path == scope
        || path
            .strip_prefix(scope.as_str())
            .is_some_and(|rest| rest.starts_with('/'))
}

fn normalize_search_query(search_query: &str) -> String {
    search_query.trim().to_ascii_lowercase()
}

fn normalize_display_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string()
}
