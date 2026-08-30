use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use pam_viewer_core::{ImageInfo, PamDocument, PamInfo, SpriteInfo};
use serde::Serialize;
use zip::write::SimpleFileOptions;

use super::xml::{XFL_NS, XSI_NS, XmlBuilder, attrs};
use crate::{FormatError, Result, encode_png};

#[derive(Clone, Debug, PartialEq)]
struct FrameElement {
    resource: usize,
    is_sprite: bool,
    first_frame: Option<usize>,
    transform: [f64; 6],
    color: [f64; 4],
}

#[derive(Clone, Debug)]
struct DomFrameData {
    start_frame: usize,
    duration: usize,
    element: Option<FrameElement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LayerState {
    Changed,
    Removed,
}

#[derive(Clone, Debug)]
struct LayerModel {
    state: Option<LayerState>,
    resource: usize,
    is_sprite: bool,
    transform: [f64; 6],
    color: [f64; 4],
    frame_start: usize,
    frame_duration: usize,
}

#[derive(Serialize)]
struct PamSidecar<'a> {
    position: [f64; 2],
    image: Vec<ImageExtra<'a>>,
    sprite: Vec<SpriteExtra<'a>>,
    main_sprite: Option<SpriteExtra<'a>>,
}

#[derive(Serialize)]
struct ImageExtra<'a> {
    name: &'a str,
    size: [i32; 2],
}

#[derive(Serialize)]
struct SpriteExtra<'a> {
    name: &'a str,
    frame_rate: f64,
    work_area: Option<[i32; 2]>,
}

pub fn generate_xfl(pam: &PamInfo, resolution: u32) -> Result<BTreeMap<String, String>> {
    generate_xfl_with_cancel(pam, resolution, None)
}

fn generate_xfl_with_cancel(
    pam: &PamInfo,
    resolution: u32,
    cancelled: Option<&AtomicBool>,
) -> Result<BTreeMap<String, String>> {
    ensure_not_cancelled(cancelled)?;
    let mut files = BTreeMap::new();
    files.insert("main.xfl".into(), "PROXY-CS5".into());
    files.insert("DOMDocument.xml".into(), generate_dom_document(pam)?);
    files.insert("PAM.sidecar.json".into(), generate_sidecar(pam)?);
    let image_entries = |(index, image): (usize, &ImageInfo)| -> Result<_> {
        ensure_not_cancelled(cancelled)?;
        Ok([
            (
                format!("LIBRARY/source/source_{}.xml", index + 1),
                generate_source(index, image, resolution.max(1)),
            ),
            (
                format!("LIBRARY/image/image_{}.xml", index + 1),
                generate_image(index, image)?,
            ),
        ])
    };
    let sprite_entry = |(index, sprite): (usize, &SpriteInfo)| -> Result<_> {
        ensure_not_cancelled(cancelled)?;
        Ok((
            format!("LIBRARY/sprite/sprite_{}.xml", index + 1),
            generate_sprite(pam, Some(index), sprite)?,
        ))
    };
    #[cfg(not(target_arch = "wasm32"))]
    let (image_entries, sprite_entries) = if pam.image.len() + pam.sprite.len() >= 8 {
        use rayon::prelude::*;
        rayon::join(
            || {
                pam.image
                    .par_iter()
                    .enumerate()
                    .map(image_entries)
                    .collect::<Result<Vec<_>>>()
            },
            || {
                pam.sprite
                    .par_iter()
                    .enumerate()
                    .map(sprite_entry)
                    .collect::<Result<Vec<_>>>()
            },
        )
    } else {
        (
            pam.image
                .iter()
                .enumerate()
                .map(image_entries)
                .collect::<Result<Vec<_>>>(),
            pam.sprite
                .iter()
                .enumerate()
                .map(sprite_entry)
                .collect::<Result<Vec<_>>>(),
        )
    };
    #[cfg(target_arch = "wasm32")]
    let (image_entries, sprite_entries) = (
        pam.image
            .iter()
            .enumerate()
            .map(image_entries)
            .collect::<Result<Vec<_>>>(),
        pam.sprite
            .iter()
            .enumerate()
            .map(sprite_entry)
            .collect::<Result<Vec<_>>>(),
    );
    for entries in image_entries? {
        files.extend(entries);
    }
    files.extend(sprite_entries?);
    if let Some(main) = pam.main_sprite.as_ref() {
        ensure_not_cancelled(cancelled)?;
        files.insert(
            "LIBRARY/main_sprite.xml".into(),
            generate_sprite(pam, None, main)?,
        );
    }
    Ok(files)
}

