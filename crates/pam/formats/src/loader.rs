use std::collections::HashMap;
use std::sync::Arc;

use image::ImageReader;
use pam_viewer_core::{ImageAsset, PamDocument, PamInfo, parse_image_file_name};

use crate::{FormatError, Result, TextFormat, decode_text};

#[derive(Clone, Debug)]
pub struct InputFile {
    pub path: String,
    pub bytes: Arc<[u8]>,
}

impl InputFile {
    pub fn new(path: impl Into<String>, bytes: impl Into<Arc<[u8]>>) -> Self {
        Self {
            path: path.into().replace('\\', "/"),
            bytes: bytes.into(),
        }
    }

    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

#[derive(Clone, Debug)]
pub struct LoadedPam {
    pub document: PamDocument,
    pub loaded_images: usize,
    pub missing_images: Vec<String>,
}

pub fn load_pam_document(files: &[InputFile]) -> Result<LoadedPam> {
    if files.is_empty() {
        return Err(FormatError::AnimationNotFound);
    }

    // FLA/XFL is handled by the dedicated importer before ordinary PAM sources.
    if files.iter().any(|file| ends_with(&file.path, ".fla"))
        || files
            .iter()
            .any(|file| file.path.to_ascii_lowercase().ends_with("domdocument.xml"))
    {
        return crate::fla::import_animation(files);
    }

    let source = find_source(files).ok_or(FormatError::AnimationNotFound)?;
    let pam = decode_source(source)?;
    build_document(source.path.clone(), pam, files, None)
}

pub(crate) fn build_document(
    source_name: String,
    pam: PamInfo,
    files: &[InputFile],
    embedded_images: Option<&HashMap<String, Arc<[u8]>>>,
) -> Result<LoadedPam> {
    let mut png_files = HashMap::<String, Arc<[u8]>>::new();
    for file in files {
        if ends_with(&file.path, ".png") {
            png_files.insert(file.file_name().to_ascii_uppercase(), file.bytes.clone());
            png_files.insert(file.path.to_ascii_uppercase(), file.bytes.clone());
        }
    }

    let load_image = |definition: &pam_viewer_core::ImageInfo| {
        let base_name = parse_image_file_name(&definition.name);
        let alternate = definition
            .name
            .split_once('|')
            .map(|(_, alternate)| alternate.to_string());
        let candidates = [Some(base_name), alternate];
        let bytes = candidates.iter().flatten().find_map(|candidate| {
            embedded_images
                .and_then(|images| {
                    images
                        .get(candidate)
                        .or_else(|| images.get(&candidate.to_ascii_uppercase()))
                        .cloned()
                })
                .or_else(|| {
                    png_files
                        .get(&format!("{candidate}.png").to_ascii_uppercase())
                        .cloned()
                })
        });
        let Some(bytes) = bytes else {
            return (None, Some(definition.name.clone()));
        };
        match decode_image(&definition.name, bytes) {
            Ok(asset) => (Some(asset), None),
            Err(_) => (None, Some(definition.name.clone())),
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    let decoded = if pam.image.len() >= 8 {
        use rayon::prelude::*;
        pam.image.par_iter().map(load_image).collect::<Vec<_>>()
    } else {
        pam.image.iter().map(load_image).collect::<Vec<_>>()
    };
    #[cfg(target_arch = "wasm32")]
    let decoded = pam.image.iter().map(load_image).collect::<Vec<_>>();
    let loaded_images = decoded.iter().filter(|(asset, _)| asset.is_some()).count();
    let missing_images = decoded
        .iter()
        .filter_map(|(_, missing)| missing.clone())
        .collect();
    let images = decoded.into_iter().map(|(asset, _)| asset).collect();
    let document = PamDocument::new(source_name, pam, images)?;
    Ok(LoadedPam {
        document,
        loaded_images,
        missing_images,
    })
}

fn find_source(files: &[InputFile]) -> Option<&InputFile> {
    [
        ".pam.json",
        ".json",
        ".pam.yaml",
        ".pam.yml",
        ".yaml",
        ".yml",
        ".pam.toml",
        ".toml",
        ".pam",
    ]
    .into_iter()
    .find_map(|extension| files.iter().find(|file| ends_with(&file.path, extension)))
}

fn decode_source(file: &InputFile) -> Result<PamInfo> {
    let lower = file.path.to_ascii_lowercase();
    if lower.ends_with(".json") {
        decode_text(std::str::from_utf8(&file.bytes)?, TextFormat::Json)
    } else if lower.ends_with(".yaml") || lower.ends_with(".yml") {
        decode_text(std::str::from_utf8(&file.bytes)?, TextFormat::Yaml)
    } else if lower.ends_with(".toml") {
        decode_text(std::str::from_utf8(&file.bytes)?, TextFormat::Toml)
    } else {
        Ok(pam_viewer_core::decode_pam_bytes(&file.bytes)?)
    }
}

fn decode_image(name: &str, bytes: Arc<[u8]>) -> Result<ImageAsset> {
    let image = ImageReader::new(std::io::Cursor::new(bytes.as_ref()))
        .with_guessed_format()?
        .decode()?
        .to_rgba8();
    let (width, height) = image.dimensions();
    Ok(ImageAsset::new(
        name,
        width,
        height,
        Arc::<[u8]>::from(image.into_raw()),
        bytes,
    ))
}

fn ends_with(value: &str, extension: &str) -> bool {
    value.to_ascii_lowercase().ends_with(extension)
}
