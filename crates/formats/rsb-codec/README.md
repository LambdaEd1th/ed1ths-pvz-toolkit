# rsb-codec

`rsb-codec` is a Rust library for reading and writing PopCap/PvZ2 RSB bundles,
packing and unpacking their RSG packets, and decoding or encoding embedded PTX
textures.

It is an independent format crate: applications can depend on it without the
Toolkit UI, a CLI, or an aggregate SDK.

## Features

- Read RSB headers, file lists, packet metadata, composites, auto pools, PTX
  metadata, and resource descriptions.
- Represent the RSB/RSG Magic values as the semantic `rsb1`/`rsgp` FourCCs and
  encode them as little-endian `u32` values (`1bsr`/`pgsr` on disk).
- Write RSB metadata sections and extract embedded RSG packets.
- Pack and unpack RSG packet data with PopCap-compatible zlib compression.
- Decode and encode common PTX pixel formats, including ETC1, PVRTC, and ASTC.
- Serialize public metadata types with Serde.
- Use generic `Read + Seek` and `Write + Seek` streams instead of filesystem-only
  APIs.

## Installation

```toml
[dependencies]
rsb-codec = { git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit" }
```

ASTC and PVRTC decoding and encoding are implemented in pure Rust. They do not
require a C/C++ compiler or architecture-specific native libraries, and are
available on WebAssembly.

### Optional WGPU acceleration

UI applications can enable the optional GPU backend without depending on a
separate renderer crate:

```toml
[dependencies]
rsb-codec = {
    git = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit",
    features = ["gpu"],
}
```

The `rsb_codec::ptx::gpu` module provides:

- Direct ASTC and ETC1 sampling when the device was created with the matching
  WGPU compressed-texture feature.
- Portable WGSL ETC1 and PVRTC decoding to render-ready RGBA textures,
  including PVRTC+A8, ETC1+A8, ETC1 with compressed alpha, and ETC1 palette
  alpha payloads.
- Fast block-parallel ASTC, ETC1, and PVRTC encoders for interactive previews,
  with GPU-side A8, compressed-alpha, and palette-index generation.
- RGBA readback and an aspect-fit `PtxPreviewRenderer` for surfaces or
  offscreen render targets.
- Automatic CPU fallback on WebGL2 or devices without the required compute or
  compressed-texture capabilities.

Use `supported_optional_features(&adapter)` when requesting the WGPU device.
The compute encoders prioritize interactive latency: ASTC uses a void-extent
color per block and ETC1 uses an average-color individual-mode fit. Continue
to use `PtxEncoder` for final, quality-oriented exports.

CPU and GPU APIs use the same resolved `PtxFormat`. `Etc1A8` represents an
uncompressed A8 plane (`alpha_format = 100`), while `Etc1CompressedAlpha`
represents a second ETC1 grayscale plane.

## Decode a PTX payload

Resolve ambiguous RSB format codes once, validate and split the payload, then
pass the same descriptor or payload to CPU and GPU consumers:

```rust,no_run
use rsb_codec::{
    ChannelOrder, PtxDecoder, PtxDescriptor, PtxFormatCode, PtxPayload,
    PtxRsbMetadata,
};

fn decode(
    bytes: &[u8],
    width: u32,
    height: u32,
) -> Result<image::RgbaImage, rsb_codec::PtxError> {
    let descriptor = PtxDescriptor::from_rsb(
        width,
        height,
        PtxRsbMetadata {
            format_code: PtxFormatCode(147),
            alpha_size: Some(width * height),
            alpha_format: Some(100),
            row_pitch: None,
            channel_order: ChannelOrder::Rgba,
        },
    )?;
    let payload = PtxPayload::parse(bytes, descriptor)?;
    PtxDecoder::decode_payload(&payload)
}
```

All payload length, block-grid, row-pitch, color-plane and alpha-plane
validation lives in `PtxPayloadLayout`; backends do not independently guess
plane offsets.

## Read an RSB

```rust,no_run
use rsb_codec::Rsb;
use std::fs::File;

fn main() -> rsb_codec::Result<()> {
    let file = File::open("main.rsb")?;
    let mut rsb = Rsb::open(file)?;

    println!("RSB version: {}", rsb.header.version);
    for packet in rsb.read_rsg_info()? {
        println!("{}: {} bytes", packet.name, packet.rsg_length);
    }

    Ok(())
}
```

## Pack and unpack an RSG

```rust,no_run
use rsb_codec::{Part1Extra, UnpackedFile, pack_rsg, unpack_rsg};
use std::io::Cursor;

fn main() -> rsb_codec::Result<()> {
    let files = vec![UnpackedFile {
        path: "IMAGES/EXAMPLE.PTX".into(),
        data: vec![0; 16],
        is_part1: true,
        part1_info: Some(Part1Extra {
            id: 0,
            width: 2,
            height: 2,
        }),
    }];

    let mut packed = Cursor::new(Vec::new());
    pack_rsg(&mut packed, &files, 4, 3)?;

    packed.set_position(0);
    let unpacked = unpack_rsg(&mut packed)?;
    assert_eq!(unpacked[0].path, "IMAGES/EXAMPLE.PTX");

    Ok(())
}
```

## Encode an ASTC PTX payload

```rust,no_run
use image::DynamicImage;
use rsb_codec::{AstcQuality, PtxEncoder};

fn encode(image: &DynamicImage) -> rsb_codec::Result<Vec<u8>> {
    PtxEncoder::encode_astc(image, 4, 4, AstcQuality::MEDIUM)
}
```

`AstcQuality` controls the search effort of the pure Rust encoder. It retains
the `0..=100` range used by Twinning, but does not promise byte-identical output
or identical preset behavior to Arm `astcenc`.

## Encode a PVRTC PTX payload

```rust,no_run
use image::DynamicImage;
use rsb_codec::encode_pvrtc_4bpp;

fn encode(image: &DynamicImage) -> rsb_codec::Result<Vec<u8>> {
    encode_pvrtc_4bpp(image, true)
}
```

The second argument controls whether alpha participates in the PVRTC endpoint
and modulation search. `PtxEncoder` disables it automatically for the
PVRTC+A8 format because that format stores an independent uncompressed alpha
plane. Width and height must each be powers of two and at least four pixels.

The PvZ2 China PTX format codes follow Twinning's conversion map:

| PTX code | ASTC footprint |
| --- | --- |
| `160` | `4x4` |
| `161` | `5x5` |
| `162` | `6x6` |
| `163` | `8x8` |

## License

AGPL-3.0-or-later. See the [workspace LICENSE](../../../LICENSE).
