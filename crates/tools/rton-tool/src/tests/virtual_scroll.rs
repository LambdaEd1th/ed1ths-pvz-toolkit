use super::*;

#[test]
fn virtual_hex_scroll_keeps_normal_row_positions() {
    let scroll = hex_virtual_scroll(
        100,
        (HEX_ROW_HEIGHT * 3) as f64,
        HEX_DEFAULT_VIEWPORT_HEIGHT,
    );

    assert!(!scroll.scaled);
    assert_eq!(scroll.content_height, 100 * HEX_ROW_HEIGHT);
    assert_eq!(scroll.viewport_start_row, 3);
    assert_eq!(scroll.start_row, 0);
    assert_eq!(
        hex_virtual_row_top(5, 0.0, scroll),
        (5 * HEX_ROW_HEIGHT) as i64
    );
}

#[test]
fn virtual_hex_scroll_reaches_tail_when_height_is_capped() {
    let row_count = (HEX_MAX_VIRTUAL_SCROLL_HEIGHT / HEX_ROW_HEIGHT) * 8;
    let scroll_top = (HEX_MAX_VIRTUAL_SCROLL_HEIGHT - HEX_VISIBLE_ROWS * HEX_ROW_HEIGHT) as f64;
    let scroll = hex_virtual_scroll(row_count, scroll_top, HEX_DEFAULT_VIEWPORT_HEIGHT);

    assert!(scroll.scaled);
    assert_eq!(scroll.content_height, HEX_MAX_VIRTUAL_SCROLL_HEIGHT);
    assert_eq!(scroll.viewport_start_row, row_count - HEX_VISIBLE_ROWS);
    assert_eq!(scroll.end_row, row_count);
    assert!(
        hex_virtual_row_top(row_count - 1, scroll_top, scroll) as usize + HEX_ROW_HEIGHT
            <= scroll.content_height
    );
}

#[test]
fn virtual_hex_scroll_reaches_tail_with_tall_viewport_when_height_is_capped() {
    let row_count = (HEX_MAX_VIRTUAL_SCROLL_HEIGHT / HEX_ROW_HEIGHT) * 8;
    let viewport_height = HEX_DEFAULT_VIEWPORT_HEIGHT * 3;
    let viewport_rows = hex_viewport_rows(viewport_height);
    let scroll_top = (HEX_MAX_VIRTUAL_SCROLL_HEIGHT - viewport_height) as f64;
    let scroll = hex_virtual_scroll(row_count, scroll_top, viewport_height);

    assert!(scroll.scaled);
    assert_eq!(scroll.viewport_start_row, row_count - viewport_rows);
    assert_eq!(scroll.end_row, row_count);
}

#[test]
fn virtual_hex_scroll_clamps_runaway_viewport_height() {
    let row_count = 1_000_000;
    let scroll = hex_virtual_scroll(row_count, 0.0, usize::MAX);

    assert_eq!(hex_viewport_rows(usize::MAX), HEX_MAX_VIEWPORT_ROWS);
    assert_eq!(scroll.viewport_start_row, 0);
    assert_eq!(scroll.start_row, 0);
    assert_eq!(scroll.end_row, HEX_MAX_VIEWPORT_ROWS + HEX_OVERSCAN_ROWS);
    assert!(scroll.end_row < row_count);
}

#[test]
fn virtual_value_tree_scroll_reaches_tail_when_height_is_capped() {
    let row_count = (VALUE_TREE_MAX_VIRTUAL_SCROLL_HEIGHT / VALUE_TREE_ROW_HEIGHT) * 8;
    let viewport_height = VALUE_TREE_DEFAULT_VIEWPORT_HEIGHT;
    let viewport_rows = value_tree_viewport_rows(viewport_height);
    let scroll_top = (VALUE_TREE_MAX_VIRTUAL_SCROLL_HEIGHT - viewport_height) as f64;
    let scroll = value_tree_virtual_scroll(row_count, scroll_top, viewport_height);

    assert!(scroll.scaled);
    assert_eq!(scroll.content_height, VALUE_TREE_MAX_VIRTUAL_SCROLL_HEIGHT);
    assert_eq!(scroll.viewport_start_row, row_count - viewport_rows);
    assert_eq!(scroll.end_row, row_count);
    assert!(
        value_tree_virtual_row_top(row_count - 1, scroll_top, scroll) as usize
            + VALUE_TREE_ROW_HEIGHT
            <= scroll.content_height
    );
}

#[test]
fn virtual_value_search_scroll_reaches_tail_when_height_is_capped() {
    let row_count = (VALUE_SEARCH_MAX_VIRTUAL_SCROLL_HEIGHT / VALUE_SEARCH_ROW_HEIGHT) * 8;
    let viewport_height = VALUE_SEARCH_DEFAULT_VIEWPORT_HEIGHT;
    let viewport_rows = value_search_viewport_rows(viewport_height);
    let scroll_top = (VALUE_SEARCH_MAX_VIRTUAL_SCROLL_HEIGHT - viewport_height) as f64;
    let scroll = value_search_virtual_scroll(row_count, scroll_top, viewport_height);

    assert!(scroll.scaled);
    assert_eq!(
        scroll.content_height,
        VALUE_SEARCH_MAX_VIRTUAL_SCROLL_HEIGHT
    );
    assert_eq!(scroll.viewport_start_row, row_count - viewport_rows);
    assert_eq!(scroll.end_row, row_count);
    assert!(
        value_search_virtual_row_top(row_count - 1, scroll_top, scroll) as usize
            + VALUE_SEARCH_ROW_HEIGHT
            <= scroll.content_height
    );
}

#[test]
fn virtual_file_list_scroll_reaches_tail_when_height_is_capped() {
    let row_count = (FILE_LIST_MAX_VIRTUAL_SCROLL_HEIGHT / FILE_LIST_ROW_HEIGHT) * 8;
    let viewport_height = FILE_LIST_DEFAULT_VIEWPORT_HEIGHT;
    let viewport_rows = file_list_viewport_rows(viewport_height);
    let scroll_top = (FILE_LIST_MAX_VIRTUAL_SCROLL_HEIGHT - viewport_height) as f64;
    let scroll = file_list_virtual_scroll(row_count, scroll_top, viewport_height);

    assert!(scroll.scaled);
    assert_eq!(scroll.content_height, FILE_LIST_MAX_VIRTUAL_SCROLL_HEIGHT);
    assert_eq!(scroll.viewport_start_row, row_count - viewport_rows);
    assert_eq!(scroll.end_row, row_count);
    assert!(
        file_list_virtual_row_top(row_count - 1, scroll_top, scroll) as usize
            + FILE_LIST_ROW_HEIGHT
            <= scroll.content_height
    );
}

#[test]
fn measured_hex_viewport_height_uses_default_for_invalid_values() {
    assert_eq!(
        measured_hex_viewport_height(f64::NAN),
        HEX_DEFAULT_VIEWPORT_HEIGHT
    );
    assert_eq!(
        measured_hex_viewport_height(-1.0),
        HEX_DEFAULT_VIEWPORT_HEIGHT
    );
    assert_eq!(measured_hex_viewport_height(42.2), 43);
}
