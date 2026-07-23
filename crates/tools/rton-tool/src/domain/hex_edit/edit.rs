#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HexEdit {
    pub(crate) offset: usize,
    pub(crate) delete_length: usize,
    pub(crate) insert: Vec<u8>,
}

pub(crate) fn edited_len(current_len: usize, edit: &HexEdit) -> usize {
    let offset = edit.offset.min(current_len);
    let deleted = edit.delete_length.min(current_len.saturating_sub(offset));
    current_len - deleted + edit.insert.len()
}

pub(crate) fn edited_len_after_edits(current_len: usize, edits: &[HexEdit]) -> usize {
    edits.iter().fold(current_len, edited_len)
}
