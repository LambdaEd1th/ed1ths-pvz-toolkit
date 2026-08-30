use super::*;
use serde_rton::{Rtid, Value};

const SAMPLE: &str = r#"{
  "name": "Peashooter",
  "enabled": true,
  "cost": 100,
  "resource": "RTID(0)",
  "payload": "$BINARY(\"0A0B0C\", 3)"
}"#;

#[test]
fn parses_editor_json_with_rton_strings() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    assert_eq!(doc.stats.objects, 1);
    assert_eq!(doc.stats.binaries, 1);
    assert_eq!(doc.stats.rtids, 1);
}

#[test]
fn empty_documents_round_trip_across_every_worker_mode() {
    let empty = Value::Object(Vec::new());
    let text_surfaces = [
        (TextFormat::Json, "{}"),
        (TextFormat::Yaml, "---\n"),
        (TextFormat::Toml, "\n"),
    ];

    for (format, text) in text_surfaces {
        assert_eq!(
            value_to_text(&empty, format).expect("empty value renders"),
            text
        );
        assert_eq!(
            parse_text(text, format).expect("empty text parses").value,
            empty
        );
    }

    let rton = encode_rton_bytes(&empty, EncodeOptions::default()).expect("empty RTON encodes");
    let sources = [
        WorkerDocumentSource::RtonBytes(rton),
        WorkerDocumentSource::Text {
            text: "{}".to_string(),
            format: TextFormat::Json,
        },
        WorkerDocumentSource::Text {
            text: "---\n".to_string(),
            format: TextFormat::Yaml,
        },
        WorkerDocumentSource::Text {
            text: "\n".to_string(),
            format: TextFormat::Toml,
        },
    ];
    let targets = [
        (WorkerEditorMode::RtonHex, None),
        (WorkerEditorMode::Json, Some((TextFormat::Json, "{}"))),
        (WorkerEditorMode::Yaml, Some((TextFormat::Yaml, "---\n"))),
        (WorkerEditorMode::Toml, Some((TextFormat::Toml, "\n"))),
    ];

    for source in sources {
        for (target_mode, expected_text) in targets {
            let outcome = perform_worker_mode_switch(WorkerModeSwitchRequest {
                previous_document_id: None,
                source: Some(source.clone()),
                target_mode,
                search_query: String::new(),
                encode_options: EncodeOptions::default(),
            })
            .expect("empty document switches mode");
            assert_eq!(outcome.document.value, empty);

            match (outcome.response.surface, expected_text) {
                (WorkerSurface::RtonBytes(bytes), None) => {
                    assert_eq!(
                        decode_rton_bytes(&bytes).expect("empty RTON decodes").value,
                        empty
                    );
                }
                (
                    WorkerSurface::Text { text, format, .. },
                    Some((expected_format, expected_text)),
                ) => {
                    assert_eq!(format, expected_format);
                    assert_eq!(text, expected_text);
                }
                _ => panic!("worker returned the wrong empty surface kind"),
            }
        }
    }
}

#[test]
fn round_trips_standard_rton_bytes() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let bytes = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");
    let decoded = decode_rton_bytes(&bytes).expect("rton decodes");
    assert_eq!(decoded.value, doc.value);
    assert_eq!(decoded.byte_len, Some(bytes.len()));
    assert_eq!(decoded.encoding_source, BinaryEncoding::Standard);
}

#[test]
fn decodes_standard_rton_from_reader() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let bytes = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");
    let decoded = decode_rton_reader(std::io::Cursor::new(bytes.clone())).expect("reader decodes");

    assert_eq!(decoded.value, doc.value);
    assert!(!decoded.encrypted_source);
    assert_eq!(decoded.byte_len, Some(bytes.len()));
    assert_eq!(decoded.encoding_source, BinaryEncoding::Standard);
}

#[test]
fn decodes_hex_rton_text() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let bytes = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");
    let hex = bytes_to_hex(&bytes);
    let decoded = decode_hex_rton(&hex).expect("hex rton decodes");
    assert_eq!(decoded.value, doc.value);
}

#[test]
fn reads_and_edits_values_by_tree_path() {
    let mut value = Value::Object(vec![
        (
            "a.b#c".to_string(),
            Value::Array(vec![Value::String("first".to_string()), Value::Bool(true)]),
        ),
        ("count".to_string(), Value::UInt16(12)),
        ("resource".to_string(), Value::Rtid(Rtid::Null)),
    ]);

    assert_eq!(
        value_at_path(&value, "$.a\\.b\\#c#0[0]").expect("path resolves"),
        &Value::String("first".to_string())
    );

    edit_value_at_path(&mut value, "$.a\\.b\\#c#0[1]", "false").expect("bool edits");
    assert_eq!(
        value_at_path(&value, "$.a\\.b\\#c#0[1]").expect("path resolves"),
        &Value::Bool(false)
    );

    edit_value_at_path(&mut value, "$.count#1", "65535").expect("u16 edits");
    assert_eq!(
        value_at_path(&value, "$.count#1").expect("path resolves"),
        &Value::UInt16(u16::MAX)
    );

    edit_value_at_path(&mut value, "$.resource#2", "RTID(example@parent)").expect("rtid edits");
    assert_eq!(
        scalar_edit_text(value_at_path(&value, "$.resource#2").expect("path resolves")),
        Some("RTID(example@parent)".to_string())
    );
}

