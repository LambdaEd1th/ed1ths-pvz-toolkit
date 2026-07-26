use image::{DynamicImage, RgbaImage};

use crate::error::{Result as RsbResult, RsbError};
use crate::ptx::codec::astc::{AstcQuality, encode_astc as encode_astc_payload};
use crate::ptx::codec::etc1::{Etc1Plane, encode_palette, encode_plane};
use crate::ptx::codec::packed::{encode_linear, encode_tiled};
use crate::ptx::codec::pvrtc::encode_pvrtc_4bpp;
use crate::ptx::error::{PtxError, Result};
use crate::ptx::{ChannelOrder, PtxDescriptor, PtxFormat, RgbaSurface};

#[derive(Debug, Clone, Copy)]
pub struct PtxEncodeOptions {
    pub format: PtxFormat,
    pub row_pitch: Option<u32>,
    pub channel_order: ChannelOrder,
    pub astc_quality: AstcQuality,
}

impl PtxEncodeOptions {
    pub const fn new(format: PtxFormat) -> Self {
        Self {
            format,
            row_pitch: None,
            channel_order: ChannelOrder::Rgba,
            astc_quality: AstcQuality::MEDIUM,
        }
    }
}

pub struct PtxEncoder;

impl PtxEncoder {
    pub fn encode_surface(surface: RgbaSurface<'_>, options: PtxEncodeOptions) -> Result<Vec<u8>> {
        let descriptor = PtxDescriptor::new(surface.width(), surface.height(), options.format)?
            .with_row_pitch(options.row_pitch)
            .with_channel_order(options.channel_order);
        match options.format {
            PtxFormat::Rgba8888
            | PtxFormat::Rgba4444
            | PtxFormat::Rgb565
            | PtxFormat::Rgba5551
            | PtxFormat::A8
            | PtxFormat::L8
            | PtxFormat::La88
            | PtxFormat::Al88
            | PtxFormat::La44
            | PtxFormat::Al44
            | PtxFormat::Rgb332
            | PtxFormat::Rgb888
            | PtxFormat::Argb8888
            | PtxFormat::Argb4444
            | PtxFormat::Argb1555 => encode_linear(surface, descriptor),
            PtxFormat::Rgba4444Block | PtxFormat::Rgb565Block | PtxFormat::Rgba5551Block => {
                encode_tiled(surface, descriptor)
            }
            PtxFormat::Etc1 => encode_plane(surface, Etc1Plane::Color),
            PtxFormat::Etc1A8 => {
                let mut output = encode_plane(surface, Etc1Plane::Color)?;
                append_a8(&mut output, surface);
                Ok(output)
            }
            PtxFormat::Etc1CompressedAlpha => {
                let mut output = encode_plane(surface, Etc1Plane::Color)?;
                output.extend(encode_plane(surface, Etc1Plane::Alpha)?);
                Ok(output)
            }
            PtxFormat::Etc1Palette => {
                let mut output = encode_plane(surface, Etc1Plane::Color)?;
                output.extend(encode_palette(surface)?);
                Ok(output)
            }
            PtxFormat::Pvrtc4BppRgba | PtxFormat::Pvrtc4BppRgbaA8 => {
                let image = DynamicImage::ImageRgba8(surface.to_image());
                let mut output =
                    encode_pvrtc_4bpp(&image, options.format == PtxFormat::Pvrtc4BppRgba)
                        .map_err(PtxError::from)?;
                if options.format == PtxFormat::Pvrtc4BppRgbaA8 {
                    append_a8(&mut output, surface);
                }
                Ok(output)
            }
            PtxFormat::Astc {
                block_width,
                block_height,
            } => encode_astc_payload(
                &DynamicImage::ImageRgba8(surface.to_image()),
                block_width,
                block_height,
                options.astc_quality,
            )
            .map_err(PtxError::from),
            PtxFormat::Unknown(code) => Err(PtxError::UnknownFormatCode(code)),
        }
    }

    pub fn encode_image(image: &RgbaImage, options: PtxEncodeOptions) -> Result<Vec<u8>> {
        Self::encode_surface(RgbaSurface::from_image(image), options)
    }

    /// Encode a raw ASTC PTX payload with an explicit pure Rust search effort.
    pub fn encode_astc(
        image: &DynamicImage,
        block_width: u32,
        block_height: u32,
        quality: AstcQuality,
    ) -> RsbResult<Vec<u8>> {
        encode_astc_payload(image, block_width, block_height, quality)
    }

    /// Compatibility wrapper for the original three-argument encoder.
    pub fn encode(image: &DynamicImage, format: PtxFormat, is_powervr: bool) -> RsbResult<Vec<u8>> {
        let rgba = image.to_rgba8();
        let mut options = PtxEncodeOptions::new(format);
        options.channel_order = if is_powervr {
            ChannelOrder::Bgra
        } else {
            ChannelOrder::Rgba
        };
        Self::encode_image(&rgba, options).map_err(to_rsb_error)
    }
}

fn append_a8(output: &mut Vec<u8>, surface: RgbaSurface<'_>) {
    for y in 0..surface.height() {
        for x in 0..surface.width() {
            output.push(surface.pixel(x, y)[3]);
        }
    }
}

fn to_rsb_error(error: PtxError) -> RsbError {
    RsbError::DeserializationError(error.to_string())
}
