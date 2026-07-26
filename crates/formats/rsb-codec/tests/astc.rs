use rsb_codec::{
    ASTC_BLOCK_SIZES, AstcQuality, PtxFormat, RsbError, astc_data_size, decode_astc,
    is_valid_astc_block_size,
};

#[test]
fn uses_twinning_pvz2_astc_format_codes() {
    let expected = [(160, (4, 4)), (161, (5, 5)), (162, (6, 6)), (163, (8, 8))];

    for (code, (block_width, block_height)) in expected {
        assert_eq!(
            PtxFormat::from(code),
            PtxFormat::Astc {
                block_width,
                block_height,
            }
        );
    }
    assert_eq!(PtxFormat::from(159), PtxFormat::Unknown(159));
}

#[test]
fn validates_astc_footprints_and_payload_sizes() {
    for (block_width, block_height) in ASTC_BLOCK_SIZES {
        assert!(is_valid_astc_block_size(block_width, block_height));
        assert_eq!(
            astc_data_size(block_width, block_height, block_width, block_height).unwrap(),
            16
        );
    }

    assert_eq!(astc_data_size(13, 9, 5, 4).unwrap(), 144);
    assert!(matches!(
        astc_data_size(8, 8, 7, 7),
        Err(RsbError::InvalidAstcBlockSize {
            width: 7,
            height: 7
        })
    ));
    assert!(matches!(
        decode_astc(&[0; 15], 4, 4, 4, 4),
        Err(RsbError::InvalidAstcDataSize {
            expected: 16,
            actual: 15
        })
    ));
}

#[test]
fn validates_twinning_quality_range() {
    assert_eq!(AstcQuality::FASTEST.get(), 0);
    assert_eq!(AstcQuality::MEDIUM.get(), 60);
    assert_eq!(AstcQuality::EXHAUSTIVE.get(), 100);
    assert_eq!(AstcQuality::new(37).unwrap().get(), 37);
    assert!(matches!(
        AstcQuality::new(101),
        Err(RsbError::InvalidAstcQuality(101))
    ));
}

#[test]
fn pure_rust_encoder_output_decodes_for_every_astc_footprint() {
    use image::{DynamicImage, ImageBuffer, Rgba};
    use rsb_codec::encode_astc;

    let expected = [63_u8, 127, 191, 223];

    for (block_width, block_height) in ASTC_BLOCK_SIZES {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            block_width,
            block_height,
            Rgba(expected),
        ));
        let encoded = encode_astc(&image, block_width, block_height, AstcQuality::FASTEST).unwrap();
        assert_eq!(encoded.len(), 16, "{block_width}x{block_height}");

        let decoded = decode_astc(
            &encoded,
            block_width,
            block_height,
            block_width,
            block_height,
        )
        .unwrap()
        .to_rgba8();
        let actual = decoded.get_pixel(0, 0).0;
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                actual.abs_diff(expected) <= 8,
                "{block_width}x{block_height}: expected {expected}, found {actual}"
            );
        }
    }
}

#[test]
fn ptx_encoder_and_decoder_dispatch_pvz2_astc_codes() {
    use image::{DynamicImage, ImageBuffer, Rgba};
    use rsb_codec::{PtxDecoder, PtxEncoder};

    for (code, block_width, block_height) in [(160, 4, 4), (161, 5, 5), (162, 6, 6), (163, 8, 8)] {
        let format = PtxFormat::from(code);
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
            block_width,
            block_height,
            Rgba([32, 96, 160, 224]),
        ));
        let encoded = PtxEncoder::encode(&image, format, false).unwrap();
        assert_eq!(encoded.len(), 16, "format code {code}");

        let decoded = PtxDecoder::decode(
            &encoded,
            block_width,
            block_height,
            code,
            None,
            None,
            None,
            false,
        )
        .unwrap();
        assert_eq!(
            (decoded.width(), decoded.height()),
            (block_width, block_height)
        );
    }
}

#[test]
fn preserves_exact_constant_colors() {
    use image::{DynamicImage, ImageBuffer, Rgba};
    use rsb_codec::{decode_astc, encode_astc};

    let expected = [17, 83, 149, 211];
    let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(13, 9, Rgba(expected)));
    let encoded = encode_astc(&image, 5, 4, AstcQuality::FASTEST).unwrap();
    let decoded = decode_astc(&encoded, 13, 9, 5, 4).unwrap().to_rgba8();
    assert!(decoded.pixels().all(|pixel| pixel.0 == expected));
}

#[test]
fn encodes_color_and_independent_alpha_gradients() {
    use image::{DynamicImage, ImageBuffer, Rgba};
    use rsb_codec::{decode_astc, encode_astc};

    let image = ImageBuffer::from_fn(16, 16, |x, y| {
        Rgba([
            (x * 17) as u8,
            (y * 17) as u8,
            ((x + y) * 8) as u8,
            if (x / 4 + y / 4) % 2 == 0 { 32 } else { 224 },
        ])
    });
    let encoded = encode_astc(
        &DynamicImage::ImageRgba8(image.clone()),
        4,
        4,
        AstcQuality::MEDIUM,
    )
    .unwrap();
    let decoded = decode_astc(&encoded, 16, 16, 4, 4).unwrap().to_rgba8();

    let mean_absolute_error = image
        .pixels()
        .zip(decoded.pixels())
        .flat_map(|(expected, actual)| {
            expected
                .0
                .into_iter()
                .zip(actual.0)
                .map(|(expected, actual)| u64::from(expected.abs_diff(actual)))
        })
        .sum::<u64>() as f64
        / (16.0 * 16.0 * 4.0);
    assert!(
        mean_absolute_error < 24.0,
        "unexpected ASTC error: {mean_absolute_error}"
    );
}

#[test]
fn rejects_illegal_blocks_without_panicking() {
    assert!(decode_astc(&[0; 16], 4, 4, 4, 4).is_err());

    let mut state = 0x8D26_5A4C_91E3_77B1u64;
    for (block_width, block_height) in ASTC_BLOCK_SIZES {
        for _ in 0..256 {
            let mut block = [0u8; 16];
            for byte in &mut block {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                *byte = state as u8;
            }
            let _ = decode_astc(&block, block_width, block_height, block_width, block_height);
        }
    }
}
