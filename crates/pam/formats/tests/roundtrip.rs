use std::sync::Arc;
use std::{collections::BTreeSet, io::Read};

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
fn pam_and_json_roundtrip_through_pam_codec_types() {
    let loaded =
        pam_viewer_formats::load_pam_document(&sample_files("sunflower")).expect("load sunflower");
    let pam = &loaded.document.pam;

    let binary = pam_viewer_core::encode_pam_bytes(pam).expect("encode PAM");
    let binary_roundtrip = pam_viewer_core::decode_pam_bytes(&binary).expect("decode PAM");
    assert_eq!(*pam, binary_roundtrip);

    let json = pam_viewer_core::encode_json(pam).expect("encode JSON");
    let json_roundtrip = pam_viewer_core::decode_json(&json).expect("decode JSON");
    if *pam != json_roundtrip {
        let left = serde_json::to_value(pam).unwrap();
        let right = serde_json::to_value(&json_roundtrip).unwrap();
        panic!(
            "JSON first difference: {}",
            first_json_difference(&left, &right, "$".into()).unwrap_or_default()
        );
    }
}

fn first_json_difference(
    left: &serde_json::Value,
    right: &serde_json::Value,
    path: String,
) -> Option<String> {
    match (left, right) {
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            for (key, left) in left {
                let next = format!("{path}.{key}");
                let Some(right) = right.get(key) else {
                    return Some(format!("{next}: missing"));
                };
                if let Some(difference) = first_json_difference(left, right, next) {
                    return Some(difference);
                }
            }
            None
        }
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => {
            if left.len() != right.len() {
                return Some(format!("{path}: lengths {} != {}", left.len(), right.len()));
            }
            left.iter()
                .zip(right)
                .enumerate()
                .find_map(|(index, (left, right))| {
                    first_json_difference(left, right, format!("{path}[{index}]"))
                })
        }
        _ if left == right => None,
        _ => Some(format!("{path}: {left} != {right}")),
    }
}

#[test]
fn fla_roundtrip_preserves_rendered_timeline_and_images() {
    let loaded =
        pam_viewer_formats::load_pam_document(&sample_files("sunflower")).expect("load sunflower");
    let original = loaded.document;
    let fla = pam_viewer_formats::export_fla(&original, 1200).expect("export FLA");
    let artifact_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/test-artifacts");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact directory");
    std::fs::write(artifact_dir.join("sunflower.fla"), &fla).expect("write FLA artifact");
    let imported = pam_viewer_formats::load_pam_document(&[InputFile::new(
        "sunflower.fla",
        Arc::<[u8]>::from(fla),
    )])
    .expect("import exported FLA")
    .document;

    assert_eq!(original.pam.version, imported.pam.version);
    assert_eq!(original.pam.frame_rate, imported.pam.frame_rate);
    assert_eq!(original.pam.position, imported.pam.position);
    assert_eq!(original.pam.size, imported.pam.size);
    assert_eq!(original.pam.image.len(), imported.pam.image.len());
    for (left, right) in original.pam.image.iter().zip(&imported.pam.image) {
        assert_eq!(left.name, right.name);
        assert_eq!(left.size, right.size);
        assert_slice_close(&left.transform, &right.transform, 1e-6);
    }

    assert_eq!(original.pam.sprite.len(), imported.pam.sprite.len());
    for (left, right) in original.pam.sprite.iter().zip(&imported.pam.sprite) {
        assert_eq!(left.name, right.name);
        assert_eq!(left.frame_rate, right.frame_rate);
        assert_eq!(left.work_area, right.work_area);
        assert_eq!(left.frame.len(), right.frame.len());
    }
    let original_main = original.pam.main_sprite.as_ref().expect("original main");
    let imported_main = imported.pam.main_sprite.as_ref().expect("imported main");
    assert_eq!(original_main.name, imported_main.name);
    assert_eq!(original_main.frame_rate, imported_main.frame_rate);
    assert_eq!(original_main.work_area, imported_main.work_area);
    assert_eq!(original_main.frame.len(), imported_main.frame.len());
    for (frame, (left, right)) in original_main
        .frame
        .iter()
        .zip(&imported_main.frame)
        .enumerate()
    {
        assert_eq!(left.label, right.label);
        assert_eq!(left.stop, right.stop, "stop flag at main frame {frame}");
        assert_eq!(
            left.command, right.command,
            "commands at main frame {frame}"
        );
    }

    assert_eq!(original.images.len(), imported.images.len());
    for (left, right) in original.images.iter().zip(&imported.images) {
        match (left, right) {
            (Some(left), Some(right)) => {
                assert_eq!(left.width, right.width);
                assert_eq!(left.height, right.height);
                assert_eq!(left.rgba, right.rgba);
            }
            (None, None) => {}
            _ => panic!("FLA changed image availability"),
        }
    }

    let mut keys = (0..original.pam.sprite.len())
        .map(pam_viewer_core::SpriteKey::Sprite)
        .collect::<Vec<_>>();
    keys.push(pam_viewer_core::SpriteKey::Main);
    let image_filter = vec![true; original.pam.image.len()];
    let sprite_filter = vec![true; original.pam.sprite.len()];
    for key in keys {
        let left_timeline = original.compiled.timeline(key).expect("original timeline");
        let right_timeline = imported.compiled.timeline(key).expect("imported timeline");
        assert_eq!(left_timeline.len(), right_timeline.len());
        for frame in 0..left_timeline.len() {
            let left = original
                .compiled
                .flatten_frame(
                    &original.pam,
                    &original.images,
                    key,
                    frame,
                    &image_filter,
                    &sprite_filter,
                )
                .expect("flatten original");
            let right = imported
                .compiled
                .flatten_frame(
                    &imported.pam,
                    &imported.images,
                    key,
                    frame,
                    &image_filter,
                    &sprite_filter,
                )
                .expect("flatten imported");
            assert_eq!(
                left.len(),
                right.len(),
                "draw count at {key:?} frame {frame}"
            );
            for (draw, (left, right)) in left.iter().zip(&right).enumerate() {
                assert_eq!(left.image_index, right.image_index);
                assert_eq!(left.additive, right.additive);
                assert_eq!(left.source_rectangle, right.source_rectangle);
                assert_slice_close(&left.matrix, &right.matrix, 2e-4);
                let left_color = [left.color.r, left.color.g, left.color.b, left.color.a];
                let right_color = [right.color.r, right.color.g, right.color.b, right.color.a];
                if left_color
                    .iter()
                    .zip(right_color)
                    .any(|(left, right)| (*left - right).abs() > 2e-5)
                {
                    panic!(
                        "color at {key:?} frame {frame} draw {draw}: {left_color:?} != {right_color:?}"
                    );
                }
                assert!((left.width - right.width).abs() <= 1e-5);
                assert!((left.height - right.height).abs() <= 1e-5);
            }
        }
    }
}

