use super::edit::HexEdit;

pub(crate) fn replace_byte_span(
    bytes: &[u8],
    offset: usize,
    delete_length: usize,
    values: &[u8],
) -> Vec<u8> {
    let safe_offset = offset.min(bytes.len());
    let safe_delete_length = delete_length.min(bytes.len().saturating_sub(safe_offset));
    let mut next = Vec::with_capacity(bytes.len() - safe_delete_length + values.len());
    next.extend_from_slice(&bytes[..safe_offset]);
    next.extend_from_slice(values);
    next.extend_from_slice(&bytes[safe_offset + safe_delete_length..]);
    next
}

pub(crate) fn apply_hex_edit(bytes: &[u8], edit: &HexEdit) -> Vec<u8> {
    replace_byte_span(bytes, edit.offset, edit.delete_length, &edit.insert)
}

pub(crate) fn apply_hex_edits(bytes: &[u8], edits: &[HexEdit]) -> Vec<u8> {
    edits.iter().fold(bytes.to_vec(), |current, edit| {
        apply_hex_edit(&current, edit)
    })
}

pub(crate) fn overwrite_byte_range(bytes: &[u8], offset: usize, values: &[u8]) -> Vec<u8> {
    if values.is_empty() || offset >= bytes.len() {
        return bytes.to_vec();
    }
    let mut next = bytes.to_vec();
    let writable = values.len().min(bytes.len() - offset);
    next[offset..offset + writable].copy_from_slice(&values[..writable]);
    next
}
