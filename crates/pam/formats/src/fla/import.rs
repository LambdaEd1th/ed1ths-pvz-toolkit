use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read};
use std::sync::{Arc, LazyLock};

use pam_viewer_core::{
    AddsInfo, FrameInfo, ImageInfo, MovesInfo, PamInfo, RemovesInfo, SpriteInfo,
};
use regex::Regex;
use serde::Deserialize;

use super::xml::{attr_f64, attr_usize, descendants_named};
use crate::loader::{InputFile, LoadedPam, build_document};
use crate::{FormatError, Result};

static SOURCE_SIZE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"_(\d+)x(\d+)(?:_\d+)?$").unwrap());
static COMMAND_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"fscommand\("([^"]+)"(?:,\s*"([^"]*)")?\)"#).unwrap());
static NUMBERED_SOURCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:LIBRARY/)?source/source_(\d+)\.xml$").unwrap());
static NUMBERED_SPRITE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:LIBRARY/)?sprite/sprite_(\d+)\.xml$").unwrap());

#[derive(Clone, Debug, Default)]
struct MainFrameInfo {
    label: Option<String>,
    stop: bool,
    command: Vec<[String; 2]>,
}

#[derive(Debug, Default, Deserialize)]
struct PamSidecar {
    version: Option<i32>,
    #[serde(rename = "frameRate")]
    frame_rate: Option<i32>,
    position: Option<[f64; 2]>,
    size: Option<[f64; 2]>,
    #[serde(rename = "imageNames")]
    image_names: Option<Vec<String>>,
    #[serde(default)]
    image: Vec<ImageExtra>,
    #[serde(default)]
    sprite: Vec<SpriteExtra>,
    main_sprite: Option<SpriteExtra>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ImageExtra {
    name: String,
    size: [i32; 2],
}

#[derive(Clone, Debug, Default, Deserialize)]
struct SpriteExtra {
    name: String,
    frame_rate: f64,
    work_area: Option<[i32; 2]>,
}

#[derive(Clone, Debug)]
enum ResourceRef {
    Image(usize),
    Sprite(usize),
    Main,
}

#[derive(Clone, Debug)]
struct SpriteState {
    resource: ResourceRef,
    transform: [f64; 6],
    color: [f64; 4],
    first_frame: Option<i32>,
    additive: bool,
}

pub(crate) fn import_animation(files: &[InputFile]) -> Result<LoadedPam> {
    let fla = files
        .iter()
        .find(|file| file.path.to_ascii_lowercase().ends_with(".fla"));
    let (source_name, entries) = if let Some(fla) = fla {
        (fla.path.clone(), read_zip(&fla.bytes)?)
    } else {
        (
            files
                .first()
                .and_then(|file| file.path.split('/').next())
                .unwrap_or("XFL")
                .to_string(),
            files
                .iter()
                .map(|file| (normalize_path(&file.path), file.bytes.clone()))
                .collect(),
        )
    };
    import_xfl(source_name, entries, files)
}

fn read_zip(bytes: &[u8]) -> Result<BTreeMap<String, Arc<[u8]>>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut output = BTreeMap::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let name = normalize_path(file.name());
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        output.insert(name, Arc::<[u8]>::from(data));
    }
    Ok(output)
}

