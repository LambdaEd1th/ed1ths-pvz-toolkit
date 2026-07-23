use super::super::*;

#[test]
fn byte_document_reader_reads_and_seeks() {
    let byte_doc = ByteDocument::from_vec(b"abcdef".to_vec()).apply_edit(&HexEdit {
        offset: 3,
        delete_length: 0,
        insert: b"XYZ".to_vec(),
    });
    let mut reader = byte_doc.reader();
    let mut buffer = [0u8; 4];

    reader
        .seek(SeekFrom::Start(2))
        .expect("reader seeks to offset");
    let read = reader.read(&mut buffer).expect("reader reads range");

    assert_eq!(read, 4);
    assert_eq!(&buffer, b"cXYZ");
    assert_eq!(
        reader
            .seek(SeekFrom::Current(-2))
            .expect("reader seeks backwards"),
        4
    );
    let mut tail = Vec::new();
    reader.read_to_end(&mut tail).expect("reader reads tail");
    assert_eq!(tail, b"YZdef".to_vec());
}

#[test]
fn rton_tabs_parse_from_piece_reader() {
    let doc = parse_text(SAMPLE_JSON, TextFormat::Json).expect("sample parses");
    let bytes = encode_rton_bytes(&doc.value, EncodeOptions::default()).expect("rton encodes");
    let byte_doc = ByteDocument::from_vec(bytes.clone()).apply_edit(&HexEdit {
        offset: 0,
        delete_length: 1,
        insert: vec![bytes[0]],
    });
    let tab = create_tab_from_byte_document(9, "piece.rton".to_string(), byte_doc)
        .expect("piece tab opens");

    assert_eq!(
        document_for_tab(&tab)
            .expect("piece reader rton parse succeeds")
            .value,
        doc.value
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn byte_document_can_map_file_path() {
    let path = std::env::temp_dir().join(format!(
        "rton-editor-byte-document-{}.bin",
        std::process::id()
    ));
    let bytes = b"mapped byte document";
    std::fs::write(&path, bytes).expect("temp file is written");

    let byte_doc = ByteDocument::from_file_path(&path).expect("temp file maps");

    assert_eq!(byte_doc.to_vec(), bytes);
    let _ = std::fs::remove_file(path);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn byte_document_accepts_empty_file_path() {
    let path = std::env::temp_dir().join(format!(
        "rton-editor-empty-byte-document-{}.bin",
        std::process::id()
    ));
    std::fs::write(&path, []).expect("empty temp file is written");

    let byte_doc = ByteDocument::from_file_path(&path).expect("empty temp file opens");

    assert!(byte_doc.is_empty());
    let _ = std::fs::remove_file(path);
}
