use image::{DynamicImage, RgbaImage};

use super::{apply_a8, apply_etc1, decode_palette, decode_plane};
use crate::error::{Result, RsbError};

pub fn decode_etc1(data: &[u8], width: u32, height: u32) -> Result<DynamicImage> {
    Ok(DynamicImage::ImageRgba8(
        decode_plane(data, width, height).map_err(to_rsb_error)?,
    ))
}

pub fn decode_etc1_a8(
    data_color: &[u8],
    data_alpha: &[u8],
    width: u32,
    height: u32,
    compressed_alpha: bool,
) -> Result<DynamicImage> {
    let mut image = decode_plane(data_color, width, height).map_err(to_rsb_error)?;
    if compressed_alpha {
        apply_etc1(&mut image, data_alpha).map_err(to_rsb_error)?;
    } else {
        apply_a8(&mut image, data_alpha).map_err(to_rsb_error)?;
    }
    Ok(DynamicImage::ImageRgba8(image))
}

pub fn decode_palette_alpha(data: &[u8], width: u32, height: u32) -> Result<DynamicImage> {
    let color_len =
        usize::try_from(u64::from(width.div_ceil(4)) * u64::from(height.div_ceil(4)) * 8)
            .map_err(|_| RsbError::DeserializationError("PTX dimensions overflow".into()))?;
    if data.len() < color_len {
        return Err(RsbError::DeserializationError(
            "insufficient ETC1 palette payload".into(),
        ));
    }
    let mut image = decode_plane(&data[..color_len], width, height).map_err(to_rsb_error)?;
    let alpha = decode_palette(
        &data[color_len..],
        usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| RsbError::DeserializationError("PTX dimensions overflow".into()))?,
    )
    .map_err(to_rsb_error)?;
    apply_alpha_values(&mut image, &alpha);
    Ok(DynamicImage::ImageRgba8(image))
}

pub fn decode_palette_alpha_values(
    data: &[u8],
    num_pixels: usize,
) -> std::result::Result<Vec<u8>, String> {
    decode_palette(data, num_pixels).map_err(|error| error.to_string())
}

fn apply_alpha_values(image: &mut RgbaImage, alpha: &[u8]) {
    for (pixel, &value) in image.pixels_mut().zip(alpha) {
        pixel[3] = value;
    }
}

fn to_rsb_error(error: crate::ptx::PtxError) -> RsbError {
    RsbError::DeserializationError(error.to_string())
}