fn import_xfl(
    source_name: String,
    entries: BTreeMap<String, Arc<[u8]>>,
    input_files: &[InputFile],
) -> Result<LoadedPam> {
    let document_xml = entry_text(&entries, "DOMDocument.xml")?
        .ok_or_else(|| FormatError::Fla("DOMDocument.xml not found in FLA/XFL".into()))?;
    let (width, height, document_frame_rate, main_flow) = parse_dom_document(&document_xml)?;
    let sidecar = [
        "PAM.sidecar.json",
        "pam.sidecar.json",
        "PAM.sidecar",
        "pam.sidecar",
    ]
    .into_iter()
    .find_map(|path| entry_text(&entries, path).transpose())
    .transpose()?
    .and_then(|text| serde_json::from_str::<PamSidecar>(&text).ok())
    .unwrap_or_default();

    let media = collect_media(&entries);
    let mut sources = BTreeMap::<usize, (String, [i32; 2])>::new();
    for path in entries.keys() {
        let Some(captures) = NUMBERED_SOURCE_RE.captures(path) else {
            continue;
        };
        let index = captures[1].parse::<usize>().unwrap_or(0);
        let Some(xml) = entry_text(&entries, path)? else {
            continue;
        };
        if let Some(source) = parse_source(&xml)? {
            sources.insert(index, source);
        }
    }
    if let Some(names) = sidecar.image_names.as_ref() {
        for (index, (name, _)) in &mut sources {
            if let Some(sidecar_name) = names.get(index.saturating_sub(1))
                && !sidecar_name.is_empty()
            {
                *name = sidecar_name.clone();
            }
        }
    }
    for (index, extra) in sidecar.image.iter().enumerate() {
        let source = sources
            .entry(index + 1)
            .or_insert_with(|| (extra.name.clone(), extra.size));
        source.0.clone_from(&extra.name);
        source.1 = extra.size;
    }

    let mut images = Vec::with_capacity(sources.len());
    for (index, (name, mut size)) in sources {
        if size == [0, 0]
            && let Some(bytes) = media
                .get(name.split('|').next().unwrap_or(&name))
                .or_else(|| media.get(&name.to_ascii_uppercase()))
            && let Ok(image) = image::load_from_memory(bytes)
        {
            size = [image.width() as i32, image.height() as i32];
        }
        let image_xml_path = format!("LIBRARY/image/image_{index}.xml");
        let transform = entry_text(&entries, &image_xml_path)?
            .map(|xml| parse_image_transform(&xml))
            .transpose()?
            .unwrap_or_else(|| vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        images.push(ImageInfo {
            name,
            size: Some(size),
            transform,
        });
    }

    let mut sprite_entries = entries
        .keys()
        .filter_map(|path| {
            NUMBERED_SPRITE_RE
                .captures(path)
                .and_then(|captures| captures[1].parse::<usize>().ok())
                .map(|index| (index, path.clone()))
        })
        .collect::<Vec<_>>();
    sprite_entries.sort_by_key(|(index, _)| *index);
    let sprite_count = sprite_entries.len();
    let version = sidecar.version.unwrap_or(6);
    let frame_rate = sidecar.frame_rate.unwrap_or(document_frame_rate);
    let mut sprites = Vec::with_capacity(sprite_count);
    for (index, path) in sprite_entries {
        let Some(xml) = entry_text(&entries, &path)? else {
            continue;
        };
        let frames = parse_sprite_document(&xml, sprite_count)?;
        let frame_count = frames.len();
        sprites.push(sprite_with_metadata(
            sidecar.sprite.get(index.saturating_sub(1)),
            format!("sprite_{}", index),
            frames,
            frame_rate,
            version,
            frame_count,
        ));
    }

    let main_xml = entry_text(&entries, "LIBRARY/main_sprite.xml")?
        .or(entry_text(&entries, "LIBRARY/main.xml")?);
    let main_sprite = if let Some(xml) = main_xml {
        let mut frames = parse_sprite_document(&xml, sprite_count)?;
        frames.resize_with(frames.len().max(main_flow.len()), FrameInfo::default);
        for (index, flow) in main_flow.into_iter().enumerate() {
            frames[index].label = flow.label;
            frames[index].stop = flow.stop;
            frames[index].command = flow.command;
        }
        let frame_count = frames.len();
        Some(sprite_with_metadata(
            sidecar.main_sprite.as_ref(),
            "main_sprite".into(),
            frames,
            frame_rate,
            version,
            frame_count,
        ))
    } else {
        None
    };

    let pam = PamInfo {
        version,
        frame_rate,
        position: sidecar.position.unwrap_or([0.0, 0.0]),
        size: sidecar.size.unwrap_or([width, height]),
        image: images,
        sprite: sprites,
        main_sprite,
    };
    build_document(source_name, pam, input_files, Some(&media))
}

fn sprite_with_metadata(
    extra: Option<&SpriteExtra>,
    fallback_name: String,
    frame: Vec<FrameInfo>,
    frame_rate: i32,
    version: i32,
    frame_count: usize,
) -> SpriteInfo {
    let name = extra
        .map(|extra| extra.name.clone())
        .unwrap_or(fallback_name);
    let sprite_frame_rate = extra
        .map(|extra| extra.frame_rate)
        .unwrap_or(frame_rate as f64);
    let default_work_area = [0, frame_count.saturating_sub(1) as i32];
    SpriteInfo {
        name: (version >= 4).then_some(name),
        frame_rate: (version >= 4).then_some(sprite_frame_rate),
        work_area: (version >= 5).then_some(
            extra
                .and_then(|extra| extra.work_area)
                .unwrap_or(default_work_area),
        ),
        frame,
    }
}

fn collect_media(entries: &BTreeMap<String, Arc<[u8]>>) -> HashMap<String, Arc<[u8]>> {
    let mut output = HashMap::new();
    for (path, bytes) in entries {
        let normalized = normalize_path(path);
        let lower = normalized.to_ascii_lowercase();
        let Some(position) = lower.rfind("media/") else {
            continue;
        };
        if !lower.ends_with(".png") {
            continue;
        }
        let name = &normalized[position + "media/".len()..normalized.len() - 4];
        output.insert(name.to_string(), bytes.clone());
        output.insert(name.to_ascii_uppercase(), bytes.clone());
    }
    output
}

fn parse_dom_document(text: &str) -> Result<(f64, f64, i32, Vec<MainFrameInfo>)> {
    let document = roxmltree::Document::parse(text)
        .map_err(|error| FormatError::Fla(format!("invalid DOMDocument.xml: {error}")))?;
    let root = document.root_element();
    let width = attr_f64(root, "width", 0.0);
    let height = attr_f64(root, "height", 0.0);
    let frame_rate = attr_usize(root, "frameRate", 30) as i32;
    let mut frames = Vec::<MainFrameInfo>::new();
    let timeline = descendants_named(root, "DOMTimeline")
        .find(|timeline| timeline.attribute("name") == Some("animation"));
    let Some(timeline) = timeline else {
        return Ok((width, height, frame_rate, frames));
    };
    for layer in descendants_named(timeline, "DOMLayer") {
        let layer_name = layer.attribute("name").unwrap_or_default();
        if !matches!(layer_name, "flow" | "command") {
            continue;
        }
        for dom_frame in descendants_named(layer, "DOMFrame") {
            let index = attr_usize(dom_frame, "index", 0);
            if frames.len() <= index {
                frames.resize_with(index + 1, MainFrameInfo::default);
            }
            if layer_name == "flow" {
                if let Some(label) = dom_frame.attribute("name") {
                    frames[index].label = Some(label.into());
                }
                frames[index].stop = descendants_named(dom_frame, "script")
                    .any(|script| text_content(script).contains("stop()"));
            } else {
                for script in descendants_named(dom_frame, "script") {
                    let script = text_content(script);
                    for captures in COMMAND_RE.captures_iter(&script) {
                        frames[index].command.push([
                            captures[1].to_string(),
                            captures
                                .get(2)
                                .map(|value| value.as_str())
                                .unwrap_or("")
                                .to_string(),
                        ]);
                    }
                }
            }
        }
    }
    Ok((width, height, frame_rate, frames))
}

fn text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect()
}

