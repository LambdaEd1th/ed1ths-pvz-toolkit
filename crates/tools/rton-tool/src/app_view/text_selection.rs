use crate::domain::TextBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VirtualTextSelection {
    pub(super) start_line: usize,
    pub(super) start_column_utf16: usize,
    pub(super) end_line: usize,
    pub(super) end_column_utf16: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct VirtualTextPoint {
    pub(super) line_index: usize,
    pub(super) column_utf16: usize,
}

pub(super) fn virtual_text_selection_from_points(
    anchor: VirtualTextPoint,
    focus: VirtualTextPoint,
) -> VirtualTextSelection {
    VirtualTextSelection {
        start_line: anchor.line_index,
        start_column_utf16: anchor.column_utf16,
        end_line: focus.line_index,
        end_column_utf16: focus.column_utf16,
    }
}

pub(super) fn virtual_text_selection_from_distinct_points(
    anchor: VirtualTextPoint,
    focus: VirtualTextPoint,
) -> Option<VirtualTextSelection> {
    (anchor != focus).then(|| virtual_text_selection_from_points(anchor, focus))
}

pub(super) fn virtual_text_selection_anchor(selection: VirtualTextSelection) -> VirtualTextPoint {
    VirtualTextPoint {
        line_index: selection.start_line,
        column_utf16: selection.start_column_utf16,
    }
}

pub(super) fn virtual_text_selection_focus(selection: VirtualTextSelection) -> VirtualTextPoint {
    VirtualTextPoint {
        line_index: selection.end_line,
        column_utf16: selection.end_column_utf16,
    }
}

pub(super) fn virtual_text_utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

pub(super) fn virtual_text_selected_text(
    buffer: &TextBuffer,
    selection: VirtualTextSelection,
) -> Option<String> {
    let (start, _, end, _) = normalized_virtual_text_selection_points(selection);
    let start = virtual_text_point_to_byte_offset(buffer, start)?;
    let end = virtual_text_point_to_byte_offset(buffer, end)?;
    (start < end)
        .then(|| buffer.byte_slice(start, end))
        .flatten()
}

pub(super) fn virtual_text_line_selection_columns(
    selection: Option<VirtualTextSelection>,
    line_index: usize,
    line_text: &str,
) -> Option<(usize, usize)> {
    let selection = selection?;
    let (start_line, start_column, end_line, end_column) =
        normalize_virtual_text_selection(selection);
    if line_index < start_line || line_index > end_line {
        return None;
    }

    let start = if line_index == start_line {
        virtual_text_utf16_column_to_byte(line_text, start_column)
    } else {
        0
    };
    let end = if line_index == end_line {
        virtual_text_utf16_column_to_byte(line_text, end_column)
    } else {
        line_text.len()
    };
    (start < end).then_some((start, end))
}

pub(super) fn normalize_virtual_text_selection(
    selection: VirtualTextSelection,
) -> (usize, usize, usize, usize) {
    let start = (selection.start_line, selection.start_column_utf16);
    let end = (selection.end_line, selection.end_column_utf16);
    if start <= end {
        (
            selection.start_line,
            selection.start_column_utf16,
            selection.end_line,
            selection.end_column_utf16,
        )
    } else {
        (
            selection.end_line,
            selection.end_column_utf16,
            selection.start_line,
            selection.start_column_utf16,
        )
    }
}

pub(super) fn normalized_virtual_text_selection_points(
    selection: VirtualTextSelection,
) -> (VirtualTextPoint, usize, VirtualTextPoint, usize) {
    let (start_line, start_column, end_line, end_column) =
        normalize_virtual_text_selection(selection);
    (
        VirtualTextPoint {
            line_index: start_line,
            column_utf16: start_column,
        },
        start_column,
        VirtualTextPoint {
            line_index: end_line,
            column_utf16: end_column,
        },
        end_column,
    )
}

pub(super) fn clamp_virtual_text_selection(
    selection: VirtualTextSelection,
    buffer: &TextBuffer,
) -> Option<VirtualTextSelection> {
    let anchor = clamp_virtual_text_point(virtual_text_selection_anchor(selection), buffer);
    let focus = clamp_virtual_text_point(virtual_text_selection_focus(selection), buffer);
    virtual_text_selection_from_distinct_points(anchor, focus)
}

pub(super) fn clamp_virtual_text_point(
    point: VirtualTextPoint,
    buffer: &TextBuffer,
) -> VirtualTextPoint {
    if buffer.line_count() == 0 {
        return VirtualTextPoint {
            line_index: 0,
            column_utf16: 0,
        };
    }
    let line_index = point.line_index.min(buffer.line_count().saturating_sub(1));
    VirtualTextPoint {
        line_index,
        column_utf16: point
            .column_utf16
            .min(virtual_text_line_utf16_len(buffer, line_index)),
    }
}

pub(super) fn virtual_text_document_end_point(buffer: &TextBuffer) -> VirtualTextPoint {
    if buffer.line_count() == 0 {
        return VirtualTextPoint {
            line_index: 0,
            column_utf16: 0,
        };
    }
    let line_index = buffer.line_count() - 1;
    VirtualTextPoint {
        line_index,
        column_utf16: virtual_text_line_utf16_len(buffer, line_index),
    }
}

pub(super) fn virtual_text_line_utf16_len(buffer: &TextBuffer, line_index: usize) -> usize {
    buffer
        .line_text(line_index)
        .map(|line| virtual_text_utf16_len(&line))
        .unwrap_or(0)
}

pub(super) fn virtual_text_point_from_jump_target(
    buffer: &TextBuffer,
    line: usize,
    byte_column: usize,
) -> VirtualTextPoint {
    let line_index = line
        .saturating_sub(1)
        .min(buffer.line_count().saturating_sub(1));
    let line_text = buffer.line_text(line_index).unwrap_or_default();
    clamp_virtual_text_point(
        VirtualTextPoint {
            line_index,
            column_utf16: virtual_text_byte_column_to_utf16(&line_text, byte_column),
        },
        buffer,
    )
}

pub(super) fn virtual_text_byte_column_to_utf16(text: &str, byte_column: usize) -> usize {
    let bounded = byte_column.min(text.len());
    let mut utf16_offset = 0_usize;
    for (byte_offset, ch) in text.char_indices() {
        if byte_offset >= bounded {
            return utf16_offset;
        }
        let next_byte = byte_offset.saturating_add(ch.len_utf8());
        if next_byte > bounded {
            return utf16_offset;
        }
        utf16_offset = utf16_offset.saturating_add(ch.len_utf16());
    }
    utf16_offset
}

pub(super) fn virtual_text_point_to_byte_offset(
    buffer: &TextBuffer,
    point: VirtualTextPoint,
) -> Option<usize> {
    let point = clamp_virtual_text_point(point, buffer);
    let start = buffer.line_start_byte(point.line_index)?;
    let line = buffer.line_text(point.line_index)?;
    Some(start + virtual_text_utf16_column_to_byte(&line, point.column_utf16))
}

pub(super) fn virtual_text_utf16_column_to_byte(text: &str, column_utf16: usize) -> usize {
    let mut utf16_offset = 0_usize;
    for (byte_offset, ch) in text.char_indices() {
        let next_utf16_offset = utf16_offset.saturating_add(ch.len_utf16());
        if next_utf16_offset > column_utf16 {
            return byte_offset;
        }
        utf16_offset = next_utf16_offset;
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_and_utf16_columns_round_trip_at_character_boundaries() {
        let text = "a中𐐷b";
        for byte in [0, 1, 4, 8, 9] {
            let utf16 = virtual_text_byte_column_to_utf16(text, byte);
            assert_eq!(virtual_text_utf16_column_to_byte(text, utf16), byte);
        }
    }
}
