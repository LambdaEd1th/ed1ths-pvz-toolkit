//! Export source image assets, independent of timeline, transforms and filters.
use std::collections::HashSet;
use std::io::{Cursor, Write};
use std::sync::atomic::AtomicBool;

use pam_editor_core::{PamDocument, parse_image_file_name};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const MAX_EXPORT_BYTES: usize = 512 * 1024 * 1024;

pub(super) async fn export_images(
    document: &PamDocument,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, String> {
    super::ensure_not_cancelled(cancelled)?;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let mut names = HashSet::new();
    let mut missing = Vec::new();
    let mut exported = 0;
    let mut total_bytes = 0usize;
    for (index, definition) in document.pam.image.iter().enumerate() {
        // Let the worker handle cancellation messages between PNG encodes.
        #[cfg(target_arch = "wasm32")]
        gloo_timers::future::TimeoutFuture::new(0).await;
        super::ensure_not_cancelled(cancelled)?;
        let Some(asset) = document.images.get(index).and_then(Option::as_ref) else {
            missing.push(format!("[{index}] {}", definition.name));
            continue;
        };
        // Encode the actual, untransformed RGBA asset at its original size.
        // Do not use PAM's logical size, stage scale, tint, or frame crop here.
        let png = pam_editor_formats::encode_png(&asset.rgba, asset.width, asset.height)
            .map_err(|error| format!("{}: {error}", definition.name))?;
        total_bytes = total_bytes.saturating_add(png.len());
        if total_bytes > MAX_EXPORT_BYTES {
            return Err("Image ZIP exceeds the 512 MiB export limit".into());
        }
        super::ensure_not_cancelled(cancelled)?;
        let path = unique_image_path(&definition.name, index, &mut names);
        add_file(&mut zip, &path, &png)?;
        exported += 1;
    }
    if exported == 0 {
        return Err("No loaded PAM image assets to export".into());
    }
    if !missing.is_empty() {
        let report = format!(
            "Missing image assets / 未加载的图片素材\nExported / 已导出: {exported}/{}\n\n{}\n",
            document.pam.image.len(),
            missing.join("\n")
        );
        add_file(&mut zip, "_missing-images.txt", report.as_bytes())?;
    }
    super::ensure_not_cancelled(cancelled)?;
    zip.finish()
        .map(|output| output.into_inner())
        .map_err(|error| error.to_string())
}

fn add_file(zip: &mut ZipWriter<Cursor<Vec<u8>>>, name: &str, bytes: &[u8]) -> Result<(), String> {
    zip.start_file(
        name,
        SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
    )
    .map_err(|error| error.to_string())?;
    zip.write_all(bytes).map_err(|error| error.to_string())
}

fn unique_image_path(value: &str, index: usize, names: &mut HashSet<String>) -> String {
    let parsed = parse_image_file_name(value);
    let path = parsed
        .split(['/', '\\'])
        .filter(|part| !matches!(*part, "" | "." | ".."))
        .map(|part| {
            let clean = part
                .chars()
                .take(100)
                .map(|character| {
                    if character.is_control() || "<>:\"|?*".contains(character) {
                        '_'
                    } else {
                        character
                    }
                })
                .collect::<String>();
            let clean = clean.trim_matches([' ', '.']);
            let stem = clean.split('.').next().unwrap_or("").to_ascii_uppercase();
            let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || (stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9'));
            if clean.is_empty() {
                "_".into()
            } else if reserved {
                format!("_{clean}")
            } else {
                clean.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    let stem = if path.is_empty() {
        format!("image_{index}")
    } else if path.to_ascii_lowercase().ends_with(".png") {
        path[..path.len() - 4].to_string()
    } else {
        path
    };
    let mut candidate = format!("{stem}.png");
    let mut suffix = 2;
    while !names.insert(candidate.to_lowercase()) {
        candidate = format!("{stem}_{suffix}.png");
        suffix += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use std::io::Read;
    use std::sync::Arc;

    use pam_editor_core::{ImageAsset, ImageInfo, PamInfo};

    use super::*;

    fn document() -> PamDocument {
        let names = [
            "atlas$Leaf[2]|fallback",
            "leaf",
            "missing",
            "../folder\\Stem",
        ];
        let pam = PamInfo {
            version: 6,
            frame_rate: 30,
            position: [0.0, 0.0],
            size: [500.0, 600.0],
            image: names
                .iter()
                .map(|name| ImageInfo {
                    name: (*name).into(),
                    size: Some([100, 200]),
                    transform: vec![2.0, 0.0, 0.0, 2.0, 30.0, 40.0],
                })
                .collect(),
            sprite: Vec::new(),
            main_sprite: None,
        };
        let asset = ImageAsset::new(
            "source",
            2,
            1,
            Arc::<[u8]>::from([255, 0, 0, 128, 0, 255, 0, 0]),
            Arc::<[u8]>::from([]),
        );
        PamDocument::new(
            "test.pam",
            pam,
            vec![Some(asset.clone()), Some(asset.clone()), None, Some(asset)],
        )
        .unwrap()
    }

    #[test]
    fn exports_all_assets_at_original_size_and_alpha_with_missing_report() {
        let bytes =
            pollster::block_on(export_images(&document(), &AtomicBool::new(false))).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(zip.len(), 4);
        for name in ["Leaf.png", "leaf_2.png", "folder/Stem.png"] {
            let mut png = Vec::new();
            zip.by_name(name).unwrap().read_to_end(&mut png).unwrap();
            let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
            assert_eq!(decoded.dimensions(), (2, 1));
            assert_eq!(decoded.as_raw(), &[255, 0, 0, 128, 0, 255, 0, 0]);
        }
        let mut report = String::new();
        zip.by_name("_missing-images.txt")
            .unwrap()
            .read_to_string(&mut report)
            .unwrap();
        assert!(report.contains("[2] missing") && report.contains("3/4"));
    }

    #[test]
    fn image_export_ignores_sprite_frame_size_and_visibility_settings() {
        use pam_editor_core::{ExportKind, ExportRequest, SpriteKey};

        let document_id = 0x1A6E5;
        super::super::with_documents(|documents| {
            documents.insert(document_id, Arc::new(document()));
        });
        let request = ExportRequest {
            document_id,
            operation_id: document_id,
            kind: ExportKind::ImagesZip,
            sprite: SpriteKey::Sprite(usize::MAX),
            current_frame: usize::MAX,
            frame_range: [10, 0],
            image_filter: vec![false; 4],
            sprite_filter: vec![false],
            size: [0, 0],
            render_scale: 0,
            fps: 0,
        };
        let result = pollster::block_on(super::super::export_document(
            request,
            &AtomicBool::new(false),
        ));
        super::super::with_documents(|documents| {
            documents.remove(&document_id);
        });
        let zip = zip::ZipArchive::new(Cursor::new(result.unwrap())).unwrap();
        assert_eq!(
            zip.file_names()
                .filter(|name| name.ends_with(".png"))
                .count(),
            3
        );
    }

    #[test]
    fn image_names_are_safe_and_case_insensitive_collisions_are_preserved() {
        let mut names = HashSet::new();
        assert_eq!(
            unique_image_path("../../C:\\dir/CON.png", 0, &mut names),
            "C_/dir/_CON.png"
        );
        assert_eq!(unique_image_path("", 1, &mut names), "image_1.png");
        assert_eq!(
            unique_image_path("image_1.PNG", 2, &mut names),
            "image_1_2.png"
        );
        assert_eq!(unique_image_path("\"bad\"?*", 3, &mut names), "_bad___.png");
        assert_eq!(
            unique_image_path("folder/../中文", 4, &mut names),
            "folder/中文.png"
        );
    }

    #[test]
    fn cancellation_and_empty_assets_do_not_produce_a_zip() {
        let mut document = document();
        let error =
            pollster::block_on(export_images(&document, &AtomicBool::new(true))).unwrap_err();
        assert!(error.contains("cancelled"));
        document.images.clear();
        assert!(pollster::block_on(export_images(&document, &AtomicBool::new(false))).is_err());
        document.pam.image.clear();
        assert!(pollster::block_on(export_images(&document, &AtomicBool::new(false))).is_err());
    }
}
