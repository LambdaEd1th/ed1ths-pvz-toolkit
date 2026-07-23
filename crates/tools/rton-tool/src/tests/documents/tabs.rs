use super::super::*;

#[test]
fn rton_tabs_use_byte_document_surface() {
    let doc = parse_text(SAMPLE_JSON, TextFormat::Json).expect("sample parses");
    let bytes = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");

    let tab = create_tab_from_bytes(7, "sample.rton".to_string(), &bytes).expect("tab opens");

    assert_eq!(tab.mode, EditorMode::RtonHex);
    assert!(tab.doc.is_none());
    assert!(tab.editor_text.is_empty());
    assert_eq!(
        tab.byte_doc.as_ref().map(|byte_doc| byte_doc.to_vec()),
        Some(bytes.clone())
    );
    assert_eq!(
        document_for_tab(&tab)
            .expect("lazy rton parse succeeds")
            .value,
        doc.value
    );

    let surface = tab_surface_for_document(&doc, EditorMode::RtonHex, EncodeOptions::default())
        .expect("surface");
    assert!(surface.editor_text.is_empty());
    assert_eq!(
        surface.byte_doc.as_ref().map(|byte_doc| byte_doc.to_vec()),
        Some(bytes)
    );
}

#[test]
fn text_tabs_parse_on_demand() {
    let items = (0..1200)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let tab = create_text_tab(
        12,
        "array.json".to_string(),
        format!("[{items}]"),
        TextFormat::Json,
    )
    .expect("tab opens");

    assert!(tab.doc.is_none());
    assert!(tab.tree_rows.rows.is_empty());
    let doc = document_for_tab(&tab).expect("text parses on demand");
    let rows = value_tree_rows_for_doc(&doc);
    assert!(!rows.truncated);
    assert_eq!(rows.rows.len(), 1201);
    assert_eq!(
        rows.rows.last().map(|row| row.path.as_str()),
        Some("$[1199]")
    );
}

#[test]
fn direct_text_buffer_tabs_use_text_buffer_surface_and_parse_on_demand() {
    let payload = "a".repeat(1_000_001);
    let text = format!("\"{payload}\"");
    let tab = create_text_tab(
        13,
        "buffer.json".to_string(),
        text.clone(),
        TextFormat::Json,
    )
    .expect("tab opens");

    assert!(tab.doc.is_none());
    assert!(tab.editor_text.is_empty());
    assert_eq!(
        tab.text_buffer.as_ref().map(|buffer| buffer.byte_count()),
        Some(text.len())
    );
    assert!(matches!(
        tab.text_state,
        TextContentState::Text {
            byte_count,
            line_count: 1,
            format: TextFormat::Json,
        } if byte_count == text.len()
    ));

    let doc = document_for_tab(&tab).expect("buffer text parses on demand");
    assert!(matches!(
        doc.value,
        RtonValue::String(ref value) if value.len() == payload.len()
    ));
}

#[test]
fn invalid_rton_tabs_still_open_as_hex() {
    let bytes = b"not an rton document";
    let tab = create_tab_from_bytes(8, "broken.rton".to_string(), bytes).expect("hex tab opens");

    assert_eq!(tab.mode, EditorMode::RtonHex);
    assert!(tab.doc.is_none());
    assert_eq!(
        tab.byte_doc.as_ref().map(|byte_doc| byte_doc.to_vec()),
        Some(bytes.to_vec())
    );
    assert!(document_for_tab(&tab).is_err());
}

#[test]
fn unknown_extension_tabs_open_as_hex() {
    let bytes = b"RTON payload candidate";

    for name in ["profile.dat", "no-extension", "asset.bytes", "notes.txt"] {
        let tab = create_tab_from_bytes(9, name.to_string(), bytes).expect("hex tab opens");
        assert_eq!(tab.mode, EditorMode::RtonHex);
        assert_eq!(
            tab.byte_doc.as_ref().map(|byte_doc| byte_doc.to_vec()),
            Some(bytes.to_vec())
        );
    }
}
