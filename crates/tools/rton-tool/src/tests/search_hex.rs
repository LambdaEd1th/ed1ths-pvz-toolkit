use super::*;

#[test]
fn searches_text_with_case_options() {
    let text = "RTON data rton";

    let insensitive = find_text_search_result(text, "rton", false);
    assert_eq!(
        insensitive.matches,
        vec![
            TextSearchMatch { start: 0, end: 4 },
            TextSearchMatch { start: 10, end: 14 }
        ]
    );

    let sensitive = find_text_search_result(text, "rton", true);
    assert_eq!(
        sensitive.matches,
        vec![TextSearchMatch { start: 10, end: 14 }]
    );
}

#[test]
fn replaces_text_search_matches() {
    let text = "RTON data rton";
    let matches = find_text_search_result(text, "rton", false).matches;

    assert_eq!(replace_text_span(text, 0, 4, "JSON"), "JSON data rton");
    assert_eq!(replace_all_text_matches(text, &matches, "X"), "X data X");
}

#[test]
fn caps_text_search_results_for_display() {
    let text = "a".repeat(TEXT_SEARCH_MATCH_DISPLAY_LIMIT + 5);
    let result = find_text_search_result(&text, "a", false);

    assert_eq!(result.matches.len(), TEXT_SEARCH_MATCH_DISPLAY_LIMIT);
    assert!(result.capped);
    assert_eq!(
        replace_all_text_query(&text, "a", "b", false),
        "b".repeat(TEXT_SEARCH_MATCH_DISPLAY_LIMIT + 5)
    );
}

#[test]
fn searches_large_text_with_parallel_ascii_case_fold() {
    let mut text = "x".repeat(700_000);
    let mut expected = Vec::new();
    for offset in (128..700_000).step_by(4096) {
        text.replace_range(offset..offset + 4, "RtOn");
        expected.push(TextSearchMatch {
            start: offset,
            end: offset + 4,
        });
    }

    let result = find_text_search_result(&text, "rton", false);

    assert_eq!(result.matches, expected);
    assert!(!result.capped);
}

#[test]
fn searches_large_text_with_parallel_case_sensitive_ascii() {
    let mut text = "x".repeat(700_000);
    let mut expected = Vec::new();
    for offset in (256..700_000).step_by(8192) {
        text.replace_range(offset..offset + 4, "RTON");
        expected.push(TextSearchMatch {
            start: offset,
            end: offset + 4,
        });
    }

    let result = find_text_search_result(&text, "RTON", true);

    assert_eq!(result.matches, expected);
    assert!(!result.capped);
}

#[test]
fn replaces_hex_byte_spans() {
    let bytes = [0x52, 0x54, 0x4f, 0x4e];

    assert_eq!(
        replace_byte_span(&bytes, 1, 2, &[0x00, 0x01, 0x02]),
        vec![0x52, 0x00, 0x01, 0x02, 0x4e]
    );
    assert_eq!(
        overwrite_byte_range(&bytes, 2, &[0xaa, 0xbb]),
        vec![0x52, 0x54, 0xaa, 0xbb]
    );
}

#[test]
fn applies_hex_edits_and_reverses_history_entries() {
    let bytes = [0x52, 0x54, 0x4f, 0x4e];
    let byte_doc = ByteDocument::from_vec(bytes.to_vec());
    let edit = HexEdit {
        offset: 1,
        delete_length: 2,
        insert: vec![0x00, 0x01, 0x02],
    };
    let undo = HexUndoEdit::from_edit(&byte_doc, &edit);
    let edited = apply_hex_edit(&bytes, &edit);
    let edited_doc = byte_doc.apply_edit(&edit);

    assert_eq!(edited, vec![0x52, 0x00, 0x01, 0x02, 0x4e]);
    assert_eq!(edited_doc.to_vec(), edited);
    assert_eq!(edited_doc.byte_at(2), Some(0x01));
    assert_eq!(edited_len(bytes.len(), &edit), edited.len());
    assert_eq!(apply_hex_edit(&edited, &undo.undo_edit()), bytes);
    assert_eq!(apply_hex_edit(&bytes, &undo.redo_edit()), edited);
}

