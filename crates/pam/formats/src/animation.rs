use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use image_webp::{ColorType as WebpColorType, WebPEncoder};

use crate::{FormatError, Result};

pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    validate_frame(rgba, width, height)?;
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Balanced);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgba)?;
    }
    Ok(output)
}

pub fn encode_apng(frames: &[Vec<u8>], width: u32, height: u32, fps: u32) -> Result<Vec<u8>> {
    encode_apng_with_cancel(frames, width, height, fps, None)
}

pub fn encode_apng_with_cancel(
    frames: &[Vec<u8>],
    width: u32,
    height: u32,
    fps: u32,
    cancelled: Option<&AtomicBool>,
) -> Result<Vec<u8>> {
    if frames.is_empty() {
        return Err(FormatError::NoFrames);
    }
    for frame in frames {
        validate_frame(frame, width, height)?;
    }
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Balanced);
        encoder.set_animated(frames.len() as u32, 0)?;
        encoder.set_frame_delay(1, fps.clamp(1, u16::MAX as u32) as u16)?;
        encoder.set_blend_op(png::BlendOp::Source)?;
        encoder.set_dispose_op(png::DisposeOp::None)?;
        let mut writer = encoder.write_header()?;
        for frame in frames {
            ensure_not_cancelled(cancelled)?;
            writer.write_image_data(frame)?;
        }
    }
    Ok(output)
}

/// Encodes full-canvas lossless VP8L frames and assembles the standard animated
/// WebP RIFF container in Rust, so the same path works on native and wasm.
pub fn encode_animated_webp(
    frames: &[Vec<u8>],
    width: u32,
    height: u32,
    fps: u32,
) -> Result<Vec<u8>> {
    encode_animated_webp_with_cancel(frames, width, height, fps, None)
}