#[test]
fn rejects_invalid_scalar_edits_without_changing_value() {
    let mut value = Value::Object(vec![("count".to_string(), Value::UInt8(7))]);

    let error = edit_value_at_path(&mut value, "$.count#0", "300")
        .expect_err("out-of-range u8 is rejected");
    assert!(error.to_string().contains("Invalid u8 value"));
    assert_eq!(
        value_at_path(&value, "$.count#0").expect("path resolves"),
        &Value::UInt8(7)
    );
}

#[test]
fn searches_value_tree_paths_and_previews() {
    let value = Value::Object(vec![
        (
            "space key".to_string(),
            Value::String("needle value".to_string()),
        ),
        (
            "items".to_string(),
            Value::Array(vec![Value::Bool(false), Value::UInt16(42)]),
        ),
    ]);

    let by_preview = search_value_tree(&value, "NEEDLE", 120);
    assert!(by_preview.done);
    assert!(!by_preview.capped);
    assert_eq!(by_preview.scanned, 5);
    assert_eq!(by_preview.matches.len(), 1);
    assert_eq!(by_preview.matches[0].path, "$.space key#0");
    assert_eq!(by_preview.matches[0].display_path, "$[\"space key\"]");
    assert_eq!(
        value_at_path(&value, &by_preview.matches[0].path).expect("path resolves"),
        &Value::String("needle value".to_string())
    );

    let by_path = search_value_tree(&value, "items[1]", 120);
    assert_eq!(by_path.matches.len(), 1);
    assert_eq!(by_path.matches[0].path, "$.items#1[1]");
    assert_eq!(by_path.matches[0].display_path, "$.items[1]");
}

#[test]
fn caps_value_search_results() {
    let value = Value::Array(vec![
        Value::String("same".to_string()),
        Value::String("same".to_string()),
        Value::String("same".to_string()),
    ]);

    let result = search_value_tree(&value, "same", 2);
    assert!(!result.done);
    assert!(result.capped);
    assert_eq!(result.matches.len(), 2);
    assert_eq!(result.scanned, 3);
}

#[test]
fn flattens_large_expanded_root_in_order() {
    let value = Value::Object(
        (0..600)
            .map(|index| (format!("key{index:03}"), Value::UInt16(index)))
            .collect(),
    );
    let expanded_paths = std::collections::HashSet::from(["$".to_string()]);

    let rows = flatten_expanded_value_tree(&value, &expanded_paths, usize::MAX);

    assert!(!rows.truncated);
    assert_eq!(rows.rows.len(), 601);
    assert_eq!(rows.rows[0].path, "$");
    assert_eq!(rows.rows[1].path, "$.key000#0");
    assert_eq!(rows.rows[1].label, "key000");
    assert_eq!(rows.rows[600].path, "$.key599#599");
    assert_eq!(rows.rows[600].label, "key599");
}

#[test]
fn round_trips_encrypted_rton_bytes() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let bytes = encode_rton_bytes(
        &doc.value,
        EncodeOptions {
            encoding: BinaryEncoding::Standard,
            encrypted: true,
        },
    )
    .expect("encrypted rton encodes");
    assert!(bytes.starts_with(ENCRYPTED_RTON_PREFIX));
    let decoded = decode_rton_bytes(&bytes).expect("encrypted rton decodes");
    assert!(decoded.encrypted_source);
    assert_eq!(decoded.encoding_source, BinaryEncoding::Standard);
    assert_eq!(decoded.value, doc.value);

    let decoded =
        decode_rton_reader(std::io::Cursor::new(bytes)).expect("encrypted reader decodes");
    assert!(decoded.encrypted_source);
    assert_eq!(decoded.encoding_source, BinaryEncoding::Standard);
    assert_eq!(decoded.value, doc.value);
}

#[test]
fn decodes_compact_rton_source_encoding() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let bytes = encode_rton_bytes(
        &doc.value,
        EncodeOptions {
            encoding: BinaryEncoding::Compact,
            encrypted: false,
        },
    )
    .expect("compact rton encodes");

    let decoded = decode_rton_bytes(&bytes).expect("compact rton decodes");
    assert!(!decoded.encrypted_source);
    assert_eq!(decoded.encoding_source, BinaryEncoding::Compact);
    assert_eq!(decoded.value, doc.value);
}

