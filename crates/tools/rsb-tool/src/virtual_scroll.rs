pub(crate) const TABLE_ROW_HEIGHT: usize = 50;
pub(crate) const TABLE_DEFAULT_VIEWPORT_HEIGHT: usize = 620;
const TABLE_OVERSCAN_ROWS: usize = 14;
const TABLE_MAX_VIEWPORT_ROWS: usize = 192;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VirtualWindow {
    pub(crate) content_height: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn table_virtual_window(
    row_count: usize,
    scroll_top: f64,
    viewport_height: usize,
) -> VirtualWindow {
    let viewport_height = viewport_height.max(TABLE_ROW_HEIGHT);
    let viewport_rows = viewport_height
        .div_ceil(TABLE_ROW_HEIGHT)
        .saturating_add(1)
        .min(TABLE_MAX_VIEWPORT_ROWS);
    let visible_start = ((scroll_top.max(0.0) as usize) / TABLE_ROW_HEIGHT).min(row_count);
    let start = visible_start.saturating_sub(TABLE_OVERSCAN_ROWS);
    let end = visible_start
        .saturating_add(viewport_rows)
        .saturating_add(TABLE_OVERSCAN_ROWS)
        .min(row_count);
    VirtualWindow {
        content_height: row_count.saturating_mul(TABLE_ROW_HEIGHT),
        start,
        end,
    }
}

pub(crate) fn measured_table_viewport_height(height: f64) -> usize {
    if !height.is_finite() || height <= 0.0 {
        TABLE_DEFAULT_VIEWPORT_HEIGHT
    } else {
        (height as usize).max(TABLE_ROW_HEIGHT)
    }
}

pub(crate) fn table_row_top(row_index: usize) -> usize {
    row_index.saturating_mul(TABLE_ROW_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::{TABLE_DEFAULT_VIEWPORT_HEIGHT, TABLE_ROW_HEIGHT, table_virtual_window};

    #[test]
    fn large_tables_mount_only_a_bounded_window() {
        let window = table_virtual_window(10_000, 100_000.0, TABLE_DEFAULT_VIEWPORT_HEIGHT);
        assert!(window.start > 0);
        assert!(window.end - window.start < 64);
        assert_eq!(window.content_height, 10_000 * TABLE_ROW_HEIGHT);
    }

    #[test]
    fn tail_window_never_exceeds_row_count() {
        let window = table_virtual_window(2_563, 1_000_000.0, 800);
        assert_eq!(window.start, 2_549);
        assert_eq!(window.end, 2_563);
    }
}
