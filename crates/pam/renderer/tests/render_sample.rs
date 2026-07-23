use std::sync::Arc;

use pam_viewer_core::SpriteKey;
use pam_viewer_formats::InputFile;

fn sample_files(name: &str) -> Vec<InputFile> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("sample")
        .join(name);
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
        pam_viewer_formats::load_pam_document(&sample_files("sunflower")).expect("load sunflower");
    let document = Arc::new(loaded.document);
    let frame_count = document
        .pam
        .main_sprite
        .as_ref()
        .expect("main sprite")
        .frame
        .len();
    let frames = (0..frame_count.min(6)).collect::<Vec<_>>();
    let rendered = pollster::block_on(pam_viewer_renderer::render_offscreen_frames(
        document.clone(),
        SpriteKey::Main,
        &frames,
        &vec![true; document.pam.image.len()],
        &vec![true; document.pam.sprite.len()],
        390,
        390,
    ))
    .expect("render frames");

    let visible_pixels = rendered[0]
        .chunks_exact(4)
        .filter(|pixel| pixel[3] != 0)
        .count();
    assert!(visible_pixels > 500, "only {visible_pixels} visible pixels");
    assert_eq!(rendered.len(), frames.len());
    assert_ne!(rendered[0], rendered[1], "animation frames must differ");

    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/test-artifacts/sunflower.png");
    std::fs::create_dir_all(output.parent().unwrap()).expect("artifact directory");
    let png = pam_viewer_formats::encode_png(&rendered[0], 390, 390).expect("encode PNG");
    std::fs::write(output, png).expect("write render artifact");
}