#[test]
fn decodes_encrypted_compact_rton_source_encoding() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let bytes = encode_rton_bytes(
        &doc.value,
        EncodeOptions {
            encoding: BinaryEncoding::Compact,
            encrypted: true,
        },
    )
    .expect("encrypted compact rton encodes");

    let decoded = decode_rton_bytes(&bytes).expect("encrypted compact rton decodes");
    assert!(decoded.encrypted_source);
    assert_eq!(decoded.encoding_source, BinaryEncoding::Compact);
    assert_eq!(decoded.value, doc.value);
}

#[test]
fn decrypts_encrypted_rton_bytes_for_display() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let plain = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");
    let encrypted = encode_rton_bytes(
        &doc.value,
        EncodeOptions {
            encoding: BinaryEncoding::Standard,
            encrypted: true,
        },
    )
    .expect("encrypted rton encodes");

    assert_eq!(
        decrypt_rton_bytes_if_needed(&plain).expect("plain check"),
        None
    );
    assert_eq!(
        decrypt_rton_bytes_if_needed(&encrypted).expect("encrypted decrypt"),
        Some(plain)
    );
}

#[test]
fn worker_mode_switch_decodes_rton_and_returns_text_surface() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let bytes = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");

    let response = perform_worker_mode_switch(WorkerModeSwitchRequest {
        previous_document_id: None,
        source: Some(WorkerDocumentSource::RtonBytes(bytes)),
        target_mode: WorkerEditorMode::Json,
        search_query: "Peashooter".to_string(),
        encode_options: EncodeOptions::default(),
    })
    .expect("worker mode switch succeeds");

    assert_eq!(response.document.value, doc.value);
    assert!(!response.response.tree_rows.rows.is_empty());
    assert_eq!(
        response
            .response
            .search_result
            .as_ref()
            .expect("search result")
            .matches
            .len(),
        1
    );

    let WorkerSurface::Text {
        text,
        byte_count,
        line_count,
        format,
    } = response.response.surface
    else {
        panic!("expected text surface");
    };
    assert_eq!(format, TextFormat::Json);
    assert!(text.contains("Peashooter"));
    assert_eq!(byte_count, text.len());
    assert_eq!(line_count, text.lines().count());
}

#[test]
fn worker_parse_decodes_source_and_returns_index_payload() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let bytes = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");

    let response = perform_worker_parse(WorkerParseRequest {
        previous_document_id: None,
        source: Some(WorkerDocumentSource::RtonBytes(bytes)),
        search_query: "cost".to_string(),
    })
    .expect("worker parse succeeds");

    assert_eq!(response.document.value, doc.value);
    assert!(!response.response.tree_rows.rows.is_empty());
    assert_eq!(
        response
            .response
            .search_result
            .as_ref()
            .expect("search result")
            .matches
            .len(),
        1
    );
    assert_eq!(response.response.search_query, "cost");
}

#[test]
fn worker_open_text_returns_surface_and_index_for_valid_text() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");

    let response = perform_worker_open_text(WorkerOpenTextRequest {
        bytes: SAMPLE.as_bytes().to_vec(),
        format: TextFormat::Json,
        search_query: "cost".to_string(),
    })
    .expect("worker open text succeeds");

    assert_eq!(response.document.as_ref().expect("doc").value, doc.value);
    assert!(!response.response.tree_rows.rows.is_empty());
    assert_eq!(
        response
            .response
            .search_result
            .as_ref()
            .expect("search result")
            .matches
            .len(),
        1
    );

    let WorkerSurface::Text {
        text,
        byte_count,
        line_count,
        format,
    } = response.response.surface
    else {
        panic!("expected text surface");
    };
    assert_eq!(format, TextFormat::Json);
    assert_eq!(text, SAMPLE);
    assert_eq!(byte_count, text.len());
    assert_eq!(line_count, text.lines().count());
}

#[test]
fn worker_open_text_keeps_surface_for_invalid_text() {
    let response = perform_worker_open_text(WorkerOpenTextRequest {
        bytes: b"{ invalid".to_vec(),
        format: TextFormat::Json,
        search_query: "invalid".to_string(),
    })
    .expect("worker open text keeps editable surface");

    assert!(response.document.is_none());
    assert!(response.response.tree_rows.rows.is_empty());
    assert!(response.response.search_result.is_none());
    let WorkerSurface::Text { text, .. } = response.response.surface else {
        panic!("expected text surface");
    };
    assert_eq!(text, "{ invalid");
}