pub fn export_fla(document: &PamDocument, resolution: u32) -> Result<Vec<u8>> {
    export_fla_with_cancel(document, resolution, None)
}

pub fn export_fla_with_cancel(
    document: &PamDocument,
    resolution: u32,
    cancelled: Option<&AtomicBool>,
) -> Result<Vec<u8>> {
    ensure_not_cancelled(cancelled)?;
    let xfl = generate_xfl_with_cancel(&document.pam, resolution, cancelled)?;
    let encode_media = |(definition, asset): (&ImageInfo, &Option<pam_viewer_core::ImageAsset>)| {
        ensure_not_cancelled(cancelled)?;
        let Some(asset) = asset.as_ref() else {
            return Ok(None);
        };
        let media_name = definition
            .name
            .split('|')
            .next()
            .unwrap_or(&definition.name);
        Ok::<_, crate::FormatError>(Some((
            format!("LIBRARY/media/{media_name}.png"),
            encode_png(&asset.rgba, asset.width, asset.height)?,
        )))
    };
    #[cfg(not(target_arch = "wasm32"))]
    let media = if document.images.len() >= 4 {
        use rayon::prelude::*;
        document
            .pam
            .image
            .par_iter()
            .zip(&document.images)
            .map(encode_media)
            .collect::<Result<Vec<_>>>()?
    } else {
        document
            .pam
            .image
            .iter()
            .zip(&document.images)
            .map(encode_media)
            .collect::<Result<Vec<_>>>()?
    };
    #[cfg(target_arch = "wasm32")]
    let media = document
        .pam
        .image
        .iter()
        .zip(&document.images)
        .map(encode_media)
        .collect::<Result<Vec<_>>>()?;
    let cursor = Cursor::new(Vec::new());
    let mut archive = zip::ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, text) in xfl {
        ensure_not_cancelled(cancelled)?;
        archive.start_file(name, options)?;
        archive.write_all(text.as_bytes())?;
    }
    let mut has_media = false;
    for entry in media {
        ensure_not_cancelled(cancelled)?;
        let Some((name, bytes)) = entry else {
            continue;
        };
        archive.start_file(name, options)?;
        archive.write_all(&bytes)?;
        has_media = true;
    }
    if !has_media {
        archive.add_directory("LIBRARY/media/", options)?;
    }
    Ok(archive.finish()?.into_inner())
}

fn ensure_not_cancelled(cancelled: Option<&AtomicBool>) -> Result<()> {
    if cancelled.is_some_and(|cancelled| cancelled.load(Ordering::Relaxed)) {
        Err(FormatError::Cancelled)
    } else {
        Ok(())
    }
}

fn generate_source(index: usize, image: &ImageInfo, resolution: u32) -> String {
    let mut xml = XmlBuilder::default();
    let source_name = format!("source/source_{}", index + 1);
    let media_name = image.name.split('|').next().unwrap_or(&image.name);
    xml.open(
        "DOMSymbolItem",
        attrs([
            ("xmlns:xsi", XSI_NS.into()),
            ("xmlns", XFL_NS.into()),
            ("name", source_name),
            ("symbolType", "graphic".into()),
        ]),
    );
    xml.open("timeline", attrs([]));
    xml.open(
        "DOMTimeline",
        attrs([("name", format!("source_{}", index + 1))]),
    );
    xml.open("layers", attrs([]));
    xml.open("DOMLayer", attrs([]));
    xml.open("frames", attrs([]));
    xml.open("DOMFrame", attrs([("index", "0".into())]));
    xml.open("elements", attrs([]));
    xml.open(
        "DOMBitmapInstance",
        attrs([("libraryItemName", format!("media/{media_name}"))]),
    );
    xml.open("matrix", attrs([]));
    let scale = 1200.0 / resolution as f64;
    xml.empty(
        "Matrix",
        attrs([("a", format_float(scale)), ("d", format_float(scale))]),
    );
    xml.close("matrix");
    xml.close("DOMBitmapInstance");
    xml.close("elements");
    xml.close("DOMFrame");
    xml.close("frames");
    xml.close("DOMLayer");
    xml.close("layers");
    xml.close("DOMTimeline");
    xml.close("timeline");
    xml.close("DOMSymbolItem");
    xml.finish()
}

