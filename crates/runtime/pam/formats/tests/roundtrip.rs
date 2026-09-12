use std::sync::Arc;

use pam_editor_formats::InputFile;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|path| {
            std::fs::read_to_string(path.join("Cargo.toml"))
                .is_ok_and(|manifest| manifest.contains("[workspace]"))
        })
        .expect("workspace root")
        .to_path_buf()
}

fn sample_files(name: &str) -> Vec<InputFile> {
    let root = workspace_root().join("sample").join(name);
    let mut paths = std::fs::read_dir(root)
        .expect("sample directory")
        .map(|entry| entry.expect("sample entry").path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            InputFile::new(
                name,
                Arc::<[u8]>::from(std::fs::read(&path).expect("sample bytes")),
            )
        })
        .collect()
}

#[test]
fn pam_and_text_roundtrip_through_pam_codec_types() {
    let files = sample_files("sunflower");
    let loaded = pam_editor_formats::load_pam_document(&files).expect("load sunflower");
    let pam = &loaded.document.pam;

    let binary = pam_editor_core::encode_pam_bytes(pam).expect("encode PAM");
    let binary_loaded = pam_editor_formats::load_pam_document(&[InputFile::new(
        "roundtrip.pam",
        Arc::<[u8]>::from(binary.clone()),
    )])
    .expect("load binary PAM");
    assert_eq!(
        binary_loaded.original_pam_bytes.as_deref(),
        Some(binary.as_slice())
    );
    let binary_roundtrip = pam_editor_core::decode_pam_bytes(&binary).expect("decode PAM");
    assert_eq!(*pam, binary_roundtrip);

    for format in [
        pam_editor_formats::TextFormat::Json,
        pam_editor_formats::TextFormat::Yaml,
        pam_editor_formats::TextFormat::Toml,
    ] {
        let text = pam_editor_formats::encode_text(pam, format).expect("encode text");
        let decoded = pam_editor_formats::decode_text(&text, format).expect("decode text");
        assert_eq!(*pam, decoded);
    }
}

#[test]
fn unsupported_fla_is_not_accepted_as_a_pam_source() {
    let result = pam_editor_formats::load_pam_document(&[InputFile::new(
        "animation.fla",
        Arc::<[u8]>::from(&b"not a PAM"[..]),
    )]);
    assert!(matches!(
        result,
        Err(pam_editor_formats::FormatError::AnimationNotFound)
    ));
}
