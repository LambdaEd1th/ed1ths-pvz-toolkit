use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{CompiledAnimation, CoreError, ImageAsset, PamDocument, PamInfo, Rect, SpriteKey};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerInputFile {
    pub path: String,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
}

impl WorkerInputFile {
    pub fn new(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into().replace('\\', "/"),
            bytes: bytes.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageAssetPayload {
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[serde(with = "serde_bytes")]
    pub rgba: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub encoded: Vec<u8>,
}

impl From<&ImageAsset> for ImageAssetPayload {
    fn from(asset: &ImageAsset) -> Self {
        Self {
            name: asset.name.clone(),
            width: asset.width,
            height: asset.height,
            rgba: asset.rgba.to_vec(),
            encoded: asset.encoded.to_vec(),
        }
    }
}

impl From<ImageAssetPayload> for ImageAsset {
    fn from(asset: ImageAssetPayload) -> Self {
        Self::new(
            asset.name,
            asset.width,
            asset.height,
            Arc::<[u8]>::from(asset.rgba),
            Arc::<[u8]>::from(asset.encoded),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PamDocumentPayload {
    pub source_name: String,
    pub pam: PamInfo,
    pub images: Vec<Option<ImageAssetPayload>>,
    pub compiled: CompiledAnimation,
    pub content_bounds: Option<Rect>,
}

impl From<&PamDocument> for PamDocumentPayload {
    fn from(document: &PamDocument) -> Self {
        Self {
            source_name: document.source_name.clone(),
            pam: document.pam.clone(),
            images: document
                .images
                .iter()
                .map(|asset| asset.as_ref().map(ImageAssetPayload::from))
                .collect(),
            compiled: document.compiled.clone(),
            content_bounds: document.content_bounds(),
        }
    }
}

impl PamDocumentPayload {
    pub fn into_document(self) -> Result<PamDocument, CoreError> {
        Ok(PamDocument::from_compiled(
            self.source_name,
            self.pam,
            self.images
                .into_iter()
                .map(|asset| asset.map(ImageAsset::from))
                .collect(),
            self.compiled,
            self.content_bounds,
        ))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenderImageAssetPayload {
    pub name: String,
    pub width: u32,
    pub height: u32,
    #[serde(with = "serde_bytes")]
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenderDocumentPayload {
    pub source_name: String,
    pub pam: PamInfo,
    pub images: Vec<Option<RenderImageAssetPayload>>,
    pub compiled: CompiledAnimation,
    pub content_bounds: Option<Rect>,
}

impl From<&PamDocument> for RenderDocumentPayload {
    fn from(document: &PamDocument) -> Self {
        Self {
            source_name: document.source_name.clone(),
            pam: document.pam.clone(),
            images: document
                .images
                .iter()
                .map(|asset| {
                    asset.as_ref().map(|asset| RenderImageAssetPayload {
                        name: asset.name.clone(),
                        width: asset.width,
                        height: asset.height,
                        rgba: asset.rgba.to_vec(),
                    })
                })
                .collect(),
            compiled: document.compiled.clone(),
            content_bounds: document.content_bounds(),
        }
    }
}

impl RenderDocumentPayload {
    pub fn into_document(self) -> PamDocument {
        PamDocument::from_compiled(
            self.source_name,
            self.pam,
            self.images
                .into_iter()
                .map(|asset| {
                    asset.map(|asset| {
                        ImageAsset::new(
                            asset.name,
                            asset.width,
                            asset.height,
                            Arc::<[u8]>::from(asset.rgba),
                            Arc::<[u8]>::from([]),
                        )
                    })
                })
                .collect(),
            self.compiled,
            self.content_bounds,
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoadedPamPayload {
    pub document: PamDocumentPayload,
    pub loaded_images: usize,
    pub missing_images: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    Png,
    Apng,
    Webp,
    Fla,
    Json,
    Yaml,
    Toml,
    Pam,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExportRequest {
    pub document_id: u64,
    pub operation_id: u64,
    pub kind: ExportKind,
    pub sprite: SpriteKey,
    pub current_frame: usize,
    pub frame_range: [usize; 2],
    pub image_filter: Vec<bool>,
    pub sprite_filter: Vec<bool>,
    pub size: [u32; 2],
    pub fps: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerRequest {
    Ping,
    Batch {
        requests: Vec<WorkerRequest>,
    },
    Load {
        document_id: u64,
        files: Vec<WorkerInputFile>,
    },
    RegisterDocument {
        document_id: u64,
        document: PamDocumentPayload,
    },
    Export(ExportRequest),
    CancelExport {
        document_id: u64,
        operation_id: u64,
    },
    ReleaseDocument {
        document_id: u64,
    },
}

impl WorkerRequest {
    pub fn document_id(&self) -> Option<u64> {
        match self {
            Self::Load { document_id, .. }
            | Self::RegisterDocument { document_id, .. }
            | Self::CancelExport { document_id, .. }
            | Self::ReleaseDocument { document_id } => Some(*document_id),
            Self::Export(request) => Some(request.document_id),
            Self::Ping | Self::Batch { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkerResponse {
    Pong,
    Batch {
        responses: Vec<WorkerResponse>,
    },
    Loaded {
        loaded: Box<LoadedPamPayload>,
    },
    Registered,
    Exported {
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    Cancelled,
    Released,
    Error {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenderViewPayload {
    pub sprite: SpriteKey,
    pub frame: usize,
    pub image_filter: Vec<bool>,
    pub sprite_filter: Vec<bool>,
    pub zoom: f32,
    pub pan: [f32; 2],
    pub boundary: bool,
    pub dark_background: bool,
}

impl Default for RenderViewPayload {
    fn default() -> Self {
        Self {
            sprite: SpriteKey::Main,
            frame: 0,
            image_filter: Vec::new(),
            sprite_filter: Vec::new(),
            zoom: 1.0,
            pan: [0.0, 0.0],
            boundary: true,
            dark_background: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RenderScenePayload {
    pub document: Option<RenderDocumentPayload>,
    pub view: RenderViewPayload,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_payload_preserves_compiled_timeline() {
        let document = PamDocument::new(
            "sample.pam",
            PamInfo {
                version: 6,
                frame_rate: 30,
                position: [0.0, 0.0],
                size: [64.0, 64.0],
                image: Vec::new(),
                sprite: Vec::new(),
                main_sprite: None,
            },
            Vec::new(),
        )
        .unwrap();

        let rebuilt = PamDocumentPayload::from(&document).into_document().unwrap();
        assert_eq!(rebuilt.source_name, "sample.pam");
        assert_eq!(rebuilt.pam, document.pam);
        assert_eq!(rebuilt.compiled, document.compiled);
        assert_eq!(rebuilt.content_bounds(), document.content_bounds());
    }
}