fn generate_image(index: usize, image: &ImageInfo) -> Result<String> {
    let matrix = image_transform_matrix(&image.transform)?;
    let mut xml = XmlBuilder::default();
    xml.open(
        "DOMSymbolItem",
        attrs([
            ("xmlns:xsi", XSI_NS.into()),
            ("xmlns", XFL_NS.into()),
            ("name", format!("image/image_{}", index + 1)),
            ("symbolType", "graphic".into()),
        ]),
    );
    xml.open("timeline", attrs([]));
    xml.open(
        "DOMTimeline",
        attrs([("name", format!("image_{}", index + 1))]),
    );
    xml.open("layers", attrs([]));
    xml.open("DOMLayer", attrs([]));
    xml.open("frames", attrs([]));
    xml.open("DOMFrame", attrs([("index", "0".into())]));
    xml.open("elements", attrs([]));
    xml.open(
        "DOMSymbolInstance",
        attrs([
            ("libraryItemName", format!("source/source_{}", index + 1)),
            ("symbolType", "graphic".into()),
            ("loop", "loop".into()),
        ]),
    );
    xml.open("matrix", attrs([]));
    xml.empty("Matrix", matrix_attributes_f64(matrix));
    xml.close("matrix");
    xml.close("DOMSymbolInstance");
    xml.close("elements");
    xml.close("DOMFrame");
    xml.close("frames");
    xml.close("DOMLayer");
    xml.close("layers");
    xml.close("DOMTimeline");
    xml.close("timeline");
    xml.close("DOMSymbolItem");
    Ok(xml.finish())
}

fn generate_sprite(pam: &PamInfo, index: Option<usize>, sprite: &SpriteInfo) -> Result<String> {
    let (symbol_name, timeline_name) = match index {
        None => ("main_sprite".into(), "main_sprite".into()),
        Some(index) => (
            format!("sprite/sprite_{}", index + 1),
            format!("sprite_{}", index + 1),
        ),
    };
    let layers = build_frame_runs(pam, sprite)?;
    let mut xml = XmlBuilder::default();
    xml.open(
        "DOMSymbolItem",
        attrs([
            ("xmlns:xsi", XSI_NS.into()),
            ("xmlns", XFL_NS.into()),
            ("name", symbol_name),
            ("symbolType", "graphic".into()),
        ]),
    );
    xml.open("timeline", attrs([]));
    xml.open("DOMTimeline", attrs([("name", timeline_name)]));
    xml.open("layers", attrs([]));
    for (layer_index, frames) in layers.iter().rev() {
        xml.open("DOMLayer", attrs([("name", layer_index.to_string())]));
        xml.open("frames", attrs([]));
        for frame in frames {
            xml.open(
                "DOMFrame",
                attrs([
                    ("index", frame.start_frame.to_string()),
                    ("duration", frame.duration.to_string()),
                ]),
            );
            xml.open("elements", attrs([]));
            if let Some(element) = frame.element.as_ref() {
                let library_name = if element.is_sprite {
                    format!("sprite/sprite_{}", element.resource + 1)
                } else {
                    format!("image/image_{}", element.resource + 1)
                };
                let mut attributes = attrs([
                    ("libraryItemName", library_name),
                    ("symbolType", "graphic".into()),
                    ("loop", "loop".into()),
                ]);
                if let Some(first_frame) = element.first_frame {
                    attributes.insert("firstFrame", first_frame.to_string());
                }
                xml.open("DOMSymbolInstance", attributes);
                xml.open("matrix", attrs([]));
                xml.empty("Matrix", matrix_attributes_f64(element.transform));
                xml.close("matrix");
                xml.open("color", attrs([]));
                xml.empty(
                    "Color",
                    attrs([
                        ("redMultiplier", format_float(element.color[0])),
                        ("greenMultiplier", format_float(element.color[1])),
                        ("blueMultiplier", format_float(element.color[2])),
                        ("alphaMultiplier", format_float(element.color[3])),
                    ]),
                );
                xml.close("color");
                xml.close("DOMSymbolInstance");
            }
            xml.close("elements");
            xml.close("DOMFrame");
        }
        xml.close("frames");
        xml.close("DOMLayer");
    }
    xml.close("layers");
    xml.close("DOMTimeline");
    xml.close("timeline");
    xml.close("DOMSymbolItem");
    Ok(xml.finish())
}