#[test]
fn worker_rton_size_returns_target_encoded_length() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let bytes = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");
    let encode_options = EncodeOptions {
        encoding: BinaryEncoding::Compact,
        encrypted: true,
    };

    let response = perform_worker_rton_size(WorkerRtonSizeRequest {
        document_id: None,
        source: Some(WorkerDocumentSource::RtonBytes(bytes)),
        encode_options,
    })
    .expect("worker rton size succeeds");
    let expected = encode_rton_bytes(&doc.value, encode_options).expect("target rton encodes");

    assert_eq!(response.byte_len, expected.len());
}

#[test]
fn worker_document_operations_support_borrowed_cache_entries() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");
    let response = perform_worker_mode_switch_for_document(
        &doc,
        WorkerEditorMode::Yaml,
        EncodeOptions::default(),
        "Peashooter".to_string(),
    )
    .expect("borrowed mode switch succeeds");
    assert_eq!(response.stats, doc.stats);
    assert_eq!(
        response.search_result.expect("search result").matches.len(),
        1
    );
    assert!(matches!(
        response.surface,
        WorkerSurface::Text {
            format: TextFormat::Yaml,
            ..
        }
    ));

    let parsed = perform_worker_parse_for_document(&doc, "cost".to_string());
    assert_eq!(parsed.stats, doc.stats);
    assert_eq!(
        parsed.search_result.expect("search result").matches.len(),
        1
    );

    let size = perform_worker_rton_size_for_document(&doc, EncodeOptions::default())
        .expect("borrowed size calculation succeeds");
    assert!(size.byte_len > 0);
}

#[test]
fn worker_owned_operations_reject_missing_fallback_sources() {
    let error = perform_worker_parse(WorkerParseRequest {
        previous_document_id: Some(7),
        source: None,
        search_query: String::new(),
    })
    .expect_err("core cannot resolve a worker-local cache id");
    assert!(matches!(error, CoreError::InvalidWorkerRequest(_)));
}

#[test]
fn worker_mode_switch_parses_text_and_returns_rton_surface() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");

    let response = perform_worker_mode_switch(WorkerModeSwitchRequest {
        previous_document_id: None,
        source: Some(WorkerDocumentSource::Text {
            text: SAMPLE.to_string(),
            format: TextFormat::Json,
        }),
        target_mode: WorkerEditorMode::RtonHex,
        search_query: String::new(),
        encode_options: EncodeOptions::default(),
    })
    .expect("worker mode switch succeeds");

    assert_eq!(response.document.value, doc.value);
    assert!(response.response.search_result.is_none());

    let WorkerSurface::RtonBytes(bytes) = response.response.surface else {
        panic!("expected rton byte surface");
    };
    let decoded = decode_rton_bytes(&bytes).expect("worker rton decodes");
    assert_eq!(decoded.value, doc.value);
}

#[test]
fn worker_mode_switch_uses_rton_encode_options() {
    let doc = parse_text(SAMPLE, TextFormat::Json).expect("json parses");

    let response = perform_worker_mode_switch(WorkerModeSwitchRequest {
        previous_document_id: None,
        source: Some(WorkerDocumentSource::Text {
            text: SAMPLE.to_string(),
            format: TextFormat::Json,
        }),
        target_mode: WorkerEditorMode::RtonHex,
        search_query: String::new(),
        encode_options: EncodeOptions {
            encoding: BinaryEncoding::Compact,
            encrypted: false,
        },
    })
    .expect("worker mode switch succeeds");

    let WorkerSurface::RtonBytes(bytes) = response.response.surface else {
        panic!("expected rton byte surface");
    };
    let compact = encode_rton_bytes(
        &doc.value,
        EncodeOptions {
            encoding: BinaryEncoding::Compact,
            encrypted: false,
        },
    )
    .expect("compact rton encodes");
    assert_eq!(bytes, compact);
}

#[test]
fn surface_text_search_preserves_utf8_boundaries_and_caps_results() {
    let text = format!("{}世界{}", "Alpha ".repeat(3_000), " alpha".repeat(3_000));
    let result = find_text_search_result(&text, "alpha", false);

    assert_eq!(result.matches.len(), SURFACE_SEARCH_MATCH_LIMIT);
    assert!(result.capped);
    assert!(result.matches.iter().all(|match_| {
        text.is_char_boundary(match_.start)
            && text.is_char_boundary(match_.end)
            && text[match_.start..match_.end].eq_ignore_ascii_case("alpha")
    }));
}

#[test]
fn surface_hex_search_returns_non_overlapping_matches() {
    let bytes = b"aaaaAAaa".to_vec();
    let result = find_hex_search_result(&bytes, b"aa", true);

    assert_eq!(
        result.matches,
        vec![
            HexSearchMatch {
                offset: 0,
                length: 2,
            },
            HexSearchMatch {
                offset: 2,
                length: 2,
            },
            HexSearchMatch {
                offset: 4,
                length: 2,
            },
            HexSearchMatch {
                offset: 6,
                length: 2,
            },
        ]
    );
    assert!(!result.capped);
}
