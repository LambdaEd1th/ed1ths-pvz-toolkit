#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextJumpTarget {
    pub(crate) id: u64,
    pub(crate) line: usize,
    pub(crate) column: usize,
    pub(crate) selection_end_column: usize,
    pub(crate) line_count: usize,
    pub(crate) focus: bool,
}