fn build_frame_runs(
    pam: &PamInfo,
    sprite: &SpriteInfo,
) -> Result<BTreeMap<i32, Vec<DomFrameData>>> {
    let mut layers = BTreeMap::<i32, LayerModel>::new();
    let mut output = BTreeMap::<i32, Vec<DomFrameData>>::new();

    for (frame_index, frame) in sprite.frame.iter().enumerate() {
        for remove in &frame.remove {
            if let Some(layer) = layers.get_mut(&remove.index) {
                layer.state = Some(LayerState::Removed);
            }
        }
        for append in &frame.append {
            layers.insert(
                append.index,
                LayerModel {
                    state: None,
                    resource: append.resource as usize,
                    is_sprite: append.sprite,
                    transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    color: [1.0; 4],
                    frame_start: frame_index,
                    frame_duration: frame_index,
                },
            );
            let mut frames = Vec::new();
            if frame_index > 0 {
                frames.push(DomFrameData {
                    start_frame: 0,
                    duration: frame_index,
                    element: None,
                });
            }
            output.insert(append.index + 1, frames);
        }
        for change in &frame.change {
            let Some(layer) = layers.get_mut(&change.index) else {
                continue;
            };
            layer.state = Some(LayerState::Changed);
            layer.transform = image_transform_matrix(&change.transform)?;
            if let Some(color) = change.color {
                layer.color = color;
            }
        }

        let indices = layers.keys().copied().collect::<Vec<_>>();
        let mut removed = Vec::new();
        for layer_index in indices {
            let layer = layers.get_mut(&layer_index).expect("known layer");
            let frames = output.entry(layer_index + 1).or_default();
            if layer.state.is_some()
                && let Some(previous) = frames.last_mut()
            {
                previous.duration = layer.frame_duration;
            }
            match layer.state {
                Some(LayerState::Changed) => {
                    let first_frame = if layer.is_sprite {
                        let child = pam.sprite.get(layer.resource).ok_or_else(|| {
                            FormatError::Fla(format!(
                                "sprite resource {} is out of range",
                                layer.resource
                            ))
                        })?;
                        let child_frames = child.frame.len().max(1);
                        Some((frame_index - layer.frame_start) % child_frames)
                    } else {
                        None
                    };
                    frames.push(DomFrameData {
                        start_frame: frame_index,
                        duration: 0,
                        element: Some(FrameElement {
                            resource: layer.resource,
                            is_sprite: layer.is_sprite,
                            first_frame,
                            transform: layer.transform,
                            color: layer.color,
                        }),
                    });
                    layer.state = None;
                    layer.frame_duration = 0;
                }
                Some(LayerState::Removed) => removed.push(layer_index),
                None => {}
            }
            layer.frame_duration += 1;
        }
        for layer_index in removed {
            layers.remove(&layer_index);
        }
    }

    for (layer_index, layer) in layers {
        if let Some(previous) = output.entry(layer_index + 1).or_default().last_mut() {
            previous.duration = layer.frame_duration;
        }
    }
    output.insert(
        0,
        vec![DomFrameData {
            start_frame: 0,
            duration: sprite.frame.len(),
            element: None,
        }],
    );
    Ok(output)
}