#[test]
fn prepares_hex_edit_batches_for_undo_and_redo() {
    let bytes = b"aaaa".to_vec();
    let byte_doc = ByteDocument::from_vec(bytes.clone());
    let edits = replace_all_byte_edits(&byte_doc, b"a", b"bb", false).expect("edits");
    let (effective_edits, undo) = prepare_hex_edits(&byte_doc, edits).expect("effective edits");
    let edited = apply_hex_edits(&bytes, &effective_edits);

    assert_eq!(
        effective_edits,
        vec![
            HexEdit {
                offset: 0,
                delete_length: 1,
                insert: b"bb".to_vec()
            },
            HexEdit {
                offset: 2,
                delete_length: 1,
                insert: b"bb".to_vec()
            },
            HexEdit {
                offset: 4,
                delete_length: 1,
                insert: b"bb".to_vec()
            },
            HexEdit {
                offset: 6,
                delete_length: 1,
                insert: b"bb".to_vec()
            },
        ]
    );
    assert_eq!(edited, b"bbbbbbbb".to_vec());
    assert_eq!(
        apply_hex_edits(&edited, &undo.undo_edits()),
        b"aaaa".to_vec()
    );
    assert_eq!(
        apply_hex_edits(&bytes, &undo.redo_edits()),
        b"bbbbbbbb".to_vec()
    );
}

#[test]
fn indexes_piece_bytes_after_multiple_edits() {
    let byte_doc = ByteDocument::from_vec(b"ace".to_vec())
        .apply_edit(&HexEdit {
            offset: 1,
            delete_length: 0,
            insert: b"b".to_vec(),
        })
        .apply_edit(&HexEdit {
            offset: 3,
            delete_length: 0,
            insert: b"d".to_vec(),
        });

    let ByteSource::Pieces(source) = &byte_doc.source else {
        panic!("edited document should use pieces");
    };

    assert_eq!(source.starts, vec![0, 1, 2, 3, 4]);
    assert_eq!(source.piece_index_at(3), Some(3));
    assert_eq!(byte_doc.to_vec(), b"abcde".to_vec());
    assert_eq!(byte_doc.range_to_vec(1, 4), Some(b"bcd".to_vec()));
}

#[test]
fn merges_adjacent_pieces_from_same_source() {
    let source = Arc::<[u8]>::from(b"abcdef".to_vec());
    let bytes = PieceBytes::from_pieces(vec![
        BytePiece {
            source: BytePieceSource::Memory(source.clone()),
            start: 0,
            len: 2,
        },
        BytePiece {
            source: BytePieceSource::Memory(source),
            start: 2,
            len: 3,
        },
    ]);

    assert_eq!(bytes.pieces.len(), 1);
    assert_eq!(bytes.starts, vec![0]);
    assert_eq!(bytes.range_to_vec(1, 4), Some(b"bcd".to_vec()));
}

#[test]
fn searches_hex_and_ascii_patterns() {
    let bytes = b"RTON data rton".to_vec();
    let hex = parse_hex_pattern("52 54 4f 4e", false, I18n::new(Locale::EN_US));
    let ascii = parse_ascii_pattern("RTON", false, I18n::new(Locale::EN_US));

    assert!(hex.valid);
    assert_eq!(find_hex_search_matches(&bytes, &hex.bytes, false).len(), 1);
    assert!(ascii.valid);
    assert_eq!(find_hex_search_matches(&bytes, &ascii.bytes, true).len(), 2);
}

