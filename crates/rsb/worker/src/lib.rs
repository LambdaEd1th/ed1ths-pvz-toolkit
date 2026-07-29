use image::{DynamicImage, ImageFormat, ImageReader, Limits, RgbaImage, imageops::FilterType};
use rsb_archive::{
    ChannelOrder, PtxDecoder, PtxDescriptor, PtxEncodeOptions, PtxEncoder, PtxFormat,
    PtxFormatCode, PtxPayloadLayout, PtxRsbMetadata, unpack_rsg,
};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use thiserror::Error;

const MAX_SOURCE_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_PREVIEW_EDGE: u32 = 4096;
pub const FULL_RESOLUTION: u32 = 0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewSpec {
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub alpha_size: Option<i32>,
    pub alpha_format: Option<i32>,
    pub pitch: i32,
    pub apple_channel_order: bool,
    pub max_dimension: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewRequest {
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub spec: PreviewSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviewResponse {
    #[serde(with = "serde_bytes")]
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub source_width: u32,
    pub source_height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacketRequest {
    #[serde(with = "serde_bytes")]
    pub raw: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacketFile {
    pub path: String,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub is_part1: bool,
    pub texture_id: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PacketResponse {
    pub files: Vec<PacketFile>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureEncoding {
    #[default]
    Metadata,
    Etc1,
    Etc1A8,
    Etc1CompressedAlpha,
    Etc1Palette,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextureEncodeRequest {
    #[serde(with = "serde_bytes")]
    pub source: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: i32,
    pub alpha_size: Option<i32>,
    pub alpha_format: Option<i32>,
    pub pitch: i32,
    pub apple_channel_order: bool,
    pub encoding: TextureEncoding,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextureEncodeResponse {
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub format: i32,
    pub alpha_size: Option<i32>,
    pub alpha_format: Option<i32>,
}

#[derive(Debug, Error)]
pub enum PreviewWorkerError {
    #[error("PTX 预览任务已取消")]
    Cancelled,
    #[error("PTX 尺寸无效")]
    InvalidDimensions,
    #[error("PTX 像素数量超过安全预览上限")]
    TooLarge,
    #[error("PTX 预览边长超过安全上限")]
    PreviewEdgeTooLarge,
    #[error("无法读取源图片：{0}")]
    Image(String),
    #[error("{0}")]
    Codec(String),
    #[error("无法生成预览 PNG：{0}")]
    Png(String),
}

pub type Result<T> = std::result::Result<T, PreviewWorkerError>;

pub fn perform(request: PreviewRequest) -> Result<PreviewResponse> {
    perform_borrowed(&request.data, &request.spec, || false)
}

pub fn unpack_packet(request: PacketRequest) -> std::result::Result<PacketResponse, String> {
    let files = unpack_rsg(&mut Cursor::new(request.raw))
        .map_err(|error| format!("RSG 解压失败：{error}"))?
        .into_iter()
        .map(|file| {
            let (texture_id, width, height) = file
                .part1_info
                .map(|info| (Some(info.id), Some(info.width), Some(info.height)))
                .unwrap_or((None, None, None));
            PacketFile {
                path: file.path,
                data: file.data,
                is_part1: file.is_part1,
                texture_id,
                width,
                height,
            }
        })
        .collect();
    Ok(PacketResponse { files })
}

pub fn encode_texture(request: TextureEncodeRequest) -> Result<TextureEncodeResponse> {
    validate_source_dimensions(request.width, request.height)?;
    let mut reader = ImageReader::new(Cursor::new(request.source))
        .with_guessed_format()
        .map_err(|error| PreviewWorkerError::Image(error.to_string()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    let source = reader
        .decode()
        .map_err(|error| PreviewWorkerError::Image(error.to_string()))?
        .to_rgba8();
    validate_source_dimensions(source.width(), source.height())?;
    let image = if source.dimensions() == (request.width, request.height) {
        source
    } else {
        image::imageops::resize(&source, request.width, request.height, FilterType::Lanczos3)
    };

    let channel_order = if request.apple_channel_order {
        ChannelOrder::Bgra
    } else {
        ChannelOrder::Rgba
    };
    let (format_code, format) = match request.encoding {
        TextureEncoding::Metadata => {
            let metadata = PtxRsbMetadata {
                format_code: PtxFormatCode::new(request.format),
                alpha_size: request
                    .alpha_size
                    .and_then(|value| u32::try_from(value).ok()),
                alpha_format: request.alpha_format,
                row_pitch: u32::try_from(request.pitch).ok().filter(|pitch| *pitch > 0),
                channel_order,
            };
            (
                request.format,
                PtxDescriptor::from_rsb(request.width, request.height, metadata)
                    .map_err(|error| PreviewWorkerError::Codec(error.to_string()))?
                    .format,
            )
        }
        TextureEncoding::Etc1 => (147, PtxFormat::Etc1),
        TextureEncoding::Etc1A8 => (147, PtxFormat::Etc1A8),
        TextureEncoding::Etc1CompressedAlpha => (147, PtxFormat::Etc1CompressedAlpha),
        TextureEncoding::Etc1Palette => (
            if matches!(request.format, 30 | 147) {
                request.format
            } else {
                147
            },
            PtxFormat::Etc1Palette,
        ),
    };
    let mut options = PtxEncodeOptions::new(format);
    options.row_pitch = u32::try_from(request.pitch).ok().filter(|pitch| *pitch > 0);
    options.channel_order = channel_order;
    let data = PtxEncoder::encode_image(&image, options)
        .map_err(|error| PreviewWorkerError::Codec(error.to_string()))?;

    let (alpha_size, alpha_format) = if format == PtxFormat::Etc1Palette {
        let descriptor = PtxDescriptor::new(request.width, request.height, format)
            .map_err(|error| PreviewWorkerError::Codec(error.to_string()))?;
        let layout = PtxPayloadLayout::for_encode(descriptor)
            .map_err(|error| PreviewWorkerError::Codec(error.to_string()))?;
        let alpha_size = layout
            .alpha
            .map(|plane| plane.range.len())
            .and_then(|size| i32::try_from(size).ok())
            .ok_or_else(|| PreviewWorkerError::Codec("ETC1 调色板 Alpha 大小溢出".into()))?;
        (Some(alpha_size), Some(request.alpha_format.unwrap_or(100)))
    } else if request.encoding == TextureEncoding::Metadata {
        (request.alpha_size, request.alpha_format)
    } else {
        (
            request.alpha_size.filter(|value| *value == 0),
            request.alpha_format,
        )
    };
    Ok(TextureEncodeResponse {
        data,
        format: format_code,
        alpha_size,
        alpha_format,
    })
}

fn validate_source_dimensions(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(PreviewWorkerError::InvalidDimensions);
    }
    if u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS {
        return Err(PreviewWorkerError::TooLarge);
    }
    Ok(())
}

pub fn perform_borrowed(
    data: &[u8],
    spec: &PreviewSpec,
    cancelled: impl Fn() -> bool,
) -> Result<PreviewResponse> {
    if spec.width == 0 || spec.height == 0 {
        return Err(PreviewWorkerError::InvalidDimensions);
    }
    if u64::from(spec.width) * u64::from(spec.height) > MAX_SOURCE_PIXELS {
        return Err(PreviewWorkerError::TooLarge);
    }
    if spec.max_dimension != FULL_RESOLUTION && spec.max_dimension > MAX_PREVIEW_EDGE {
        return Err(PreviewWorkerError::PreviewEdgeTooLarge);
    }
    check_cancelled(&cancelled)?;

    let decoded = PtxDecoder::decode_rgba8(
        data,
        spec.width,
        spec.height,
        spec.format,
        spec.alpha_size,
        spec.alpha_format,
        Some(spec.pitch),
        spec.apple_channel_order,
    )
    .map_err(|error| PreviewWorkerError::Codec(error.to_string()))?;
    check_cancelled(&cancelled)?;

    let (width, height) = if spec.max_dimension == FULL_RESOLUTION {
        (spec.width, spec.height)
    } else {
        preview_dimensions(spec.width, spec.height, spec.max_dimension)
    };
    let image = if width == spec.width && height == spec.height {
        decoded
    } else {
        image::imageops::resize(&decoded, width, height, FilterType::Triangle)
    };
    check_cancelled(&cancelled)?;

    let png = encode_png(image)?;
    check_cancelled(&cancelled)?;
    Ok(PreviewResponse {
        png,
        width,
        height,
        source_width: spec.width,
        source_height: spec.height,
    })
}

fn check_cancelled(cancelled: &impl Fn() -> bool) -> Result<()> {
    if cancelled() {
        Err(PreviewWorkerError::Cancelled)
    } else {
        Ok(())
    }
}

fn preview_dimensions(width: u32, height: u32, max_dimension: u32) -> (u32, u32) {
    let largest = width.max(height);
    if largest <= max_dimension {
        return (width, height);
    }
    if width >= height {
        (
            max_dimension,
            ((u64::from(height) * u64::from(max_dimension) + u64::from(width) / 2)
                / u64::from(width))
            .max(1) as u32,
        )
    } else {
        (
            ((u64::from(width) * u64::from(max_dimension) + u64::from(height) / 2)
                / u64::from(height))
            .max(1) as u32,
            max_dimension,
        )
    }
}

fn encode_png(image: RgbaImage) -> Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| PreviewWorkerError::Png(error.to_string()))?;
    Ok(output.into_inner())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn decode_preview(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<PreviewRequest>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response =
        perform(request).map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn unpack_packet_preview(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<PacketRequest>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response =
        unpack_packet(request).map_err(|error| wasm_bindgen::JsValue::from_str(&error))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn encode_texture_preview(
    request: wasm_bindgen::JsValue,
) -> std::result::Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let request = serde_wasm_bindgen::from_value::<TextureEncodeRequest>(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    let response = encode_texture(request)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))?;
    serde_wasm_bindgen::to_value(&response)
        .map_err(|error| wasm_bindgen::JsValue::from_str(&error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        PreviewRequest, PreviewSpec, TextureEncodeRequest, TextureEncoding, encode_png,
        encode_texture, perform, preview_dimensions,
    };
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use rsb_archive::{PtxDecoder, PtxDescriptor, PtxFormat};
    use std::io::Cursor;

    #[test]
    fn preview_dimensions_preserve_aspect_ratio_and_limits() {
        assert_eq!(preview_dimensions(2048, 3003, 512), (349, 512));
        assert_eq!(preview_dimensions(512, 512, 1024), (512, 512));
        assert_eq!(preview_dimensions(4096, 1024, 2048), (2048, 512));
    }

    #[test]
    fn large_rgba_texture_is_bounded_to_a_small_preview() {
        let width = 2048_u32;
        let height = 3003_u32;
        let mut data = vec![0_u8; width as usize * height as usize * 4];
        for (index, pixel) in data.chunks_exact_mut(4).enumerate() {
            pixel[0] = (index % 251) as u8;
            pixel[1] = ((index / width as usize) % 239) as u8;
            pixel[2] = 127;
            pixel[3] = if index % 5 == 0 { 0 } else { 255 };
        }
        let response = perform(PreviewRequest {
            data,
            spec: PreviewSpec {
                width,
                height,
                format: 0,
                alpha_size: None,
                alpha_format: None,
                pitch: (width * 4) as i32,
                apple_channel_order: false,
                max_dimension: 512,
            },
        })
        .expect("large RGBA preview should decode");
        assert_eq!((response.width, response.height), (349, 512));
        assert!(response.png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(response.png.len() < 2 * 1024 * 1024);
    }

    #[test]
    fn zero_max_dimension_exports_the_full_resolution() {
        let width = 7_u32;
        let height = 5_u32;
        let data = RgbaImage::from_fn(width, height, |x, y| {
            Rgba([x as u8 * 10, y as u8 * 20, 120, 255])
        })
        .into_raw();
        let response = perform(PreviewRequest {
            data,
            spec: PreviewSpec {
                width,
                height,
                format: 0,
                alpha_size: None,
                alpha_format: None,
                pitch: (width * 4) as i32,
                apple_channel_order: false,
                max_dimension: super::FULL_RESOLUTION,
            },
        })
        .expect("full-resolution RGBA export should decode");
        assert_eq!((response.width, response.height), (width, height));
        assert_eq!(
            (response.source_width, response.source_height),
            (width, height)
        );
        assert!(response.png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn encodes_a_png_source_as_a_resized_ptx_payload() {
        let source = RgbaImage::from_fn(2, 2, |x, y| Rgba([x as u8 * 80, y as u8 * 90, 140, 200]));
        let response = encode_texture(TextureEncodeRequest {
            source: encode_png(source).unwrap(),
            width: 4,
            height: 3,
            format: 0,
            alpha_size: None,
            alpha_format: None,
            pitch: 0,
            apple_channel_order: false,
            encoding: TextureEncoding::Metadata,
        })
        .unwrap();
        assert_eq!(response.format, 0);
        assert_eq!(response.data.len(), 4 * 3 * 4);
        let decoded = PtxDecoder::decode_with_descriptor(
            &response.data,
            PtxDescriptor::new(4, 3, PtxFormat::Rgba8888).unwrap(),
        )
        .unwrap();
        assert_eq!(decoded.dimensions(), (4, 3));
    }

    #[test]
    fn preserves_alpha_when_image_import_uses_etc1_a8() {
        let source = RgbaImage::from_fn(4, 4, |x, y| Rgba([20, 80, 160, (x * 40 + y * 10) as u8]));
        let expected_alpha = source.pixels().map(|pixel| pixel[3]).collect::<Vec<_>>();
        let response = encode_texture(TextureEncodeRequest {
            source: encode_png(source).unwrap(),
            width: 4,
            height: 4,
            format: 147,
            alpha_size: None,
            alpha_format: Some(100),
            pitch: 0,
            apple_channel_order: false,
            encoding: TextureEncoding::Etc1A8,
        })
        .unwrap();
        assert_eq!(response.format, 147);
        assert_eq!(response.alpha_size, None);
        let descriptor = PtxDescriptor::new(4, 4, PtxFormat::Etc1A8).unwrap();
        let decoded = PtxDecoder::decode_with_descriptor(&response.data, descriptor).unwrap();
        assert_eq!(
            decoded.pixels().map(|pixel| pixel[3]).collect::<Vec<_>>(),
            expected_alpha
        );
    }

    #[test]
    fn accepts_webp_as_a_texture_source() {
        let source = RgbaImage::from_pixel(3, 2, Rgba([12, 34, 56, 210]));
        let mut webp = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(source)
            .write_to(&mut webp, ImageFormat::WebP)
            .unwrap();
        let response = encode_texture(TextureEncodeRequest {
            source: webp.into_inner(),
            width: 3,
            height: 2,
            format: 0,
            alpha_size: None,
            alpha_format: None,
            pitch: 0,
            apple_channel_order: false,
            encoding: TextureEncoding::Metadata,
        })
        .unwrap();
        assert_eq!(response.data.len(), 3 * 2 * 4);
    }
}