fn parse_source(text: &str) -> Result<Option<(String, [i32; 2])>> {
    let document = roxmltree::Document::parse(text)
        .map_err(|error| FormatError::Fla(format!("invalid source symbol: {error}")))?;
    let Some(bitmap) = descendants_named(document.root_element(), "DOMBitmapInstance").next()
    else {
        return Ok(None);
    };
    let name = bitmap
        .attribute("libraryItemName")
        .unwrap_or_default()
        .strip_prefix("media/")
        .unwrap_or_default()
        .to_string();
    let dimension_name = name.split('|').next().unwrap_or(&name);
    let size = SOURCE_SIZE_RE
        .captures(dimension_name)
        .map(|captures| {
            [
                captures[1].parse().unwrap_or(0),
                captures[2].parse().unwrap_or(0),
            ]
        })
        .unwrap_or([0, 0]);
    Ok(Some((name, size)))
}

fn parse_image_transform(text: &str) -> Result<Vec<f64>> {
    let document = roxmltree::Document::parse(text)
        .map_err(|error| FormatError::Fla(format!("invalid image symbol: {error}")))?;
    Ok(descendants_named(document.root_element(), "Matrix")
        .next()
        .map(matrix_values)
        .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
        .to_vec())
}

fn parse_sprite_document(text: &str, sprite_count: usize) -> Result<Vec<FrameInfo>> {
    let document = roxmltree::Document::parse(text)
        .map_err(|error| FormatError::Fla(format!("invalid sprite symbol: {error}")))?;
    let mut total_frames = 0;
    let mut state_map = BTreeMap::<i32, Vec<Option<SpriteState>>>::new();
    for layer in descendants_named(document.root_element(), "DOMLayer") {
        let Ok(xfl_layer_index) = layer.attribute("name").unwrap_or_default().parse::<i32>() else {
            continue;
        };
        // Twinning reserves XFL layer 0 as an empty timing layer and stores PAM
        // display indices offset by one.
        if xfl_layer_index <= 0 {
            continue;
        }
        let layer_index = xfl_layer_index - 1;
        for dom_frame in descendants_named(layer, "DOMFrame") {
            let start = attr_usize(dom_frame, "index", 0);
            let duration = attr_usize(dom_frame, "duration", 1);
            let end = start + duration;
            total_frames = total_frames.max(end);
            let state = descendants_named(dom_frame, "DOMSymbolInstance")
                .next()
                .and_then(parse_symbol_state);
            let timeline = state_map.entry(layer_index).or_default();
            timeline.resize(end, None);
            if let Some(state) = state {
                for slot in &mut timeline[start..end] {
                    *slot = Some(state.clone());
                }
            }
        }
    }
    let mut frames = vec![FrameInfo::default(); total_frames];
    for (layer_index, timeline) in state_map {
        let mut previous: Option<SpriteState> = None;
        let mut virtual_previous: Option<SpriteState> = None;
        for (time, frame) in frames.iter_mut().enumerate() {
            let current = timeline.get(time).and_then(Clone::clone);
            match (&previous, &current) {
                (Some(_), None) => {
                    frame.remove.push(RemovesInfo { index: layer_index });
                    virtual_previous = None;
                }
                (None, Some(current)) => {
                    append_state(frame, layer_index, current, sprite_count);
                    virtual_previous = Some(identity_state(current.resource.clone()));
                }
                (Some(previous), Some(current))
                    if !same_resource(&previous.resource, &current.resource) =>
                {
                    frame.remove.push(RemovesInfo { index: layer_index });
                    append_state(frame, layer_index, current, sprite_count);
                    virtual_previous = Some(identity_state(current.resource.clone()));
                }
                _ => {}
            }
            if let (Some(current), Some(virtual_state)) = (&current, &virtual_previous)
                && !state_values_equal(virtual_state, current)
            {
                let color_changed = !array_close(&virtual_state.color, &current.color);
                frame.change.push(MovesInfo {
                    index: layer_index,
                    transform: current.transform.to_vec(),
                    color: color_changed.then_some(current.color),
                    source_rectangle: None,
                    sprite_frame_number: current.first_frame,
                });
                virtual_previous = Some(current.clone());
            }
            previous = current;
        }
    }
    Ok(frames)
}