#[test]
fn searches_and_replaces_large_hex_with_parallel_chunks() {
    let mut bytes = vec![0u8; 700_000];
    let mut expected_matches = Vec::new();
    for offset in (64..700_000 - 2).step_by(4096) {
        bytes[offset] = 0xaa;
        bytes[offset + 1] = 0xbb;
        expected_matches.push(HexSearchMatch { offset, length: 2 });
    }
    let byte_doc = ByteDocument::from_vec(bytes.clone());

    let result = find_hex_search_result(&byte_doc, &[0xaa, 0xbb], false);
    let edits = replace_all_byte_edits(&byte_doc, &[0xaa, 0xbb], &[0xcc, 0xdd], false)
        .expect("parallel edits");

    assert_eq!(result.matches, expected_matches);
    assert!(!result.capped);
    assert_eq!(edits.len(), expected_matches.len());
    assert_eq!(edits.first().map(|edit| edit.offset), Some(64));
    assert_eq!(
        edits.last().map(|edit| edit.insert.as_slice()),
        Some(&[0xcc, 0xdd][..])
    );
}

#[test]
fn searches_and_replaces_across_piece_boundaries() {
    let byte_doc = ByteDocument::from_vec(b"abef".to_vec()).apply_edit(&HexEdit {
        offset: 2,
        delete_length: 0,
        insert: b"cd".to_vec(),
    });

    let result = find_hex_search_result(&byte_doc, b"bcde", false);
    let edits = replace_all_byte_edits(&byte_doc, b"bcde", b"Q", false);

    assert_eq!(
        result.matches,
        vec![HexSearchMatch {
            offset: 1,
            length: 4
        }]
    );
    assert_eq!(
        edits,
        Some(vec![HexEdit {
            offset: 1,
            delete_length: 4,
            insert: b"Q".to_vec()
        }])
    );
    assert_eq!(
        byte_doc.apply_edits(&edits.expect("replace edit")).to_vec(),
        b"aQf".to_vec()
    );
    assert_eq!(replace_all_byte_edits(&byte_doc, b"zz", b"Q", false), None);
    assert_eq!(
        replace_all_byte_edits(&byte_doc, b"bcde", b"bcde", false),
        None
    );
}

#[test]
fn caps_hex_search_results_for_display() {
    let bytes = vec![b'a'; HEX_SEARCH_MATCH_DISPLAY_LIMIT + 5];
    let result = find_hex_search_result(&bytes, b"a", false);

    assert_eq!(result.matches.len(), HEX_SEARCH_MATCH_DISPLAY_LIMIT);
    assert!(result.capped);
}

#[test]
fn identifies_common_rton_tags() {
    let tag = rton_tag_info(0x85).expect("object start tag");

    assert_eq!(tag.name, "ObjectStart");
    assert_eq!(tag.category, "container");
    assert!(rton_tag_info(0x7f).is_none());
}

#[test]
fn decodes_rton_varints_for_inspector() {
    let varint = read_rton_varint(&[0xac, 0x02], 0).expect("varint");

    assert_eq!(varint.value, "300");
    assert_eq!(varint.zigzag, "150");
    assert_eq!(varint.length, 2);
    assert_eq!(varint.bytes, "AC 02");
    assert_eq!(varint.next_offset, 2);
}

#[test]
fn inspects_fixed_width_rton_payloads() {
    let bytes = [0x20, 0x78, 0x56, 0x34, 0x12];
    let payload =
        inspect_rton_payload(&bytes, 0, 0x20, rton_tag_info(0x20)).expect("int32 payload");

    assert_eq!(payload.label, "i32le");
    assert_eq!(payload.value, "305419896");
    assert_eq!(payload.bytes.as_deref(), Some("78 56 34 12"));
    assert_eq!(payload.range.as_deref(), Some("0x1..0x5"));
}

#[test]
fn resolves_string_table_references_for_inspector() {
    let bytes = [
        b'R', b'T', b'O', b'N', 1, 0, 0, 0, 0x90, 3, b'f', b'o', b'o', 0x91, 0,
    ];
    let tables = maybe_collect_string_tables(&bytes, 13).expect("string tables");
    let info = inspect_rton_string_info(&bytes, 13, Some(&tables)).expect("string ref");

    assert_eq!(tables.ascii, vec!["foo".to_string()]);
    assert_eq!(info.mode, RtonStringMode::Reference);
    assert_eq!(info.index.as_deref(), Some("0"));
    assert_eq!(info.resolved_text.as_deref(), Some("foo"));
}