fn generate_dom_document(pam: &PamInfo) -> Result<String> {
    let main = pam.main_sprite.as_ref();
    let total_frames = main.map_or(0, |sprite| sprite.frame.len());
    let mut xml = XmlBuilder::default();
    xml.open(
        "DOMDocument",
        attrs([
            ("xmlns:xsi", XSI_NS.into()),
            ("xmlns", XFL_NS.into()),
            ("width", pam.size[0].to_string()),
            ("height", pam.size[1].to_string()),
            ("frameRate", pam.frame_rate.to_string()),
            ("xflVersion", "2.971".into()),
        ]),
    );
    xml.open("folders", attrs([]));
    for folder in ["media", "source", "image", "sprite"] {
        xml.empty(
            "DOMFolderItem",
            attrs([("name", folder.into()), ("isExpanded", "false".into())]),
        );
    }
    xml.close("folders");
    xml.open("media", attrs([]));
    for image in &pam.image {
        let name = image.name.split('|').next().unwrap_or(&image.name);
        xml.empty(
            "DOMBitmapItem",
            attrs([
                ("name", format!("media/{name}")),
                ("href", format!("media/{name}.png")),
            ]),
        );
    }
    xml.close("media");
    xml.open("symbols", attrs([]));
    for index in 0..pam.image.len() {
        xml.empty(
            "Include",
            attrs([("href", format!("source/source_{}.xml", index + 1))]),
        );
    }
    for index in 0..pam.image.len() {
        xml.empty(
            "Include",
            attrs([("href", format!("image/image_{}.xml", index + 1))]),
        );
    }
    for index in 0..pam.sprite.len() {
        xml.empty(
            "Include",
            attrs([("href", format!("sprite/sprite_{}.xml", index + 1))]),
        );
    }
    if pam.main_sprite.is_some() {
        xml.empty("Include", attrs([("href", "main_sprite.xml".into())]));
    }
    xml.close("symbols");
    xml.open("timelines", attrs([]));
    xml.open("DOMTimeline", attrs([("name", "animation".into())]));
    xml.open("layers", attrs([]));
    if let Some(main) = main {
        generate_flow_layer(&mut xml, main, total_frames);
        generate_command_layer(&mut xml, main, total_frames);
    } else {
        generate_empty_layer(&mut xml, "flow");
        generate_empty_layer(&mut xml, "command");
    }
    xml.open("DOMLayer", attrs([("name", "instance".into())]));
    xml.open("frames", attrs([]));
    if main.is_some() {
        xml.open(
            "DOMFrame",
            attrs([
                ("index", "0".into()),
                ("duration", total_frames.to_string()),
            ]),
        );
        xml.open("elements", attrs([]));
        xml.empty(
            "DOMSymbolInstance",
            attrs([
                ("libraryItemName", "main_sprite".into()),
                ("symbolType", "graphic".into()),
                ("loop", "loop".into()),
            ]),
        );
        xml.close("elements");
        xml.close("DOMFrame");
    }
    xml.close("frames");
    xml.close("DOMLayer");
    xml.close("layers");
    xml.close("DOMTimeline");
    xml.close("timelines");
    xml.close("DOMDocument");
    Ok(xml.finish())
}

fn generate_empty_layer(xml: &mut XmlBuilder, name: &str) {
    xml.open("DOMLayer", attrs([("name", name.into())]));
    xml.open("frames", attrs([]));
    xml.close("frames");
    xml.close("DOMLayer");
}

fn generate_flow_layer(xml: &mut XmlBuilder, sprite: &SpriteInfo, total_frames: usize) {
    xml.open("DOMLayer", attrs([("name", "flow".into())]));
    xml.open("frames", attrs([]));
    let mut cursor = 0;
    for (index, frame) in sprite.frame.iter().enumerate() {
        if frame.label.is_none() && !frame.stop {
            continue;
        }
        push_empty_frame(xml, cursor, index.saturating_sub(cursor));
        let mut attributes = attrs([("index", index.to_string())]);
        if let Some(label) = frame.label.as_ref() {
            attributes.insert("name", label.clone());
            attributes.insert("labelType", "name".into());
        }
        xml.open("DOMFrame", attributes);
        xml.open("elements", attrs([]));
        xml.close("elements");
        if frame.stop {
            xml.open("Actionscript", attrs([]));
            xml.open("script", attrs([]));
            xml.raw("<![CDATA[stop();]]>");
            xml.close("script");
            xml.close("Actionscript");
        }
        xml.close("DOMFrame");
        cursor = index + 1;
    }
    push_empty_frame(xml, cursor, total_frames.saturating_sub(cursor));
    xml.close("frames");
    xml.close("DOMLayer");
}

