//! Read-only extraction over the logical catalog. All actions use the same resolver.
use crate::domain::{ArchiveDocument, PacketDocument};
use crate::editing::{PacketEdits, RemovedPackets};
use crate::resources::{
    AtlasRegion, MappedResource, MappingState, ResourceCatalog, ResourceEntry, ResourceLocation,
    normalize_path,
};
use image::{ImageFormat, RgbaImage};
use std::collections::BTreeSet;
use std::io::{Cursor, Write};
use std::sync::Arc;
use toolkit_ui::{ToolFile, ToolKind};

pub const MAX_EXPORT_BYTES: usize = 512 * 1024 * 1024;

pub fn can_extract(row: &MappedResource) -> bool {
    row.locations.len() == 1
        && matches!(
            row.state,
            MappingState::File | MappingState::AtlasChild | MappingState::Unlisted
        )
}

pub fn can_preview(row: &MappedResource) -> bool {
    can_extract(row)
        && (row.state == MappingState::AtlasChild
            || row.entry.atlas
            || row.locations.first().is_some_and(|location| {
                [".PTX", ".PNG", ".JPEG", ".JPG", ".WEBP", ".BMP"]
                    .iter()
                    .any(|extension| location.path.to_ascii_uppercase().ends_with(extension))
            }))
}

pub fn tool_kind(row: &MappedResource) -> Option<ToolKind> {
    row.locations
        .first()
        .and_then(|location| ToolKind::from_path(&location.path))
        .or_else(|| ToolKind::from_path(&row.entry.path))
        .or(match row.entry.kind.as_str() {
            "PopAnim" => Some(ToolKind::Pam),
            "SoundBank" => Some(ToolKind::Bnk),
            _ => None,
        })
}

pub fn output_path(row: &MappedResource) -> String {
    let physical = row
        .locations
        .first()
        .map(|location| location.path.as_str())
        .unwrap_or("");
    let path = if row.entry.path.is_empty() {
        physical
    } else {
        &row.entry.path
    };
    let mut path = safe_path(path, &row.entry.id);
    if row.state == MappingState::AtlasChild {
        if !path.to_ascii_lowercase().ends_with(".png") {
            path.push_str(".png");
        }
    } else if let Some(extension) = physical
        .rsplit('/')
        .next()
        .unwrap_or(physical)
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        && !path
            .to_ascii_lowercase()
            .ends_with(&format!(".{}", extension.to_ascii_lowercase()))
    {
        path.push('.');
        path.push_str(&extension.to_ascii_lowercase());
    }
    path
}

