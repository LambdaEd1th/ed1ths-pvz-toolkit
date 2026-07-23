pub(crate) const HEX_ROW_HEIGHT: usize = 28;
pub(crate) const HEX_VISIBLE_ROWS: usize = 36;
pub(crate) const HEX_OVERSCAN_ROWS: usize = 8;
pub(crate) const HEX_MAX_VIEWPORT_ROWS: usize = 128;
pub(crate) const HEX_MAX_VIRTUAL_SCROLL_HEIGHT: usize = 16_000_000;
pub(crate) const HEX_DEFAULT_VIEWPORT_HEIGHT: usize = HEX_VISIBLE_ROWS * HEX_ROW_HEIGHT;

pub(crate) const VALUE_TREE_ROW_HEIGHT: usize = 28;
pub(crate) const VALUE_TREE_OVERSCAN_ROWS: usize = 16;
pub(crate) const VALUE_TREE_MAX_VIEWPORT_ROWS: usize = 192;
pub(crate) const VALUE_TREE_MAX_VIRTUAL_SCROLL_HEIGHT: usize = 16_000_000;
pub(crate) const VALUE_TREE_DEFAULT_VIEWPORT_HEIGHT: usize = 420;

pub(crate) const VALUE_SEARCH_ROW_HEIGHT: usize = 44;
pub(crate) const VALUE_SEARCH_OVERSCAN_ROWS: usize = 12;
pub(crate) const VALUE_SEARCH_MAX_VIEWPORT_ROWS: usize = 128;
pub(crate) const VALUE_SEARCH_MAX_VIRTUAL_SCROLL_HEIGHT: usize = 16_000_000;
pub(crate) const VALUE_SEARCH_DEFAULT_VIEWPORT_HEIGHT: usize = 420;

pub(crate) const TEXT_ROW_HEIGHT: usize = 21;
pub(crate) const TEXT_OVERSCAN_ROWS: usize = 24;
pub(crate) const TEXT_MAX_VIEWPORT_ROWS: usize = 256;
pub(crate) const TEXT_MAX_VIRTUAL_SCROLL_HEIGHT: usize = 16_000_000;
pub(crate) const TEXT_DEFAULT_VIEWPORT_HEIGHT: usize = 700;

