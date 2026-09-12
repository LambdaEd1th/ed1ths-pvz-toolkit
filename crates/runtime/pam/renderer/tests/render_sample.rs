use std::sync::Arc;

use pam_editor_core::SpriteKey;
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
fn sunflower_frames_render_with_visible_and_changing_pixels() {
    let loaded =
        pam_editor_formats::load_pam_document(&sample_files("sunflower")).expect("load sunflower");
    let document = Arc::new(loaded.document);
    let frame_count = document
        .pam
        .main_sprite
        .as_ref()
        .expect("main sprite")
        .frame
        .len();
    let frames = (0..frame_count.min(6)).collect::<Vec<_>>();
    let rendered = pollster::block_on(pam_editor_renderer::render_offscreen_frames(
        document.clone(),
        SpriteKey::Main,
        &frames,
        &vec![true; document.pam.image.len()],
        &vec![true; document.pam.sprite.len()],
        pam_editor_renderer::ExportTarget {
            size: [390, 390],
            scale: 1.0,
        },
    ))
    .expect("render frames");

    let visible_pixels = rendered[0]
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] != 0)
        .count();
    assert!(visible_pixels > 500, "only {visible_pixels} visible pixels");
    assert_eq!(rendered.len(), frames.len());
    assert_ne!(rendered[0], rendered[1], "animation frames must differ");

    let expanded = pollster::block_on(pam_editor_renderer::render_offscreen_frames(
        document.clone(),
        SpriteKey::Main,
        &[frames[0]],
        &vec![true; document.pam.image.len()],
        &vec![true; document.pam.sprite.len()],
        pam_editor_renderer::ExportTarget {
            size: [800, 600],
            scale: 1.0,
        },
    ))
    .expect("render expanded canvas");
    for row in 0..390 {
        let original = &rendered[0][row * 390 * 4..(row + 1) * 390 * 4];
        let expanded_row = &expanded[0][row * 800 * 4..row * 800 * 4 + 390 * 4];
        for (column, (&expanded_channel, &original_channel)) in
            expanded_row.iter().zip(original).enumerate()
        {
            assert!(
                expanded_channel.abs_diff(original_channel) <= 1,
                "expanded canvas changed row {row}, byte {column}: \
                 {original_channel} -> {expanded_channel}"
            );
        }
    }

    let output = workspace_root().join("target/test-artifacts/sunflower.png");
    std::fs::create_dir_all(output.parent().unwrap()).expect("artifact directory");
    let png = pam_editor_formats::encode_png(&rendered[0], 390, 390).expect("encode PNG");
    std::fs::write(output, png).expect("write render artifact");
}