pub fn encode_animated_webp_with_cancel(
    frames: &[Vec<u8>],
    width: u32,
    height: u32,
    fps: u32,
    cancelled: Option<&AtomicBool>,
) -> Result<Vec<u8>> {
    if frames.is_empty() {
        return Err(FormatError::NoFrames);
    }
    let duration_ms = (1000.0 / fps.max(1) as f64)
        .round()
        .clamp(1.0, 16_777_215.0) as u32;
    let encode_frame = |frame: &Vec<u8>| {
        ensure_not_cancelled(cancelled)?;
        validate_frame(frame, width, height)?;
        let mut still = Vec::new();
        WebPEncoder::new(&mut still).encode(frame, width, height, WebpColorType::Rgba8)?;
        extract_image_chunks(&still)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let chunks = if frames.len() >= 4 {
        use rayon::prelude::*;
        frames
            .par_iter()
            .map(encode_frame)
            .collect::<Result<Vec<_>>>()?
    } else {
        frames
            .iter()
            .map(encode_frame)
            .collect::<Result<Vec<_>>>()?
    };
    #[cfg(target_arch = "wasm32")]
    let chunks = frames
        .iter()
        .map(encode_frame)
        .collect::<Result<Vec<_>>>()?;

    let mut body = Vec::new();
    let mut vp8x = vec![(1 << 1) | (1 << 4), 0, 0, 0];
    push_u24(&mut vp8x, width.saturating_sub(1));
    push_u24(&mut vp8x, height.saturating_sub(1));
    write_riff_chunk(&mut body, b"VP8X", &vp8x)?;
    write_riff_chunk(&mut body, b"ANIM", &[0, 0, 0, 0, 0, 0])?;
    for image_chunks in chunks {
        ensure_not_cancelled(cancelled)?;
        let mut frame = Vec::new();
        push_u24(&mut frame, 0);
        push_u24(&mut frame, 0);
        push_u24(&mut frame, width.saturating_sub(1));
        push_u24(&mut frame, height.saturating_sub(1));
        push_u24(&mut frame, duration_ms);
        frame.push(0b10); // Full-frame replacement; do not alpha-blend with the prior frame.
        frame.extend_from_slice(&image_chunks);
        write_riff_chunk(&mut body, b"ANMF", &frame)?;
    }

    let mut output = Vec::with_capacity(body.len() + 12);
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    output.extend_from_slice(b"WEBP");
    output.extend_from_slice(&body);
    Ok(output)
}

fn ensure_not_cancelled(cancelled: Option<&AtomicBool>) -> Result<()> {
    if cancelled.is_some_and(|cancelled| cancelled.load(Ordering::Relaxed)) {
        Err(FormatError::Cancelled)
    } else {
        Ok(())
    }
}

fn validate_frame(frame: &[u8], width: u32, height: u32) -> Result<()> {
    let expected = width as usize * height as usize * 4;
    if frame.len() != expected {
        return Err(FormatError::InvalidFrameSize {
            expected,
            actual: frame.len(),
        });
    }
    Ok(())
}

fn extract_image_chunks(still_webp: &[u8]) -> Result<Vec<u8>> {
    if still_webp.len() < 20 || &still_webp[..4] != b"RIFF" || &still_webp[8..12] != b"WEBP" {
        return Err(FormatError::Fla("invalid lossless WebP frame".into()));
    }
    let mut offset = 12;
    let mut output = Vec::new();
    while offset + 8 <= still_webp.len() {
        let kind = &still_webp[offset..offset + 4];
        let size =
            u32::from_le_bytes(still_webp[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let chunk_end = offset + 8 + size + (size & 1);
        if chunk_end > still_webp.len() {
            return Err(FormatError::Fla("truncated WebP frame".into()));
        }
        if matches!(kind, b"ALPH" | b"VP8 " | b"VP8L") {
            output.extend_from_slice(&still_webp[offset..chunk_end]);
        }
        offset = chunk_end;
    }
    if output.is_empty() {
        return Err(FormatError::Fla("WebP frame has no image chunk".into()));
    }
    Ok(output)
}

fn push_u24(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes()[..3]);
}

fn write_riff_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) -> Result<()> {
    output.write_all(kind)?;
    output.write_all(&(data.len() as u32).to_le_bytes())?;
    output.write_all(data)?;
    if data.len() & 1 != 0 {
        output.push(0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::atomic::AtomicBool;

    use super::*;

    #[test]
    fn png_roundtrips_rgba_pixels() {
        let pixels = vec![255, 0, 0, 255, 0, 255, 0, 128];
        let encoded = encode_png(&pixels, 2, 1).unwrap();
        let decoded = image::load_from_memory(&encoded).unwrap().to_rgba8();
        assert_eq!(decoded.as_raw(), &pixels);
    }

    #[test]
    fn apng_exposes_all_frames_and_timing() {
        let frames = vec![vec![255; 2 * 2 * 4], vec![0; 2 * 2 * 4]];
        let encoded = encode_apng(&frames, 2, 2, 60).unwrap();
        let decoder = png::Decoder::new(Cursor::new(encoded));
        let mut reader = decoder.read_info().unwrap();
        assert_eq!(
            reader
                .info()
                .animation_control
                .as_ref()
                .map(|value| value.num_frames),
            Some(2)
        );
        let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut buffer).unwrap();
        let timing = reader.info().frame_control.as_ref().unwrap();
        assert_eq!((timing.delay_num, timing.delay_den), (1, 60));
        reader.next_frame(&mut buffer).unwrap();
    }

    #[test]
    fn animated_webp_has_animation_chunks() {
        let frames = vec![vec![255; 2 * 2 * 4], vec![0; 2 * 2 * 4]];
        let encoded = encode_animated_webp(&frames, 2, 2, 30).unwrap();
        assert_eq!(&encoded[..4], b"RIFF");
        assert!(encoded.windows(4).any(|chunk| chunk == b"ANIM"));
        assert_eq!(
            encoded.windows(4).filter(|chunk| *chunk == b"ANMF").count(),
            2
        );
    }

    #[test]
    fn animated_encoders_honor_cancellation() {
        let frames = vec![vec![255; 2 * 2 * 4]; 2];
        let cancelled = AtomicBool::new(true);
        assert!(matches!(
            encode_apng_with_cancel(&frames, 2, 2, 30, Some(&cancelled)),
            Err(FormatError::Cancelled)
        ));
        assert!(matches!(
            encode_animated_webp_with_cancel(&frames, 2, 2, 30, Some(&cancelled)),
            Err(FormatError::Cancelled)
        ));
    }
}
