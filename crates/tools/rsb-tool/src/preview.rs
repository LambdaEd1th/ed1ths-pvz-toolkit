use crate::domain::{ArchiveChannelOrderMode, ArchiveDocument, PacketDocument};
use crate::{platform, processing};
use rsb_archive::{
    ChannelOrder, PtxDescriptor, PtxFormat, PtxFormatCode, PtxRsbMetadata, RsbPtxInfo,
};
use rsb_preview_worker::{FULL_RESOLUTION, PreviewSpec};
use std::collections::VecDeque;
use std::sync::Arc;

pub const THUMBNAIL_MAX_DIMENSION: u32 = 512;
pub const DETAIL_MAX_DIMENSION: u32 = 2048;
const CACHE_MEMORY_LIMIT: usize = 32 * 1024 * 1024;
const CACHE_ENTRY_LIMIT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveChannelOrderInference {
    Apple,
    Rgba,
    Conflicting,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveChannelOrderDetection {
    pub inference: ArchiveChannelOrderInference,
    pub apple_evidence: usize,
    pub rgba_evidence: usize,
    pub disambiguated_palettes: usize,
}

impl ArchiveChannelOrderDetection {
    const fn automatic_order(self) -> ChannelOrder {
        match self.inference {
            ArchiveChannelOrderInference::Apple => ChannelOrder::Bgra,
            ArchiveChannelOrderInference::Rgba
            | ArchiveChannelOrderInference::Conflicting
            | ArchiveChannelOrderInference::Unknown => ChannelOrder::Rgba,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewQuality {
    Thumbnail,
    Detail,
}

impl PreviewQuality {
    pub const fn max_dimension(self) -> u32 {
        match self {
            Self::Thumbnail => THUMBNAIL_MAX_DIMENSION,
            Self::Detail => DETAIL_MAX_DIMENSION,
        }
    }
}

#[derive(Clone)]
pub struct PreviewAsset(Arc<PreviewAssetInner>);

struct PreviewAssetInner {
    url: Arc<str>,
}

impl PreviewAsset {
    pub(crate) fn new(url: String) -> Self {
        Self(Arc::new(PreviewAssetInner {
            url: Arc::from(url),
        }))
    }

    pub fn url(&self) -> Arc<str> {
        Arc::clone(&self.0.url)
    }
}

impl PartialEq for PreviewAsset {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for PreviewAsset {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreviewAsset")
            .field("url", &self.0.url.split(':').next().unwrap_or_default())
            .finish()
    }
}

impl Drop for PreviewAssetInner {
    fn drop(&mut self) {
        platform::release_preview_url(&self.url);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PtxPreview {
    pub file_index: usize,
    pub name: String,
    pub asset: PreviewAsset,
    pub width: u32,
    pub height: u32,
    pub rendered_width: u32,
    pub rendered_height: u32,
    pub format: String,
    pub global_index: usize,
    pub payload_size: usize,
    encoded_size: usize,
    resident_size: usize,
    pub apple_channel_order: bool,
    pub quality: PreviewQuality,
}

impl PtxPreview {
    fn cache_cost(&self) -> usize {
        self.encoded_size.saturating_add(self.resident_size)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PtxPreviewState {
    Loading {
        file_index: usize,
        quality: PreviewQuality,
    },
    Ready(PtxPreview),
    Error {
        file_index: usize,
        quality: PreviewQuality,
        message: String,
    },
}

impl PtxPreviewState {
    pub const fn file_index(&self) -> usize {
        match self {
            Self::Loading { file_index, .. }
            | Self::Error { file_index, .. }
            | Self::Ready(PtxPreview { file_index, .. }) => *file_index,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewCacheKey {
    archive_identity: usize,
    packet_identity: usize,
    global_index: usize,
    width: u32,
    height: u32,
    format: i32,
    alpha_size: Option<i32>,
    alpha_format: Option<i32>,
    pitch: i32,
    max_dimension: u32,
    apple_channel_order: bool,
}

#[derive(Default)]
pub struct PreviewCache {
    entries: VecDeque<(PreviewCacheKey, PtxPreview)>,
    memory_bytes: usize,
}

impl PreviewCache {
    pub fn get(&mut self, key: PreviewCacheKey) -> Option<PtxPreview> {
        let index = self
            .entries
            .iter()
            .position(|(candidate, _)| *candidate == key)?;
        let entry = self.entries.remove(index)?;
        let preview = entry.1.clone();
        self.entries.push_front(entry);
        Some(preview)
    }

    pub fn insert(&mut self, key: PreviewCacheKey, preview: PtxPreview) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|(candidate, _)| *candidate == key)
            && let Some((_, previous)) = self.entries.remove(index)
        {
            self.memory_bytes = self.memory_bytes.saturating_sub(previous.cache_cost());
        }
        self.memory_bytes = self.memory_bytes.saturating_add(preview.cache_cost());
        self.entries.push_front((key, preview));
        while (self.memory_bytes > CACHE_MEMORY_LIMIT || self.entries.len() > CACHE_ENTRY_LIMIT)
            && self.entries.len() > 1
        {
            if let Some((_, removed)) = self.entries.pop_back() {
                self.memory_bytes = self.memory_bytes.saturating_sub(removed.cache_cost());
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.memory_bytes = 0;
    }
}

pub struct PreparedPreview {
    pub cache_key: PreviewCacheKey,
    pub packet: Arc<PacketDocument>,
    pub file_index: usize,
    pub quality: PreviewQuality,
    name: String,
    width: u32,
    height: u32,
    format: String,
    global_index: usize,
    payload_size: usize,
    apple_channel_order: bool,
    pub(crate) spec: PreviewSpec,
}

pub struct PreparedPngExport {
    pub packet: Arc<PacketDocument>,
    pub file_index: usize,
    pub output_name: String,
    pub spec: PreviewSpec,
}

pub fn prepare_preview(
    archive: &Arc<ArchiveDocument>,
    packet: Arc<PacketDocument>,
    file_index: usize,
    quality: PreviewQuality,
) -> Result<PreparedPreview, String> {
    let file = packet
        .files
        .get(file_index)
        .ok_or_else(|| "PTX 文件索引已失效".to_string())?;
    if !file.path.to_ascii_lowercase().ends_with(".ptx") {
        return Err("所选文件不是 PTX 纹理".to_string());
    }
    let metadata = archive
        .texture_metadata(&packet.record, file)
        .ok_or_else(|| "归档中缺少该 PTX 的全局纹理元数据".to_string())?;
    let apple_channel_order = archive_uses_apple_channel_order(archive);
    let name = file
        .path
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(&file.path)
        .to_string();
    let payload_size = file.data.len();
    let rsb_metadata = PtxRsbMetadata {
        format_code: PtxFormatCode::new(metadata.info.format),
        alpha_size: metadata
            .info
            .alpha_size
            .and_then(|value| u32::try_from(value).ok()),
        alpha_format: metadata.info.alpha_format,
        row_pitch: u32::try_from(metadata.info.pitch).ok(),
        channel_order: if apple_channel_order {
            ChannelOrder::Bgra
        } else {
            ChannelOrder::Rgba
        },
    };
    let format =
        PtxDescriptor::from_rsb_payload(metadata.width, metadata.height, rsb_metadata, &file.data)
            .map(|descriptor| format_label(descriptor.format))
            .unwrap_or_else(|_| format!("Format {}", metadata.info.format));
    let max_dimension = quality.max_dimension();
    Ok(PreparedPreview {
        cache_key: PreviewCacheKey {
            archive_identity: archive.identity(),
            packet_identity: Arc::as_ptr(&packet) as usize,
            global_index: metadata.global_index,
            width: metadata.width,
            height: metadata.height,
            format: metadata.info.format,
            alpha_size: metadata.info.alpha_size,
            alpha_format: metadata.info.alpha_format,
            pitch: metadata.info.pitch,
            max_dimension,
            apple_channel_order,
        },
        packet,
        file_index,
        quality,
        name,
        width: metadata.width,
        height: metadata.height,
        format,
        global_index: metadata.global_index,
        payload_size,
        apple_channel_order,
        spec: PreviewSpec {
            width: metadata.width,
            height: metadata.height,
            format: metadata.info.format,
            alpha_size: metadata.info.alpha_size,
            alpha_format: metadata.info.alpha_format,
            pitch: metadata.info.pitch,
            apple_channel_order,
            max_dimension,
        },
    })
}

pub fn prepare_png_export(
    archive: &Arc<ArchiveDocument>,
    packet: Arc<PacketDocument>,
    file_index: usize,
) -> Result<PreparedPngExport, String> {
    let mut prepared = prepare_preview(archive, packet, file_index, PreviewQuality::Thumbnail)?;
    prepared.spec.max_dimension = FULL_RESOLUTION;
    let output_name = prepared
        .name
        .rsplit_once('.')
        .map(|(stem, _)| format!("{stem}.png"))
        .unwrap_or_else(|| format!("{}.png", prepared.name));
    Ok(PreparedPngExport {
        packet: prepared.packet,
        file_index: prepared.file_index,
        output_name,
        spec: prepared.spec,
    })
}

pub async fn decode_prepared(
    prepared: PreparedPreview,
    generation: u64,
) -> Result<(PreviewCacheKey, PtxPreview), String> {
    let processed = processing::perform(
        prepared.packet,
        prepared.file_index,
        prepared.spec,
        generation,
    )
    .await?;
    if !processing::is_current(generation) {
        platform::release_preview_url(&processed.url);
        return Err("PTX 预览任务已被新的选择替代".to_string());
    }
    let encoded_size = processed.response.png.len();
    let resident_size = (processed.response.width as usize)
        .saturating_mul(processed.response.height as usize)
        .saturating_mul(4);
    let preview = PtxPreview {
        file_index: prepared.file_index,
        name: prepared.name,
        asset: PreviewAsset::new(processed.url),
        width: prepared.width,
        height: prepared.height,
        rendered_width: processed.response.width,
        rendered_height: processed.response.height,
        format: prepared.format,
        global_index: prepared.global_index,
        payload_size: prepared.payload_size,
        encoded_size,
        resident_size,
        apple_channel_order: prepared.apple_channel_order,
        quality: prepared.quality,
    };
    Ok((prepared.cache_key, preview))
}

pub(crate) fn detect_archive_channel_order(
    ptx_infos: &[RsbPtxInfo],
) -> ArchiveChannelOrderDetection {
    let mut apple_evidence = 0;
    let mut rgba_evidence = 0;
    let mut disambiguated_palettes = 0;

    for info in ptx_infos {
        match info.format {
            // PVRTC+A8 is an unambiguous Apple-family marker.
            148 => apple_evidence += 1,
            // Code 30 is PVRTC unless the extended alpha payload identifies
            // PvZ2 China's ETC1 palette-alpha representation.
            30 if info.alpha_size.is_some_and(|size| size > 0) => {
                rgba_evidence += 1;
                disambiguated_palettes += 1;
            }
            30 => apple_evidence += 1,
            // Code 147 covers the ETC1 family, including its A8, compressed
            // alpha, and palette-alpha variants.
            147 => rgba_evidence += 1,
            _ => {}
        }
    }

    let inference = match (apple_evidence > 0, rgba_evidence > 0) {
        (true, false) => ArchiveChannelOrderInference::Apple,
        (false, true) => ArchiveChannelOrderInference::Rgba,
        (true, true) => ArchiveChannelOrderInference::Conflicting,
        (false, false) => ArchiveChannelOrderInference::Unknown,
    };
    ArchiveChannelOrderDetection {
        inference,
        apple_evidence,
        rgba_evidence,
        disambiguated_palettes,
    }
}

pub(crate) fn archive_channel_order_detection(
    archive: &ArchiveDocument,
) -> ArchiveChannelOrderDetection {
    detect_archive_channel_order(&archive.ptx_infos)
}

pub(crate) const fn effective_channel_order(
    mode: ArchiveChannelOrderMode,
    detection: ArchiveChannelOrderDetection,
) -> ChannelOrder {
    match mode {
        ArchiveChannelOrderMode::Auto => detection.automatic_order(),
        ArchiveChannelOrderMode::Rgba => ChannelOrder::Rgba,
        ArchiveChannelOrderMode::Apple => ChannelOrder::Bgra,
    }
}

pub(crate) fn archive_channel_order(archive: &ArchiveDocument) -> ChannelOrder {
    effective_channel_order(
        archive.channel_order_mode,
        archive_channel_order_detection(archive),
    )
}

pub(crate) fn archive_uses_apple_channel_order(archive: &ArchiveDocument) -> bool {
    archive_channel_order(archive) == ChannelOrder::Bgra
}

fn format_label(format: PtxFormat) -> String {
    match format {
        PtxFormat::Rgba8888 => "RGBA 8888".into(),
        PtxFormat::Rgba4444 => "RGBA 4444".into(),
        PtxFormat::Rgb565 => "RGB 565".into(),
        PtxFormat::Rgba5551 => "RGBA 5551".into(),
        PtxFormat::Rgba4444Block => "RGBA 4444 · tiled".into(),
        PtxFormat::Rgb565Block => "RGB 565 · tiled".into(),
        PtxFormat::Rgba5551Block => "RGBA 5551 · tiled".into(),
        PtxFormat::Pvrtc4BppRgba => "PVRTC 4bpp".into(),
        PtxFormat::Pvrtc4BppRgbaA8 => "PVRTC 4bpp + A8".into(),
        PtxFormat::Etc1 => "ETC1".into(),
        PtxFormat::Etc1A8 => "ETC1 + A8".into(),
        PtxFormat::Etc1CompressedAlpha => "ETC1 + compressed alpha".into(),
        PtxFormat::Etc1Palette => "ETC1 + alpha palette".into(),
        PtxFormat::Astc {
            block_width,
            block_height,
        } => format!("ASTC {block_width}×{block_height}"),
        PtxFormat::A8 => "A8".into(),
        PtxFormat::L8 => "L8".into(),
        PtxFormat::La88 => "LA 88".into(),
        PtxFormat::Al88 => "AL 88".into(),
        PtxFormat::La44 => "LA 44".into(),
        PtxFormat::Al44 => "AL 44".into(),
        PtxFormat::Rgb332 => "RGB 332".into(),
        PtxFormat::Rgb888 => "RGB 888".into(),
        PtxFormat::Argb8888 => "ARGB 8888".into(),
        PtxFormat::Argb4444 => "ARGB 4444".into(),
        PtxFormat::Argb1555 => "ARGB 1555".into(),
        PtxFormat::Unknown(code) => format!("Format {code}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArchiveChannelOrderInference, CACHE_ENTRY_LIMIT, CACHE_MEMORY_LIMIT, PreviewAsset,
        PreviewCache, PreviewCacheKey, PreviewQuality, PtxPreview, detect_archive_channel_order,
        effective_channel_order,
    };
    use crate::domain::ArchiveChannelOrderMode;
    use rsb_archive::{ChannelOrder, RsbPtxInfo};

    fn preview(index: usize, dimension: u32) -> PtxPreview {
        PtxPreview {
            file_index: index,
            name: format!("{index}.ptx"),
            asset: PreviewAsset::new(format!("data:image/png;base64,{index}")),
            width: dimension,
            height: dimension,
            rendered_width: dimension,
            rendered_height: dimension,
            format: "RGBA 8888".into(),
            global_index: index,
            payload_size: 1,
            encoded_size: 1,
            resident_size: (dimension as usize)
                .saturating_mul(dimension as usize)
                .saturating_mul(4),
            apple_channel_order: true,
            quality: PreviewQuality::Detail,
        }
    }

    #[test]
    fn cache_is_bounded_by_decoded_texture_memory() {
        let mut cache = PreviewCache::default();
        for index in 0..12 {
            cache.insert(
                PreviewCacheKey {
                    archive_identity: 1,
                    packet_identity: 1,
                    global_index: index,
                    width: 1536,
                    height: 1536,
                    format: 0,
                    alpha_size: None,
                    alpha_format: None,
                    pitch: 0,
                    max_dimension: 2048,
                    apple_channel_order: true,
                },
                preview(index, 1536),
            );
        }

        assert!(cache.entries.len() <= CACHE_ENTRY_LIMIT);
        assert!(cache.memory_bytes <= CACHE_MEMORY_LIMIT);
        assert!(cache.entries.len() < 12);
    }

    fn texture(format: i32, alpha_size: Option<i32>) -> RsbPtxInfo {
        RsbPtxInfo {
            format,
            alpha_size,
            ..Default::default()
        }
    }

    #[test]
    fn disambiguates_code_30_before_inferring_apple_order() {
        let pvrtc = detect_archive_channel_order(&[texture(30, None), texture(148, None)]);
        assert_eq!(pvrtc.inference, ArchiveChannelOrderInference::Apple);
        assert_eq!(pvrtc.apple_evidence, 2);

        let palette = detect_archive_channel_order(&[texture(30, Some(49))]);
        assert_eq!(palette.inference, ArchiveChannelOrderInference::Rgba);
        assert_eq!(palette.apple_evidence, 0);
        assert_eq!(palette.rgba_evidence, 1);
        assert_eq!(palette.disambiguated_palettes, 1);
    }

    #[test]
    fn conflicting_or_missing_platform_evidence_defaults_to_rgba() {
        let conflicting = detect_archive_channel_order(&[texture(148, None), texture(147, None)]);
        assert_eq!(
            conflicting.inference,
            ArchiveChannelOrderInference::Conflicting
        );
        assert_eq!(
            effective_channel_order(ArchiveChannelOrderMode::Auto, conflicting),
            ChannelOrder::Rgba
        );

        let unknown = detect_archive_channel_order(&[texture(0, None)]);
        assert_eq!(unknown.inference, ArchiveChannelOrderInference::Unknown);
        assert_eq!(
            effective_channel_order(ArchiveChannelOrderMode::Auto, unknown),
            ChannelOrder::Rgba
        );
    }

    #[test]
    fn manual_channel_order_overrides_automatic_detection() {
        let detected = detect_archive_channel_order(&[texture(148, None)]);
        assert_eq!(
            effective_channel_order(ArchiveChannelOrderMode::Rgba, detected),
            ChannelOrder::Rgba
        );
        assert_eq!(
            effective_channel_order(ArchiveChannelOrderMode::Apple, detected),
            ChannelOrder::Bgra
        );
    }
}
