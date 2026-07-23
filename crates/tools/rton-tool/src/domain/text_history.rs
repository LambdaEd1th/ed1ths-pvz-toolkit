use super::TextRangeReplacement;

const TEXT_UNDO_HISTORY_LIMIT: usize = 64;
const TEXT_UNDO_SNAPSHOT_BYTE_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TextHistory {
    pub(crate) past: Vec<TextHistoryEntry>,
    pub(crate) future: Vec<TextHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextHistoryEntry {
    Snapshot(String),
    Range(TextRangeHistoryEntry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextRangeHistoryEntry {
    pub(crate) undo: TextRangeReplacement,
    pub(crate) redo: TextRangeReplacement,
}

pub(crate) fn can_record_text_undo(previous: &str, next: &str) -> bool {
    previous.len() <= TEXT_UNDO_SNAPSHOT_BYTE_LIMIT && next.len() <= TEXT_UNDO_SNAPSHOT_BYTE_LIMIT
}

pub(crate) fn push_text_undo_snapshot(history: &mut TextHistory, text: String) {
    history.past.push(TextHistoryEntry::Snapshot(text));
    trim_history(&mut history.past);
    history.future.clear();
}

pub(crate) fn push_text_redo_snapshot(history: &mut TextHistory, text: String) {
    history.future.push(TextHistoryEntry::Snapshot(text));
    trim_history(&mut history.future);
}

pub(crate) fn push_text_past_snapshot(history: &mut TextHistory, text: String) {
    history.past.push(TextHistoryEntry::Snapshot(text));
    trim_history(&mut history.past);
}

pub(crate) fn push_text_undo_range(history: &mut TextHistory, entry: TextRangeHistoryEntry) {
    history.past.push(TextHistoryEntry::Range(entry));
    trim_history(&mut history.past);
    history.future.clear();
}

pub(crate) fn push_text_redo_range(history: &mut TextHistory, entry: TextRangeHistoryEntry) {
    history.future.push(TextHistoryEntry::Range(entry));
    trim_history(&mut history.future);
}

pub(crate) fn push_text_past_range(history: &mut TextHistory, entry: TextRangeHistoryEntry) {
    history.past.push(TextHistoryEntry::Range(entry));
    trim_history(&mut history.past);
}

fn trim_history<T>(items: &mut Vec<T>) {
    if items.len() > TEXT_UNDO_HISTORY_LIMIT {
        let excess = items.len() - TEXT_UNDO_HISTORY_LIMIT;
        items.drain(0..excess);
    }
}
