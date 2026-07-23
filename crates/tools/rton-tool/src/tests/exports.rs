use super::*;

#[test]
fn rton_export_does_not_add_encoding_suffix() {
    assert_eq!(export_rton_name("/tmp/level.rton"), "level.rton");
    assert_eq!(export_rton_name("resources.json"), "resources.rton");
    assert_eq!(export_rton_name(""), "export.rton");
}

#[test]
fn batch_export_paths_replace_known_extensions() {
    assert_eq!(
        batch_output_path("PACKAGES/LEVEL.RTON", BatchExportMode::Json),
        "PACKAGES/LEVEL.json"
    );
    assert_eq!(
        batch_output_path("config.dat", BatchExportMode::Rton),
        "config.dat.rton"
    );
    assert_eq!(
        batch_output_path("folder/value.unknown", BatchExportMode::Toml),
        "folder/value.unknown.toml"
    );
    assert_eq!(batch_output_path("", BatchExportMode::Yaml), "rton.yaml");
}

#[test]
fn unique_zip_paths_add_suffixes() {
    let mut used = HashSet::new();

    assert_eq!(unique_zip_path("a/b.json", &mut used), "a/b.json");
    assert_eq!(unique_zip_path("a/b.json", &mut used), "a/b-2.json");
    assert_eq!(unique_zip_path("a/b.json", &mut used), "a/b-3.json");
}

#[test]
fn batch_export_encodes_rton_and_text() {
    let doc = parse_text(SAMPLE_JSON, TextFormat::Json).expect("sample parses");
    let json = encode_batch_export_document(&doc, BatchExportMode::Json, EncodeOptions::default())
        .expect("json encodes");
    assert!(
        String::from_utf8(json)
            .expect("json is utf8")
            .contains("\"objclass\"")
    );

    let rton = encode_batch_export_document(&doc, BatchExportMode::Rton, EncodeOptions::default())
        .expect("rton encodes");
    let decoded = decode_rton_reader(std::io::Cursor::new(rton)).expect("rton decodes");
    assert_eq!(decoded.value, doc.value);
}

#[test]
fn zip_archive_contains_entries() {
    let archive = create_zip_archive(vec![
        ZipFileEntry {
            path: "a.json".to_string(),
            bytes: b"{}".to_vec(),
        },
        ZipFileEntry {
            path: "folder/b.rton".to_string(),
            bytes: b"RTON".to_vec(),
        },
    ])
    .expect("zip is created");

    assert!(archive.starts_with(&0x04034b50_u32.to_le_bytes()));
    let haystack = String::from_utf8_lossy(&archive);
    assert!(haystack.contains("a.json"));
    assert!(haystack.contains("folder/b.rton"));
    assert!(
        archive
            .windows(4)
            .any(|window| window == 0x06054b50_u32.to_le_bytes())
    );
}