fn generate_command_layer(xml: &mut XmlBuilder, sprite: &SpriteInfo, total_frames: usize) {
    xml.open("DOMLayer", attrs([("name", "command".into())]));
    xml.open("frames", attrs([]));
    let mut cursor = 0;
    for (index, frame) in sprite.frame.iter().enumerate() {
        if frame.command.is_empty() {
            continue;
        }
        push_empty_frame(xml, cursor, index.saturating_sub(cursor));
        xml.open("DOMFrame", attrs([("index", index.to_string())]));
        xml.open("Actionscript", attrs([]));
        xml.open("script", attrs([]));
        let commands = frame
            .command
            .iter()
            .map(|[command, argument]| {
                format!(
                    "fscommand(\"{}\", \"{}\");",
                    escape_actionscript(command),
                    escape_actionscript(argument)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        xml.raw(format!("<![CDATA[{commands}\n]]>"));
        xml.close("script");
        xml.close("Actionscript");
        xml.close("DOMFrame");
        cursor = index + 1;
    }
    push_empty_frame(xml, cursor, total_frames.saturating_sub(cursor));
    xml.close("frames");
    xml.close("DOMLayer");
}

fn push_empty_frame(xml: &mut XmlBuilder, index: usize, duration: usize) {
    if duration == 0 {
        return;
    }
    xml.empty(
        "DOMFrame",
        attrs([
            ("index", index.to_string()),
            ("duration", duration.to_string()),
        ]),
    );
}

fn generate_sidecar(pam: &PamInfo) -> Result<String> {
    Ok(serde_json::to_string_pretty(&PamSidecar {
        position: pam.position,
        image: pam
            .image
            .iter()
            .map(|image| ImageExtra {
                name: &image.name,
                size: image.size.unwrap_or([0, 0]),
            })
            .collect(),
        sprite: pam
            .sprite
            .iter()
            .map(|sprite| sprite_extra(sprite, pam.version))
            .collect(),
        main_sprite: pam
            .main_sprite
            .as_ref()
            .map(|sprite| sprite_extra(sprite, pam.version)),
    })?)
}

fn sprite_extra(sprite: &SpriteInfo, version: i32) -> SpriteExtra<'_> {
    let frame_rate = sprite.frame_rate.unwrap_or(0.0);
    let default_work_area = [0, sprite.frame.len().saturating_sub(1) as i32];
    let work_area = if version < 5 || sprite.work_area == Some(default_work_area) {
        None
    } else {
        sprite.work_area
    };
    SpriteExtra {
        name: sprite.name.as_deref().unwrap_or(""),
        frame_rate,
        work_area,
    }
}

fn image_transform_matrix(values: &[f64]) -> Result<[f64; 6]> {
    match values {
        [x, y] => Ok([1.0, 0.0, 0.0, 1.0, *x, *y]),
        [angle, x, y] => {
            let (sin, cos) = angle.sin_cos();
            Ok([cos, sin, -sin, cos, *x, *y])
        }
        [a, b, c, d, x, y] => Ok([*a, *b, *c, *d, *x, *y]),
        values => Err(FormatError::Fla(format!(
            "image transform must have 2, 3, or 6 values, got {}",
            values.len()
        ))),
    }
}

fn matrix_attributes_f64(matrix: [f64; 6]) -> BTreeMap<&'static str, String> {
    attrs([
        ("a", format_float(matrix[0])),
        ("b", format_float(matrix[1])),
        ("c", format_float(matrix[2])),
        ("d", format_float(matrix[3])),
        ("tx", format_float(matrix[4])),
        ("ty", format_float(matrix[5])),
    ])
}

fn format_float(value: f64) -> String {
    format!("{value:.6}")
}

fn escape_actionscript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
