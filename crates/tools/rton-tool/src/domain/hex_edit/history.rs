use super::edit::HexEdit;
use crate::domain::byte_document::ByteDocument;

const HEX_UNDO_HISTORY_LIMIT: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HexHistory {
    pub(crate) past: Vec<HexUndoBatch>,
    pub(crate) future: Vec<HexUndoBatch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HexUndoEdit {
    offset: usize,
    deleted: Vec<u8>,
    inserted: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HexUndoBatch {
    edits: Vec<HexUndoEdit>,
}

impl HexUndoEdit {
    pub(crate) fn from_edit(current: &ByteDocument, edit: &HexEdit) -> Self {
        let start = edit.offset.min(current.len());
        let end = start.saturating_add(edit.delete_length).min(current.len());
        Self {
            offset: start,
            deleted: current.range_to_vec(start, end).unwrap_or_default(),
            inserted: edit.insert.clone(),
        }
    }

    pub(crate) fn redo_edit(&self) -> HexEdit {
        HexEdit {
            offset: self.offset,
            delete_length: self.deleted.len(),
            insert: self.inserted.clone(),
        }
    }

    pub(crate) fn undo_edit(&self) -> HexEdit {
        HexEdit {
            offset: self.offset,
            delete_length: self.inserted.len(),
            insert: self.deleted.clone(),
        }
    }
}

impl HexUndoBatch {
    pub(crate) fn redo_edits(&self) -> Vec<HexEdit> {
        self.edits
            .iter()
            .map(HexUndoEdit::redo_edit)
            .collect::<Vec<_>>()
    }

    pub(crate) fn undo_edits(&self) -> Vec<HexEdit> {
        self.edits
            .iter()
            .rev()
            .map(HexUndoEdit::undo_edit)
            .collect::<Vec<_>>()
    }
}

pub(crate) fn prepare_hex_edits(
    current: &ByteDocument,
    edits: Vec<HexEdit>,
) -> Option<(Vec<HexEdit>, HexUndoBatch)> {
    let mut document = current.clone();
    let mut effective_edits = Vec::new();
    let mut undo_edits = Vec::new();

    for edit in edits {
        if edit.delete_length == 0 && edit.insert.is_empty() {
            continue;
        }
        let undo = HexUndoEdit::from_edit(&document, &edit);
        if undo.deleted == undo.inserted {
            continue;
        }
        document = document.apply_edit(&edit);
        effective_edits.push(edit);
        undo_edits.push(undo);
    }

    (!effective_edits.is_empty()).then_some((effective_edits, HexUndoBatch { edits: undo_edits }))
}

pub(crate) fn push_hex_undo_batch(history: &mut HexHistory, batch: HexUndoBatch) {
    history.past.push(batch);
    trim_hex_history(&mut history.past);
    history.future.clear();
}

pub(crate) fn push_hex_redo_batch(history: &mut HexHistory, batch: HexUndoBatch) {
    history.future.push(batch);
    trim_hex_history(&mut history.future);
}

pub(crate) fn push_hex_past_batch(history: &mut HexHistory, batch: HexUndoBatch) {
    history.past.push(batch);
    trim_hex_history(&mut history.past);
}

fn trim_hex_history(items: &mut Vec<HexUndoBatch>) {
    if items.len() > HEX_UNDO_HISTORY_LIMIT {
        let excess = items.len() - HEX_UNDO_HISTORY_LIMIT;
        items.drain(0..excess);
    }
}
