use super::super::*;

#[test]
fn text_history_undo_and_redo_restore_editor_text() {
    let mut tab = tab(1);
    tab.editor_text = Arc::from("one");

    push_text_undo_snapshot(&mut tab.text_history, tab.editor_text.to_string());
    tab.editor_text = Arc::from("two");

    assert!(tab_can_undo(&tab));
    undo_text_tab(&mut tab);
    assert_eq!(
        tab.text_buffer.as_ref().map(|buffer| buffer.materialize()),
        Some("one".to_string())
    );
    assert!(tab_can_redo(&tab));
    redo_text_tab(&mut tab);
    assert_eq!(
        tab.text_buffer.as_ref().map(|buffer| buffer.materialize()),
        Some("two".to_string())
    );
}

#[test]
fn hex_history_undo_and_redo_restore_bytes() {
    let mut tab =
        create_tab_from_bytes(10, "raw.dat".to_string(), b"abcdef").expect("hex tab opens");
    let edit = HexEdit {
        offset: 2,
        delete_length: 2,
        insert: b"XY".to_vec(),
    };
    let byte_doc = tab.byte_doc.clone().expect("hex bytes");
    let (_, undo) = prepare_hex_edits(&byte_doc, vec![edit.clone()]).expect("effective edit");
    push_hex_undo_batch(&mut tab.hex_history, undo);
    tab.byte_doc = Some(byte_doc.apply_edit(&edit));

    assert_eq!(
        tab.byte_doc.as_ref().map(ByteDocument::to_vec),
        Some(b"abXYef".to_vec())
    );
    undo_hex_tab(&mut tab);
    assert_eq!(
        tab.byte_doc.as_ref().map(ByteDocument::to_vec),
        Some(b"abcdef".to_vec())
    );
    redo_hex_tab(&mut tab);
    assert_eq!(
        tab.byte_doc.as_ref().map(ByteDocument::to_vec),
        Some(b"abXYef".to_vec())
    );
}