fn parse_symbol_state(node: roxmltree::Node<'_, '_>) -> Option<SpriteState> {
    let library_name = node.attribute("libraryItemName")?;
    let resource = if let Some(value) = library_name.strip_prefix("image/image_") {
        ResourceRef::Image(value.parse::<usize>().ok()?.saturating_sub(1))
    } else if let Some(value) = library_name.strip_prefix("sprite/sprite_") {
        ResourceRef::Sprite(value.parse::<usize>().ok()?.saturating_sub(1))
    } else if library_name == "main" {
        ResourceRef::Main
    } else {
        return None;
    };
    let matrix = descendants_named(node, "Matrix")
        .next()
        .map(matrix_values)
        .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let color = descendants_named(node, "Color")
        .next()
        .map(|color| {
            [
                color_component(color, "redMultiplier", "redOffset"),
                color_component(color, "greenMultiplier", "greenOffset"),
                color_component(color, "blueMultiplier", "blueOffset"),
                color_component(color, "alphaMultiplier", "alphaOffset"),
            ]
        })
        .unwrap_or([1.0; 4]);
    Some(SpriteState {
        resource,
        transform: matrix,
        color,
        first_frame: node
            .attribute("firstFrame")
            .and_then(|value| value.parse().ok()),
        additive: node.attribute("blendMode") == Some("add"),
    })
}