#[test]
fn fla_uses_twinning_xfl_layout_and_sidecar_structure() {
    let loaded =
        pam_viewer_formats::load_pam_document(&sample_files("sunflower")).expect("load sunflower");
    let fla = pam_viewer_formats::export_fla(&loaded.document, 1200).expect("export FLA");
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(fla)).expect("FLA archive");
    let names = archive
        .file_names()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    assert!(names.contains("main.xfl"));
    assert!(names.contains("DOMDocument.xml"));
    assert!(names.contains("PAM.sidecar.json"));
    assert!(names.contains("LIBRARY/main_sprite.xml"));
    assert!(!names.contains("LIBRARY/main.xml"));

    let mut sidecar = String::new();
    archive
        .by_name("PAM.sidecar.json")
        .expect("sidecar")
        .read_to_string(&mut sidecar)
        .expect("read sidecar");
    let sidecar: serde_json::Value = serde_json::from_str(&sidecar).expect("parse sidecar");
    let keys = sidecar
        .as_object()
        .expect("sidecar object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        BTreeSet::from(["image", "main_sprite", "position", "sprite"])
    );
    assert_eq!(
        sidecar["image"].as_array().map(Vec::len),
        Some(loaded.document.pam.image.len())
    );
    assert_eq!(
        sidecar["sprite"].as_array().map(Vec::len),
        Some(loaded.document.pam.sprite.len())
    );

    let mut document = String::new();
    archive
        .by_name("DOMDocument.xml")
        .expect("DOMDocument")
        .read_to_string(&mut document)
        .expect("read DOMDocument");
    assert!(document.contains("href=\"main_sprite.xml\""));
    assert!(document.contains("libraryItemName=\"main_sprite\""));
    assert!(document.contains("<DOMLayer name=\"instance\">"));
}

fn assert_slice_close<T>(left: &[T], right: &[T], tolerance: f64)
where
    T: Copy + Into<f64> + std::fmt::Debug,
{
    assert_eq!(left.len(), right.len());
    for (index, (left, right)) in left.iter().zip(right).enumerate() {
        let difference = ((*left).into() - (*right).into()).abs();
        assert!(
            difference <= tolerance,
            "values differ at {index}: {left:?} != {right:?} ({difference} > {tolerance})"
        );
    }
}
