mod document;
mod image_name;
mod labels;
mod math;
mod special_layers;
mod timeline;
mod worker_protocol;

pub use document::{ImageAsset, PamDocument};
pub use image_name::parse_image_file_name;
pub use labels::{FrameLabel, parse_frame_labels};
pub use math::{Color, Matrix, Rect, multiply_color, multiply_matrix, transform_to_matrix};
pub use pam_codec::{
    AddsInfo, FrameInfo, ImageInfo, MovesInfo, PamInfo, Rectangle, RemovesInfo, SpriteInfo,
    decode_pam, encode_pam,
};
pub use special_layers::{SpecialLayerIndices, special_layer_indices};
pub use timeline::{
    CompiledAnimation, DrawCommand, LayerSnapshot, SpriteKey, SpriteTimeline, compile_animation,
};
pub use worker_protocol::{
    ExportKind, ExportRequest, ImageAssetPayload, LoadedPamPayload, PamDocumentPayload,
    RenderDocumentPayload, RenderImageAssetPayload, RenderScenePayload, RenderViewPayload,
    WorkerInputFile, WorkerRequest, WorkerResponse,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid transform length {actual}; expected 2, 3, or 6 values")]
    InvalidTransform { actual: usize },
    #[error("sprite {0:?} does not exist")]
    MissingSprite(SpriteKey),
    #[error("PAM animation has no main sprite")]
    MissingMainSprite,
    #[error("PAM codec error: {0}")]
    Codec(#[from] pam_codec::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub fn decode_pam_bytes(bytes: &[u8]) -> Result<PamInfo> {
    Ok(pam_codec::decode_pam(&mut std::io::Cursor::new(bytes))?)
}

pub fn encode_pam_bytes(pam: &PamInfo) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    pam_codec::encode_pam(pam, &mut output)?;
    Ok(output)
}

/// JSON is intentionally serialized and deserialized directly through pam-codec's
/// serde definitions. There is no second compatibility schema in pam-viewer.
pub fn decode_json(text: &str) -> Result<PamInfo> {
    Ok(serde_json::from_str(text)?)
}

pub fn encode_json(pam: &PamInfo) -> Result<String> {
    let mut text = serde_json::to_string_pretty(pam)?;
    text.push('\n');
    Ok(text)
}
