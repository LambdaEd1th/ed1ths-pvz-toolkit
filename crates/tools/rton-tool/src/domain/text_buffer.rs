use ropey::Rope;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextBuffer {
    rope: Rope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextRangeReplacement {
    pub(crate) start_line: usize,
    pub(crate) start_column_utf16: usize,
    pub(crate) end_line: usize,
    pub(crate) end_column_utf16: usize,
    pub(crate) replacement: String,
}

impl TextBuffer {
    pub(crate) fn new(text: String) -> Self {
        Self {
            rope: Rope::from_str(&text),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn from_arc(text: Arc<str>) -> Self {
        Self {
            rope: Rope::from_str(&text),
        }
    }

    pub(crate) fn line_count(&self) -> usize {
        if self.rope.len_bytes() == 0 {
            0
        } else if self.rope.byte(self.rope.len_bytes() - 1) == b'\n' {
            self.rope.len_lines().saturating_sub(1)
        } else {
            self.rope.len_lines()
        }
    }

    pub(crate) fn byte_count(&self) -> usize {
        self.rope.len_bytes()
    }

    pub(crate) fn materialize(&self) -> String {
        self.rope.to_string()
    }

    pub(crate) fn line_text(&self, line_index: usize) -> Option<String> {
        if line_index >= self.line_count() {
            return None;
        }
        let mut text = self.rope.line(line_index).to_string();
        if text.ends_with('\n') {
            text.pop();
            if text.ends_with('\r') {
                text.pop();
            }
        }
        Some(text)
    }

    pub(crate) fn line_byte_len(&self, line_index: usize) -> Option<usize> {
        if line_index >= self.line_count() {
            return None;
        }
        let line = self.rope.line(line_index);
        let mut len = line.len_bytes();
        if len > 0 && line.byte(len - 1) == b'\n' {
            len -= 1;
            if len > 0 && line.byte(len - 1) == b'\r' {
                len -= 1;
            }
        }
        Some(len)
    }

    pub(crate) fn line_start_byte(&self, line_index: usize) -> Option<usize> {
        (line_index < self.line_count()).then(|| self.rope.line_to_byte(line_index))
    }

    pub(crate) fn byte_to_line(&self, byte_offset: usize) -> usize {
        self.rope
            .byte_to_line(byte_offset.min(self.rope.len_bytes()))
    }

    pub(crate) fn replace_byte_range(
        &self,
        start: usize,
        end: usize,
        replacement: &str,
    ) -> Option<Self> {
        if start > end || end > self.rope.len_bytes() {
            return None;
        }
        let start_char = self.rope.byte_to_char(start);
        let end_char = self.rope.byte_to_char(end);
        let mut rope = self.rope.clone();
        rope.remove(start_char..end_char);
        rope.insert(start_char, replacement);
        Some(Self { rope })
    }

    pub(crate) fn byte_slice(&self, start: usize, end: usize) -> Option<String> {
        if start > end || end > self.rope.len_bytes() {
            return None;
        }
        Some(
            self.rope
                .slice(self.rope.byte_to_char(start)..self.rope.byte_to_char(end))
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_semantics_match_editor_rows() {
        assert_eq!(TextBuffer::new(String::new()).line_count(), 0);
        assert_eq!(TextBuffer::new("a\n".to_string()).line_count(), 1);
        assert_eq!(TextBuffer::new("a\nb".to_string()).line_count(), 2);
    }

    #[test]
    fn line_byte_lengths_exclude_line_endings() {
        let buffer = TextBuffer::new("one\r\ntwo\nthree".to_string());
        assert_eq!(buffer.line_byte_len(0), Some(3));
        assert_eq!(buffer.line_byte_len(1), Some(3));
        assert_eq!(buffer.line_byte_len(2), Some(5));
        assert_eq!(buffer.line_byte_len(3), None);
    }

    #[test]
    fn range_replacement_preserves_utf8_boundaries() {
        let buffer = TextBuffer::new("a中b".to_string());
        let next = buffer
            .replace_byte_range(1, 4, "文")
            .expect("valid UTF-8 byte range");
        assert_eq!(next.materialize(), "a文b");
    }
}