pub(crate) const FILE_LIST_ROW_HEIGHT: usize = 54;
pub(crate) const FILE_LIST_OVERSCAN_ROWS: usize = 16;
pub(crate) const FILE_LIST_MAX_VIEWPORT_ROWS: usize = 160;
pub(crate) const FILE_LIST_MAX_VIRTUAL_SCROLL_HEIGHT: usize = 16_000_000;
pub(crate) const FILE_LIST_DEFAULT_VIEWPORT_HEIGHT: usize = 520;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HexVirtualScroll {
    pub(crate) content_height: usize,
    pub(crate) viewport_content_height: usize,
    pub(crate) scaled: bool,
    pub(crate) viewport_start_row: usize,
    pub(crate) start_row: usize,
    pub(crate) end_row: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValueTreeVirtualScroll {
    pub(crate) content_height: usize,
    pub(crate) viewport_content_height: usize,
    pub(crate) scaled: bool,
    pub(crate) viewport_start_row: usize,
    pub(crate) start_row: usize,
    pub(crate) end_row: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValueSearchVirtualScroll {
    pub(crate) content_height: usize,
    pub(crate) viewport_content_height: usize,
    pub(crate) scaled: bool,
    pub(crate) viewport_start_row: usize,
    pub(crate) start_row: usize,
    pub(crate) end_row: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextVirtualScroll {
    pub(crate) content_height: usize,
    pub(crate) viewport_content_height: usize,
    pub(crate) scaled: bool,
    pub(crate) viewport_start_row: usize,
    pub(crate) start_row: usize,
    pub(crate) end_row: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileListVirtualScroll {
    pub(crate) content_height: usize,
    pub(crate) viewport_content_height: usize,
    pub(crate) scaled: bool,
    pub(crate) viewport_start_row: usize,
    pub(crate) start_row: usize,
    pub(crate) end_row: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualScroll {
    content_height: usize,
    viewport_content_height: usize,
    scaled: bool,
    viewport_start_row: usize,
    start_row: usize,
    end_row: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct VirtualScrollSpec {
    row_height: usize,
    overscan_rows: usize,
    max_viewport_rows: usize,
    max_scroll_height: usize,
    default_viewport_height: usize,
}

impl From<VirtualScroll> for HexVirtualScroll {
    fn from(scroll: VirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}

impl From<VirtualScroll> for ValueTreeVirtualScroll {
    fn from(scroll: VirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}

impl From<VirtualScroll> for ValueSearchVirtualScroll {
    fn from(scroll: VirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}

impl From<VirtualScroll> for TextVirtualScroll {
    fn from(scroll: VirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}

impl From<VirtualScroll> for FileListVirtualScroll {
    fn from(scroll: VirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}

const HEX_SCROLL_SPEC: VirtualScrollSpec = VirtualScrollSpec {
    row_height: HEX_ROW_HEIGHT,
    overscan_rows: HEX_OVERSCAN_ROWS,
    max_viewport_rows: HEX_MAX_VIEWPORT_ROWS,
    max_scroll_height: HEX_MAX_VIRTUAL_SCROLL_HEIGHT,
    default_viewport_height: HEX_DEFAULT_VIEWPORT_HEIGHT,
};

const VALUE_TREE_SCROLL_SPEC: VirtualScrollSpec = VirtualScrollSpec {
    row_height: VALUE_TREE_ROW_HEIGHT,
    overscan_rows: VALUE_TREE_OVERSCAN_ROWS,
    max_viewport_rows: VALUE_TREE_MAX_VIEWPORT_ROWS,
    max_scroll_height: VALUE_TREE_MAX_VIRTUAL_SCROLL_HEIGHT,
    default_viewport_height: VALUE_TREE_DEFAULT_VIEWPORT_HEIGHT,
};

const VALUE_SEARCH_SCROLL_SPEC: VirtualScrollSpec = VirtualScrollSpec {
    row_height: VALUE_SEARCH_ROW_HEIGHT,
    overscan_rows: VALUE_SEARCH_OVERSCAN_ROWS,
    max_viewport_rows: VALUE_SEARCH_MAX_VIEWPORT_ROWS,
    max_scroll_height: VALUE_SEARCH_MAX_VIRTUAL_SCROLL_HEIGHT,
    default_viewport_height: VALUE_SEARCH_DEFAULT_VIEWPORT_HEIGHT,
};

const TEXT_SCROLL_SPEC: VirtualScrollSpec = VirtualScrollSpec {
    row_height: TEXT_ROW_HEIGHT,
    overscan_rows: TEXT_OVERSCAN_ROWS,
    max_viewport_rows: TEXT_MAX_VIEWPORT_ROWS,
    max_scroll_height: TEXT_MAX_VIRTUAL_SCROLL_HEIGHT,
    default_viewport_height: TEXT_DEFAULT_VIEWPORT_HEIGHT,
};

const FILE_LIST_SCROLL_SPEC: VirtualScrollSpec = VirtualScrollSpec {
    row_height: FILE_LIST_ROW_HEIGHT,
    overscan_rows: FILE_LIST_OVERSCAN_ROWS,
    max_viewport_rows: FILE_LIST_MAX_VIEWPORT_ROWS,
    max_scroll_height: FILE_LIST_MAX_VIRTUAL_SCROLL_HEIGHT,
    default_viewport_height: FILE_LIST_DEFAULT_VIEWPORT_HEIGHT,
};

pub(crate) fn hex_virtual_scroll(
    row_count: usize,
    scroll_top: f64,
    viewport_height: usize,
) -> HexVirtualScroll {
    virtual_scroll(row_count, scroll_top, viewport_height, HEX_SCROLL_SPEC).into()
}

pub(crate) fn measured_hex_viewport_height(height: f64) -> usize {
    measured_viewport_height(height, HEX_SCROLL_SPEC)
}

#[cfg(test)]
pub(crate) fn hex_viewport_rows(viewport_height: usize) -> usize {
    viewport_rows(viewport_height, HEX_SCROLL_SPEC)
}

pub(crate) fn hex_virtual_row_top(
    row_index: usize,
    scroll_top: f64,
    scroll: HexVirtualScroll,
) -> i64 {
    virtual_row_top(row_index, scroll_top, scroll.into(), HEX_SCROLL_SPEC)
}

pub(crate) fn hex_scroll_top_for_row(
    row_index: usize,
    row_count: usize,
    viewport_height: usize,
) -> f64 {
    scroll_top_for_row(row_index, row_count, viewport_height, HEX_SCROLL_SPEC)
}

pub(crate) fn value_tree_virtual_scroll(
    row_count: usize,
    scroll_top: f64,
    viewport_height: usize,
) -> ValueTreeVirtualScroll {
    virtual_scroll(
        row_count,
        scroll_top,
        viewport_height,
        VALUE_TREE_SCROLL_SPEC,
    )
    .into()
}

pub(crate) fn measured_value_tree_viewport_height(height: f64) -> usize {
    measured_viewport_height(height, VALUE_TREE_SCROLL_SPEC)
}

#[cfg(test)]
pub(crate) fn value_tree_viewport_rows(viewport_height: usize) -> usize {
    viewport_rows(viewport_height, VALUE_TREE_SCROLL_SPEC)
}

pub(crate) fn value_tree_virtual_row_top(
    row_index: usize,
    scroll_top: f64,
    scroll: ValueTreeVirtualScroll,
) -> i64 {
    virtual_row_top(row_index, scroll_top, scroll.into(), VALUE_TREE_SCROLL_SPEC)
}

pub(crate) fn value_search_virtual_scroll(
    row_count: usize,
    scroll_top: f64,
    viewport_height: usize,
) -> ValueSearchVirtualScroll {
    virtual_scroll(
        row_count,
        scroll_top,
        viewport_height,
        VALUE_SEARCH_SCROLL_SPEC,
    )
    .into()
}

pub(crate) fn measured_value_search_viewport_height(height: f64) -> usize {
    measured_viewport_height(height, VALUE_SEARCH_SCROLL_SPEC)
}

#[cfg(test)]
pub(crate) fn value_search_viewport_rows(viewport_height: usize) -> usize {
    viewport_rows(viewport_height, VALUE_SEARCH_SCROLL_SPEC)
}

pub(crate) fn value_search_virtual_row_top(
    row_index: usize,
    scroll_top: f64,
    scroll: ValueSearchVirtualScroll,
) -> i64 {
    virtual_row_top(
        row_index,
        scroll_top,
        scroll.into(),
        VALUE_SEARCH_SCROLL_SPEC,
    )
}

pub(crate) fn text_virtual_scroll(
    row_count: usize,
    scroll_top: f64,
    viewport_height: usize,
) -> TextVirtualScroll {
    virtual_scroll(row_count, scroll_top, viewport_height, TEXT_SCROLL_SPEC).into()
}

pub(crate) fn measured_text_viewport_height(height: f64) -> usize {
    measured_viewport_height(height, TEXT_SCROLL_SPEC)
}

pub(crate) fn text_virtual_row_top(
    row_index: usize,
    scroll_top: f64,
    scroll: TextVirtualScroll,
) -> i64 {
    virtual_row_top(row_index, scroll_top, scroll.into(), TEXT_SCROLL_SPEC)
}

pub(crate) fn file_list_virtual_scroll(
    row_count: usize,
    scroll_top: f64,
    viewport_height: usize,
) -> FileListVirtualScroll {
    virtual_scroll(
        row_count,
        scroll_top,
        viewport_height,
        FILE_LIST_SCROLL_SPEC,
    )
    .into()
}

pub(crate) fn measured_file_list_viewport_height(height: f64) -> usize {
    measured_viewport_height(height, FILE_LIST_SCROLL_SPEC)
}

#[cfg(test)]
pub(crate) fn file_list_viewport_rows(viewport_height: usize) -> usize {
    viewport_rows(viewport_height, FILE_LIST_SCROLL_SPEC)
}

pub(crate) fn file_list_virtual_row_top(
    row_index: usize,
    scroll_top: f64,
    scroll: FileListVirtualScroll,
) -> i64 {
    virtual_row_top(row_index, scroll_top, scroll.into(), FILE_LIST_SCROLL_SPEC)
}

fn virtual_scroll(
    row_count: usize,
    scroll_top: f64,
    viewport_height: usize,
    spec: VirtualScrollSpec,
) -> VirtualScroll {
    let logical_height = row_count.saturating_mul(spec.row_height);
    let content_height = logical_height.min(spec.max_scroll_height);
    let viewport_rows = viewport_rows(viewport_height, spec);
    let viewport_content_height = viewport_rows.saturating_mul(spec.row_height);
    if row_count == 0 {
        return VirtualScroll {
            content_height,
            viewport_content_height,
            scaled: false,
            viewport_start_row: 0,
            start_row: 0,
            end_row: 0,
        };
    }

    let max_start_row = row_count.saturating_sub(viewport_rows);
    let scaled = logical_height > spec.max_scroll_height;
    let viewport_start_row = if scaled {
        let scrollable_height = content_height
            .saturating_sub(viewport_content_height)
            .max(1);
        let ratio = (safe_scroll_top(scroll_top) / scrollable_height as f64).clamp(0.0, 1.0);
        ((ratio * max_start_row as f64).floor() as usize).min(max_start_row)
    } else {
        ((safe_scroll_top(scroll_top) / spec.row_height as f64).floor() as usize).min(max_start_row)
    };

    let start_row = viewport_start_row.saturating_sub(spec.overscan_rows);
    let end_row = viewport_start_row
        .saturating_add(viewport_rows)
        .saturating_add(spec.overscan_rows)
        .min(row_count);

    VirtualScroll {
        content_height,
        viewport_content_height,
        scaled,
        viewport_start_row,
        start_row,
        end_row,
    }
}

fn measured_viewport_height(height: f64, spec: VirtualScrollSpec) -> usize {
    if height.is_finite() && height > 0.0 {
        height.ceil().min(usize::MAX as f64) as usize
    } else {
        spec.default_viewport_height
    }
}

fn viewport_rows(viewport_height: usize, spec: VirtualScrollSpec) -> usize {
    viewport_height
        .div_ceil(spec.row_height)
        .max(1)
        .min(spec.max_viewport_rows)
}

fn virtual_row_top(
    row_index: usize,
    scroll_top: f64,
    scroll: VirtualScroll,
    spec: VirtualScrollSpec,
) -> i64 {
    if !scroll.scaled {
        return row_index
            .saturating_mul(spec.row_height)
            .min(i64::MAX as usize) as i64;
    }

    let relative_rows = row_index as i128 - scroll.viewport_start_row as i128;
    let effective_scroll_top = safe_scroll_top(scroll_top).round().min(
        scroll
            .content_height
            .saturating_sub(scroll.viewport_content_height) as f64,
    ) as i128;
    let top = effective_scroll_top + relative_rows * spec.row_height as i128;
    top.clamp(0, i64::MAX as i128) as i64
}

fn scroll_top_for_row(
    row_index: usize,
    row_count: usize,
    viewport_height: usize,
    spec: VirtualScrollSpec,
) -> f64 {
    if row_count == 0 {
        return 0.0;
    }

    let logical_height = row_count.saturating_mul(spec.row_height);
    if logical_height <= spec.max_scroll_height {
        return row_index.saturating_mul(spec.row_height) as f64;
    }

    let viewport_rows = viewport_rows(viewport_height, spec);
    let viewport_content_height = viewport_rows.saturating_mul(spec.row_height);
    let scrollable_height = spec
        .max_scroll_height
        .saturating_sub(viewport_content_height)
        .max(1);
    let max_start_row = row_count.saturating_sub(viewport_rows).max(1);
    let target_start_row = row_index.min(max_start_row);
    (target_start_row as f64 / max_start_row as f64) * scrollable_height as f64
}

fn safe_scroll_top(scroll_top: f64) -> f64 {
    if scroll_top.is_finite() && scroll_top > 0.0 {
        scroll_top
    } else {
        0.0
    }
}

impl From<HexVirtualScroll> for VirtualScroll {
    fn from(scroll: HexVirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}

impl From<ValueTreeVirtualScroll> for VirtualScroll {
    fn from(scroll: ValueTreeVirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}

impl From<ValueSearchVirtualScroll> for VirtualScroll {
    fn from(scroll: ValueSearchVirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}

impl From<TextVirtualScroll> for VirtualScroll {
    fn from(scroll: TextVirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}

impl From<FileListVirtualScroll> for VirtualScroll {
    fn from(scroll: FileListVirtualScroll) -> Self {
        Self {
            content_height: scroll.content_height,
            viewport_content_height: scroll.viewport_content_height,
            scaled: scroll.scaled,
            viewport_start_row: scroll.viewport_start_row,
            start_row: scroll.start_row,
            end_row: scroll.end_row,
        }
    }
}