pub fn safe_path(path: &str, fallback: &str) -> String {
    let parts = path
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .map(|part| {
            part.chars()
                .map(|ch| {
                    if ch.is_control() || ":*?\"<>|".contains(ch) {
                        '_'
                    } else {
                        ch
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        if fallback.is_empty() {
            "resource.bin".into()
        } else {
            safe_path(fallback, "")
        }
    } else {
        parts.join("/")
    }
}

/// A single packet and atlas are cached, keeping memory bounded for large RSBs.
pub struct ResourceReader {
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed: RemovedPackets,
    packet: Option<Arc<PacketDocument>>,
    atlas: Option<(ResourceLocation, RgbaImage)>,
}

impl ResourceReader {
    pub fn new(archive: Arc<ArchiveDocument>, edits: PacketEdits, removed: RemovedPackets) -> Self {
        Self {
            archive,
            edits,
            removed,
            packet: None,
            atlas: None,
        }
    }

    async fn resolve(
        &mut self,
        row: &MappedResource,
    ) -> Result<(Arc<PacketDocument>, usize), String> {
        if !can_extract(row) {
            return Err(format!("{}：{}", row.state.label(), row.explanation));
        }
        let location = &row.locations[0];
        if self.removed.contains(&location.packet_index) {
            return Err("所属 RSG 已移除，请重新扫描".into());
        }
        let packet = if let Some(edit) = self.edits.get(&location.packet_index) {
            edit.document.clone()
        } else if let Some(packet) = self
            .packet
            .as_ref()
            .filter(|packet| packet.record.index == location.packet_index)
        {
            packet.clone()
        } else {
            let packet = Arc::new(
                crate::processing::load_manifest_packet(
                    self.archive.clone(),
                    location.packet_index,
                )
                .await?,
            );
            self.packet = Some(packet.clone());
            packet
        };
        let matches = packet
            .files
            .iter()
            .enumerate()
            .filter(|(_, file)| normalize_path(&file.path) == normalize_path(&location.path))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err("包内路径缺失或有重名，请重新扫描".into());
        }
        Ok((packet, matches[0]))
    }

    pub async fn extract(&mut self, row: &MappedResource) -> Result<ToolFile, String> {
        let bytes = if row.state == MappingState::AtlasChild {
            self.png(row).await?
        } else {
            let (packet, index) = self.resolve(row).await?;
            packet.files[index].data.clone()
        };
        Ok(ToolFile::new(output_path(row), bytes))
    }

    pub async fn png(&mut self, row: &MappedResource) -> Result<Vec<u8>, String> {
        if !can_preview(row) {
            return Err("此资源不是可预览的图片".into());
        }
        let location = &row.locations[0];
        if !self.atlas.as_ref().is_some_and(|(key, _)| key == location) {
            let (packet, index) = self.resolve(row).await?;
            let png = if location.path.to_ascii_uppercase().ends_with(".PTX") {
                let prepared = crate::preview::prepare_png_export(&self.archive, packet, index)?;
                crate::processing::decode_png(prepared.packet, prepared.file_index, prepared.spec)
                    .await?
                    .png
            } else {
                packet.files[index].data.clone()
            };
            let decoded = image::load_from_memory(&png)
                .map_err(|error| error.to_string())?
                .into_rgba8();
            self.atlas = Some((location.clone(), decoded));
        }
        let atlas = &self.atlas.as_ref().expect("atlas decoded").1;
        let cropped;
        let pixels = if row.state == MappingState::AtlasChild {
            let region = row.entry.region.as_ref().ok_or("图集子图缺少裁剪区域")?;
            cropped = crop(atlas, region)?;
            &cropped
        } else {
            atlas
        };
        let mut png = Cursor::new(Vec::new());
        pixels
            .write_to(&mut png, ImageFormat::Png)
            .map_err(|error| error.to_string())?;
        Ok(png.into_inner())
    }

    pub async fn pam_files(
        &mut self,
        row: &MappedResource,
        catalog: &ResourceCatalog,
        mut cancelled: impl FnMut() -> bool,
    ) -> Result<(Vec<ToolFile>, Vec<String>), String> {
        let mut file = self.extract(row).await?;
        let extension = file
            .name
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let format = match extension.as_str() {
            "json" => Some(pam_editor_formats::TextFormat::Json),
            "yaml" | "yml" => Some(pam_editor_formats::TextFormat::Yaml),
            "toml" => Some(pam_editor_formats::TextFormat::Toml),
            _ => None,
        };
        let pam = if let Some(format) = format {
            pam_editor_formats::decode_text(
                std::str::from_utf8(&file.bytes).map_err(|error| error.to_string())?,
                format,
            )
            .map_err(|error| error.to_string())?
        } else {
            if extension != "pam" {
                file.name.push_str(".pam");
            }
            pam_editor_core::decode_pam_bytes(&file.bytes).map_err(|error| error.to_string())?
        };
        let mut files = vec![file];
        let mut warnings = Vec::new();
        let mut names = BTreeSet::new();
        let mut total = files[0].bytes.len();
        let mut companions = Vec::new();
        for image in &pam.image {
            let name = pam_editor_core::parse_image_file_name(&image.name);
            if name.is_empty() || !names.insert(normalize_path(&name)) {
                continue;
            }
            let candidate = match find_pam_image(&row.entry, &name, image.size, catalog) {
                Ok(index) => Ok(index),
                Err(first) => match image.name.split_once('|') {
                    Some((_, alternate)) => find_pam_image(
                        &row.entry,
                        &pam_editor_core::parse_image_file_name(alternate),
                        image.size,
                        catalog,
                    ),
                    None => Err(first),
                },
            };
            match candidate {
                Ok(index) => companions.push((name, index)),
                Err(error) => warnings.push(format!("{name}：{error}")),
            }
        }
        companions.sort_by(|a, b| {
            catalog.rows[a.1]
                .locations
                .cmp(&catalog.rows[b.1].locations)
                .then(a.0.cmp(&b.0))
        });
        for (name, index) in companions {
            if cancelled() {
                return Err("已取消加载 PAM 图片".into());
            }
            match self.png(&catalog.rows[index]).await {
                Ok(bytes) if total + bytes.len() <= MAX_EXPORT_BYTES => {
                    total += bytes.len();
                    files.push(ToolFile::new(format!("{name}.png"), bytes));
                }
                Ok(_) => warnings.push(format!("{name}：图片总量超过 512 MiB，已跳过")),
                Err(error) => warnings.push(format!("{name}：{error}")),
            }
            #[cfg(target_arch = "wasm32")]
            gloo_timers::future::TimeoutFuture::new(0).await;
        }
        Ok((files, warnings))
    }
}

pub fn crop(atlas: &RgbaImage, region: &AtlasRegion) -> Result<RgbaImage, String> {
    if region.width == 0
        || region.height == 0
        || region
            .x
            .checked_add(region.width)
            .is_none_or(|right| right > atlas.width())
        || region
            .y
            .checked_add(region.height)
            .is_none_or(|bottom| bottom > atlas.height())
    {
        return Err(format!(
            "裁剪区域 ({}, {}, {} × {}) 超出图集 {} × {}",
            region.x,
            region.y,
            region.width,
            region.height,
            atlas.width(),
            atlas.height()
        ));
    }
    Ok(
        image::imageops::crop_imm(atlas, region.x, region.y, region.width, region.height)
            .to_image(),
    )
}

fn image_key(value: &str) -> String {
    normalize_path(value).trim_end_matches(".PNG").to_string()
}

pub fn find_pam_image(
    pam: &ResourceEntry,
    name: &str,
    size: Option<[i32; 2]>,
    catalog: &ResourceCatalog,
) -> Result<usize, String> {
    let name = image_key(name);
    let mut candidates = catalog
        .rows
        .iter()
        .enumerate()
        .filter(|(_, row)| can_preview(row) && !row.entry.atlas)
        .filter_map(|(index, row)| {
            let entry = &row.entry;
            let id_match = image_key(&entry.id) == name;
            if !id_match && image_key(&entry.path) != name {
                return None;
            }
            // Explicit locale/resolution must never silently cross to a different variant.
            if !pam.language.is_empty()
                && !entry.language.is_empty()
                && !pam.language.eq_ignore_ascii_case(&entry.language)
            {
                return None;
            }
            if !pam.resolution.is_empty()
                && !entry.resolution.is_empty()
                && pam.resolution != entry.resolution
            {
                return None;
            }
            let dimensions = entry
                .region
                .as_ref()
                .is_some_and(|region| size == Some([region.width as i32, region.height as i32]));
            let rank = (
                id_match,
                entry.subgroup.eq_ignore_ascii_case(&pam.subgroup),
                !pam.group.is_empty() && entry.group.eq_ignore_ascii_case(&pam.group),
                entry.language.eq_ignore_ascii_case(&pam.language),
                dimensions,
                entry.resolution.parse::<u32>().unwrap_or_default(),
            );
            Some((rank, index))
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable_by_key(|candidate| std::cmp::Reverse(candidate.0));
    let Some(&(rank, index)) = candidates.first() else {
        return Err("未找到对应图片".into());
    };
    if candidates
        .get(1)
        .is_some_and(|candidate| candidate.0 == rank)
    {
        return Err("存在多个同等优先级的图片，未自动选择".into());
    }
    Ok(index)
}

/// ZIP is built incrementally, without retaining a second copy of every output.
pub struct ResourceZip {
    writer: zip::ZipWriter<Cursor<Vec<u8>>>,
    names: BTreeSet<String>,
    pub count: usize,
    pub total_bytes: usize,
}

impl ResourceZip {
    pub fn new() -> Self {
        Self {
            writer: zip::ZipWriter::new(Cursor::new(Vec::new())),
            names: BTreeSet::new(),
            count: 0,
            total_bytes: 0,
        }
    }
    pub fn add(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        if self.total_bytes.saturating_add(bytes.len()) > MAX_EXPORT_BYTES {
            return Err("本次导出超过 512 MiB，请分批选择".into());
        }
        let path = safe_path(path, "resource.bin");
        let mut unique = path.clone();
        let mut suffix = 2;
        while !self.names.insert(unique.to_ascii_lowercase()) {
            let (stem, ext) = path
                .rsplit_once('.')
                .filter(|(_, ext)| !ext.contains('/'))
                .unwrap_or((&path, ""));
            unique = format!(
                "{stem} ({suffix}){}",
                if ext.is_empty() {
                    String::new()
                } else {
                    format!(".{ext}")
                }
            );
            suffix += 1;
        }
        // Most archive resources and exported PNGs are already compressed.
        self.writer
            .start_file(
                unique,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .map_err(|error| error.to_string())?;
        self.writer
            .write_all(bytes)
            .map_err(|error| error.to_string())?;
        self.count += 1;
        self.total_bytes += bytes.len();
        Ok(())
    }
    pub fn finish(self) -> Result<Vec<u8>, String> {
        self.writer
            .finish()
            .map(|cursor| cursor.into_inner())
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image_row(
        id: &str,
        group: &str,
        resolution: &str,
        language: &str,
        size: u32,
    ) -> MappedResource {
        MappedResource {
            entry: ResourceEntry {
                id: id.into(),
                path: format!("images/{id}"),
                kind: "Image".into(),
                group: group.into(),
                resolution: resolution.into(),
                language: language.into(),
                parent: "ATLAS".into(),
                region: Some(AtlasRegion {
                    x: 0,
                    y: 0,
                    width: size,
                    height: size,
                }),
                ..Default::default()
            },
            state: MappingState::AtlasChild,
            locations: vec![ResourceLocation {
                packet_index: 0,
                packet_name: "packet".into(),
                path: "atlas.ptx".into(),
            }],
            explanation: String::new(),
            search: String::new(),
        }
    }
    #[test]
    fn pam_matching_prefers_group_size_and_explicit_variants_without_guessing_ties() {
        let mut catalog = ResourceCatalog {
            rows: vec![
                image_row("LEAF", "Other", "768", "", 20),
                image_row("LEAF", "Plant", "768", "", 20),
                image_row("LEAF", "Plant", "1536", "", 40),
            ],
            ..Default::default()
        };
        let mut pam = ResourceEntry {
            group: "Plant".into(),
            ..Default::default()
        };
        assert_eq!(
            find_pam_image(&pam, "leaf", Some([20, 20]), &catalog).unwrap(),
            1
        );
        assert_eq!(find_pam_image(&pam, "leaf", None, &catalog).unwrap(), 2);
        pam.resolution = "768".into();
        assert_eq!(
            find_pam_image(&pam, "images/leaf.png", None, &catalog).unwrap(),
            1
        );
        catalog.rows.push(catalog.rows[1].clone());
        assert!(
            find_pam_image(&pam, "leaf", None, &catalog)
                .unwrap_err()
                .contains("多个")
        );
        assert!(find_pam_image(&pam, "unrelated/leaf", None, &catalog).is_err());
        catalog.rows = vec![image_row("LEAF", "Plant", "768", "frFR", 20)];
        pam.language = "enUS".into();
        assert!(find_pam_image(&pam, "leaf", None, &catalog).is_err());
    }
    #[test]
    fn output_uses_logical_case_and_real_file_extensions() {
        let mut row = image_row("leaf", "Plant", "768", "", 20);
        assert_eq!(output_path(&row), "images/leaf.png");
        row.state = MappingState::File;
        row.entry.path = "anim/plant".into();
        row.locations[0].path = "ANIM/PLANT.PAM".into();
        assert_eq!(output_path(&row), "anim/plant.pam");
        row.entry.path.push_str(".pam");
        assert_eq!(output_path(&row), "anim/plant.pam");
    }
    #[test]
    fn crop_keeps_pixels_and_alpha_and_rejects_invalid_regions() {
        let atlas = RgbaImage::from_fn(4, 3, |x, y| image::Rgba([x as u8, y as u8, 9, 80]));
        let region = AtlasRegion {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        };
        let child = crop(&atlas, &region).unwrap();
        assert_eq!(child.dimensions(), (2, 2));
        assert_eq!(child.get_pixel(1, 1).0, [2, 2, 9, 80]);
        assert!(
            crop(
                &atlas,
                &AtlasRegion {
                    x: u32::MAX,
                    ..region
                }
            )
            .is_err()
        );
        assert!(crop(&atlas, &AtlasRegion { width: 0, ..region }).is_err());
    }
    #[test]
    fn zip_paths_are_safe_and_case_insensitive_collisions_survive() {
        let mut zip = ResourceZip::new();
        zip.add("../images/a.png", b"one").unwrap();
        zip.add("images/A.png", b"two").unwrap();
        let mut read = zip::ZipArchive::new(Cursor::new(zip.finish().unwrap())).unwrap();
        assert_eq!(read.len(), 2);
        assert_eq!(read.by_index(0).unwrap().name(), "images/a.png");
        assert_eq!(read.by_index(1).unwrap().name(), "images/A (2).png");
        assert_eq!(safe_path("C:\\..\\x\0.png", ""), "C_/x_.png");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn exports_real_atlas_children_and_loads_pam_companions_when_available() -> Result<(), String> {
        let path = std::env::var_os("RSB_RESOURCE_REAL_SAMPLE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../../pvz2/com.popcap.ios.PvZ2.app/main.rsb")
            });
        if !path.exists() {
            eprintln!("skipping resource action real sample test");
            return Ok(());
        }
        pollster::block_on(async {
            let archive = Arc::new(crate::loader::open_native(path)?);
            let data = crate::resources::load_manifests(
                archive.clone(),
                PacketEdits::new(),
                RemovedPackets::new(),
            )
            .await;
            let physical = crate::resources::physical_files(
                &archive,
                &PacketEdits::new(),
                &RemovedPackets::new(),
            );
            let catalog = crate::resources::build_catalog(&data, &physical);
            let mut reader =
                ResourceReader::new(archive.clone(), PacketEdits::new(), RemovedPackets::new());
            let child = catalog
                .rows
                .iter()
                .find(|row| row.state == MappingState::AtlasChild)
                .ok_or("sample has no child")?;
            let child_file = reader.extract(child).await?;
            let child_png = image::load_from_memory(&child_file.bytes)
                .map_err(|error| error.to_string())?
                .into_rgba8();
            let region = child.entry.region.as_ref().unwrap();
            assert_eq!(child_png.dimensions(), (region.width, region.height));
            let atlas = catalog
                .rows
                .iter()
                .find(|row| row.entry.atlas && row.locations == child.locations)
                .ok_or("sample has no parent atlas")?;
            let atlas_png = image::load_from_memory(&reader.png(atlas).await?)
                .map_err(|error| error.to_string())?
                .into_rgba8();
            assert_eq!(child_png, crop(&atlas_png, region)?);
            eprintln!(
                "real child {}: {} × {}",
                child_file.name, region.width, region.height
            );
            let pams = catalog
                .rows
                .iter()
                .filter(|row| tool_kind(row) == Some(ToolKind::Pam))
                .filter(|row| {
                    row.entry.id.to_ascii_uppercase().contains("PEASHOOTER")
                        || row.entry.id.to_ascii_uppercase().contains("SUNFLOWER")
                })
                .take(4)
                .collect::<Vec<_>>();
            assert!(!pams.is_empty(), "sample should include plant PAMs");
            for row in pams {
                let (files, warnings) = reader.pam_files(row, &catalog, || false).await?;
                let pam = pam_editor_core::decode_pam_bytes(&files[0].bytes)
                    .map_err(|error| error.to_string())?;
                eprintln!(
                    "PAM {}: {} companions / {} images; warnings={:?}",
                    row.entry.id,
                    files.len() - 1,
                    pam.image.len(),
                    warnings
                );
                assert!(
                    warnings.is_empty(),
                    "PAM image resolution must be complete: {warnings:?}"
                );
                assert!(files.len() > 1);
                assert!(files.iter().skip(1).all(|file| file.name.ends_with(".png")));
            }
            Ok(())
        })
    }
}
