//! Reader, writer, packet, and texture codecs for PopCap/PvZ2 RSB bundles.
//!
//! The crate works with standard Rust streams and does not depend on the
//! Toolkit application. Start with [`Rsb`] to inspect an existing bundle,
//! [`RsbWriter`] to write metadata, [`pack_rsg`] and [`unpack_rsg`] for packet
//! payloads, or [`PtxDecoder`] and [`PtxEncoder`] for embedded textures.

pub mod error;
pub mod io;
pub mod ptx;
pub mod rsg;
pub mod schema;
mod utils;

pub use error::{Result, RsbError};
pub use io::reader::Rsb;
pub use io::writer::RsbWriter;
#[cfg(feature = "gpu")]
pub use ptx::gpu::{
    DecodeBackend, EncodeBackend, GpuEncodeOptions, GpuEncodedPtx, PreviewOptions, PtxGpuCodec,
    PtxGpuError, PtxGpuFormat, PtxGpuTexture, PtxPreviewRenderer, PtxTextureDescriptor,
    optional_texture_features, supported_optional_features,
};
pub use ptx::{
    ASTC_BLOCK_SIZES, AstcQuality, ChannelOrder, PtxAlphaEncoding, PtxDecoder, PtxDescriptor,
    PtxEncodeOptions, PtxEncoder, PtxError, PtxFormat, PtxFormatCode, PtxPayload, PtxPayloadLayout,
    PtxPlaneLayout, PtxResult, PtxRsbMetadata, RgbaSurface, astc_data_size, decode_astc,
    decode_pvrtc_4bpp, decode_pvrtc_4bpp_a8, encode_astc, encode_pvrtc_4bpp,
    is_valid_astc_block_size, resolve_rsb_format,
};
pub use rsg::{
    Part0Info, Part1Extra, Part1Info, RSG_MAGIC, RsgHeader, RsgPayload, UnpackedFile, pack_rsg,
    unpack_rsg,
};
pub use schema::types::*;
