#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DropPlacement {
    Before,
    After,
}

impl DropPlacement {
    pub(crate) fn class(self) -> &'static str {
        match self {
            DropPlacement::Before => "drop-before",
            DropPlacement::After => "drop-after",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DropMarker<T> {
    pub(crate) id: T,
    pub(crate) placement: DropPlacement,
}
