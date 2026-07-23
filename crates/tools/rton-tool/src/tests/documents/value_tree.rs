use super::super::*;

#[test]
fn full_value_tree_rows_are_not_truncated() {
    let value = RtonValue::Array((0..1200).map(RtonValue::UInt32).collect::<Vec<_>>());

    let rows = flatten_value_tree(&value, "", usize::MAX);

    assert!(!rows.truncated);
    assert_eq!(rows.rows.len(), 1201);
    assert_eq!(
        rows.rows.last().map(|row| row.path.as_str()),
        Some("$[1199]")
    );
}

#[test]
fn collapsed_value_tree_indices_hide_descendants() {
    let rows = vec![
        value_tree_test_row("$", "root", "object", 0, 2),
        value_tree_test_row("$.a#0", "a", "object", 1, 1),
        value_tree_test_row("$.a#0.b#0", "b", "u8", 2, 0),
        value_tree_test_row("$.c#1", "c", "u8", 1, 0),
    ];

    assert!(value_tree_visible_indices(&rows, &[]).is_none());
    assert_eq!(
        value_tree_visible_indices(&rows, &["$".to_string()]),
        Some(vec![0])
    );
    assert_eq!(
        value_tree_visible_indices(&rows, &["$.a#0".to_string()]),
        Some(vec![0, 1, 3])
    );
}

#[test]
fn value_search_results_are_not_capped() {
    let doc = DecodedDocument::new(
        RtonValue::Array(
            (0..1200)
                .map(|_| RtonValue::String("same".to_string()))
                .collect::<Vec<_>>(),
        ),
        false,
        None,
    );

    let result = value_search_result_for_doc(&doc, "same").expect("search runs");

    assert!(result.done);
    assert!(!result.capped);
    assert_eq!(result.matches.len(), 1200);
    assert_eq!(
        result.matches.last().map(|item| item.path.as_str()),
        Some("$[1199]")
    );
}
