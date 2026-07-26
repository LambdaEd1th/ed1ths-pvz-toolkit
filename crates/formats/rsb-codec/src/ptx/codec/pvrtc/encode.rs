use super::layout::{BILINEAR_FACTORS, get_morton_number_rect};
use super::packet::PvrTcPacket;
use crate::error::{Result, RsbError};
use crate::ptx::color::{ColorRGBA, Rgba32};
use image::DynamicImage;

/// Encodes a square power-of-two RGBA image into PVRTC1 4bpp packets.
///
/// Packets are returned in PVRTC Morton order. Invalid dimensions return an
/// empty vector; callers that need a typed error should use
/// [`encode_pvrtc_4bpp`].
pub fn encode_rgba_4bpp(colors: &[Rgba32], width: i32) -> Vec<PvrTcPacket> {
    encode_4bpp_packets(colors, width, width, true).unwrap_or_default()
}

fn encode_4bpp_packets(
    colors: &[Rgba32],
    width: i32,
    height: i32,
    include_alpha: bool,
) -> Option<Vec<PvrTcPacket>> {
    if width < 4
        || height < 4
        || !(width as u32).is_power_of_two()
        || !(height as u32).is_power_of_two()
        || colors.len() != (width * height) as usize
    {
        return None;
    }

    let blocks_x = width / 4;
    let blocks_y = height / 4;
    let packet_count = (blocks_x * blocks_y) as usize;
    let block_mask_x = blocks_x - 1;
    let block_mask_y = blocks_y - 1;
    let mut packets = vec![PvrTcPacket::new(0); packet_count];

    // Pass one chooses conservative endpoint colors for every 4x4 packet.
    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            let first = colors[((block_y * 4) * width + block_x * 4) as usize];
            let mut minimum = first;
            let mut maximum = first;
            for y in 0..4 {
                for x in 0..4 {
                    let color = colors[((block_y * 4 + y) * width + block_x * 4 + x) as usize];
                    minimum.r = minimum.r.min(color.r);
                    minimum.g = minimum.g.min(color.g);
                    minimum.b = minimum.b.min(color.b);
                    maximum.r = maximum.r.max(color.r);
                    maximum.g = maximum.g.max(color.g);
                    maximum.b = maximum.b.max(color.b);
                    if include_alpha {
                        minimum.a = minimum.a.min(color.a);
                        maximum.a = maximum.a.max(color.a);
                    } else {
                        minimum.a = 255;
                        maximum.a = 255;
                    }
                }
            }

            let packet = &mut packets[get_morton_number_rect(block_x, block_y, blocks_x, blocks_y)];
            packet.set_use_punchthrough_alpha(false);
            packet.set_color_a_rgba(minimum, include_alpha);
            packet.set_color_b_rgba(maximum, include_alpha);
        }
    }

    // Pass two selects a modulation value against the bilinearly interpolated
    // endpoints from this packet and its wrapped neighbors.
    for block_y in 0..blocks_y {
        for block_x in 0..blocks_x {
            let mut modulation_data = 0u32;
            let mut factor_index = 0;
            for pixel_y in 0..4 {
                let y_offset = if pixel_y < 2 { -1 } else { 0 };
                let y0 = (block_y + y_offset) & block_mask_y;
                let y1 = (y0 + 1) & block_mask_y;

                for pixel_x in 0..4 {
                    let x_offset = if pixel_x < 2 { -1 } else { 0 };
                    let x0 = (block_x + x_offset) & block_mask_x;
                    let x1 = (x0 + 1) & block_mask_x;
                    let factors = BILINEAR_FACTORS[factor_index];
                    factor_index += 1;

                    let p0 = packets[get_morton_number_rect(x0, y0, blocks_x, blocks_y)];
                    let p1 = packets[get_morton_number_rect(x1, y0, blocks_x, blocks_y)];
                    let p2 = packets[get_morton_number_rect(x0, y1, blocks_x, blocks_y)];
                    let p3 = packets[get_morton_number_rect(x1, y1, blocks_x, blocks_y)];
                    let color_a = p0.get_color_a_rgba() * factors[0] as i32
                        + p1.get_color_a_rgba() * factors[1] as i32
                        + p2.get_color_a_rgba() * factors[2] as i32
                        + p3.get_color_a_rgba() * factors[3] as i32;
                    let color_b = p0.get_color_b_rgba() * factors[0] as i32
                        + p1.get_color_b_rgba() * factors[1] as i32
                        + p2.get_color_b_rgba() * factors[2] as i32
                        + p3.get_color_b_rgba() * factors[3] as i32;

                    let source =
                        colors[((block_y * 4 + pixel_y) * width + block_x * 4 + pixel_x) as usize];
                    let source = ColorRGBA::new(
                        i32::from(source.r) * 16,
                        i32::from(source.g) * 16,
                        i32::from(source.b) * 16,
                        if include_alpha {
                            i32::from(source.a) * 16
                        } else {
                            255 * 16
                        },
                    );
                    let direction = color_b - color_a;
                    let relative = source - color_a;
                    let projection = (relative % direction) * 16;
                    let length_squared = direction % direction;
                    let mut modulation = 0;
                    if projection > 3 * length_squared {
                        modulation += 1;
                    }
                    if projection > 8 * length_squared {
                        modulation += 1;
                    }
                    if projection > 13 * length_squared {
                        modulation += 1;
                    }
                    modulation_data = modulation_data.wrapping_add(modulation).rotate_right(2);
                }
            }
            packets[get_morton_number_rect(block_x, block_y, blocks_x, blocks_y)]
                .set_modulation_data(modulation_data);
        }
    }

    Some(packets)
}

/// Encodes a power-of-two image as a raw PVRTC1 4bpp payload.
pub fn encode_pvrtc_4bpp(image: &DynamicImage, include_alpha: bool) -> Result<Vec<u8>> {
    let rgba = image.to_rgba8();
    if rgba.width() < 4
        || rgba.height() < 4
        || !rgba.width().is_power_of_two()
        || !rgba.height().is_power_of_two()
    {
        return Err(RsbError::Other(format!(
            "PVRTC1 4bpp encoding requires power-of-two dimensions of at least 4x4 pixels, found {}x{}",
            rgba.width(),
            rgba.height()
        )));
    }

    let colors: Vec<Rgba32> = rgba.pixels().copied().map(Rgba32::from_pixel).collect();
    let packets = encode_4bpp_packets(
        &colors,
        rgba.width() as i32,
        rgba.height() as i32,
        include_alpha,
    )
    .ok_or_else(|| RsbError::Other("Invalid PVRTC1 4bpp input".into()))?;
    let mut output = Vec::with_capacity(packets.len() * 8);
    for packet in packets {
        output.extend_from_slice(&packet.pvr_tc_word.to_le_bytes());
    }
    Ok(output)
}
