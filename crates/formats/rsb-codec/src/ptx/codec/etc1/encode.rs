use image::DynamicImage;

use super::{Etc1Plane, encode_palette, encode_plane};
use crate::error::{Result, RsbError};
use crate::ptx::RgbaSurface;

pub fn encode_alpha(data: &[u8], _width: u32, _height: u32) -> Vec<u8> {
    data.to_vec()
}

pub fn encode_palette_alpha(image: &DynamicImage) -> Result<Vec<u8>> {
    let rgba = image.to_rgba8();
    let surface = RgbaSurface::from_image(&rgba);
    let mut output = encode_plane(surface, Etc1Plane::Color).map_err(to_rsb_error)?;
    output.extend(encode_palette(surface).map_err(to_rsb_error)?);
    Ok(output)
}

fn to_rsb_error(error: crate::ptx::PtxError) -> RsbError {
    RsbError::DeserializationError(error.to_string())
}