fn color_component(node: roxmltree::Node<'_, '_>, multiplier: &str, offset: &str) -> f64 {
    (attr_f64(node, multiplier, 1.0) * 255.0 + attr_f64(node, offset, 0.0)).clamp(0.0, 255.0)
        / 255.0
}

fn append_state(frame: &mut FrameInfo, layer_index: i32, state: &SpriteState, sprite_count: usize) {
    let (resource, sprite) = match state.resource {
        ResourceRef::Image(index) => (index, false),
        ResourceRef::Sprite(index) => (index, true),
        ResourceRef::Main => (sprite_count, true),
    };
    frame.append.push(AddsInfo {
        index: layer_index,
        name: None,
        resource: resource as u32,
        sprite,
        additive: state.additive,
        preload_frame: state.first_frame.unwrap_or(0),
        time_scale: 1.0,
    });
}

fn identity_state(resource: ResourceRef) -> SpriteState {
    SpriteState {
        resource,
        transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        color: [1.0; 4],
        first_frame: None,
        additive: false,
    }
}

fn same_resource(left: &ResourceRef, right: &ResourceRef) -> bool {
    matches!(
        (left, right),
        (ResourceRef::Main, ResourceRef::Main)
            | (ResourceRef::Image(_), ResourceRef::Image(_))
            | (ResourceRef::Sprite(_), ResourceRef::Sprite(_))
    ) && match (left, right) {
        (ResourceRef::Image(left), ResourceRef::Image(right))
        | (ResourceRef::Sprite(left), ResourceRef::Sprite(right)) => left == right,
        _ => true,
    }
}

fn state_values_equal(left: &SpriteState, right: &SpriteState) -> bool {
    array_close(&left.transform, &right.transform)
        && array_close(&left.color, &right.color)
        && left.first_frame == right.first_frame
        && left.additive == right.additive
}

fn array_close<const N: usize>(left: &[f64; N], right: &[f64; N]) -> bool {
    left.iter()
        .zip(right)
        .all(|(left, right)| (left - right).abs() <= 1e-9)
}

fn matrix_values(node: roxmltree::Node<'_, '_>) -> [f64; 6] {
    [
        attr_f64(node, "a", 1.0),
        attr_f64(node, "b", 0.0),
        attr_f64(node, "c", 0.0),
        attr_f64(node, "d", 1.0),
        attr_f64(node, "tx", 0.0),
        attr_f64(node, "ty", 0.0),
    ]
}

fn entry_text(entries: &BTreeMap<String, Arc<[u8]>>, path: &str) -> Result<Option<String>> {
    let Some(bytes) = find_entry(entries, path) else {
        return Ok(None);
    };
    Ok(Some(std::str::from_utf8(bytes)?.to_string()))
}

fn find_entry<'a>(entries: &'a BTreeMap<String, Arc<[u8]>>, path: &str) -> Option<&'a Arc<[u8]>> {
    let expected = normalize_path(path);
    entries.get(&expected).or_else(|| {
        entries.iter().find_map(|(name, bytes)| {
            (name == &expected || name.ends_with(&format!("/{expected}"))).then_some(bytes)
        })
    })
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches('/').to_string()
}
