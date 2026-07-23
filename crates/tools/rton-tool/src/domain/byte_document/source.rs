use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use super::MappedByteSource;
use super::mmap::from_file_path_source;
use super::{BytePiece, BytePieceSource, PieceBytes};
use crate::domain::HexEdit;

#[derive(Debug, Clone)]
pub(crate) enum ByteSource {
    Memory(Arc<[u8]>),
    #[cfg(not(target_arch = "wasm32"))]
    FileMap(Arc<MappedByteSource>),
    Pieces(Arc<PieceBytes>),
}

impl ByteSource {
    pub(super) fn from_file_path(path: impl AsRef<Path>) -> Result<Self, String> {
        from_file_path_source(path)
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Memory(bytes) => bytes.len(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => source.bytes.len(),
            Self::Pieces(source) => source.len,
        }
    }

    pub(super) fn byte_at(&self, offset: usize) -> Option<u8> {
        match self {
            Self::Memory(bytes) => bytes.get(offset).copied(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => source.bytes.get(offset).copied(),
            Self::Pieces(source) => source.byte_at(offset),
        }
    }

    pub(super) fn range_to_vec(&self, start: usize, end: usize) -> Option<Vec<u8>> {
        if start > end || end > self.len() {
            return None;
        }

        match self {
            Self::Memory(bytes) => Some(bytes[start..end].to_vec()),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => Some(source.bytes[start..end].to_vec()),
            Self::Pieces(source) => source.range_to_vec(start, end),
        }
    }

    pub(super) fn copy_range_to(&self, start: usize, output: &mut [u8]) -> Option<usize> {
        if output.is_empty() {
            return Some(0);
        }
        if start > self.len() {
            return None;
        }
        let read = output.len().min(self.len().saturating_sub(start));
        match self {
            Self::Memory(bytes) => output[..read].copy_from_slice(&bytes[start..start + read]),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => {
                output[..read].copy_from_slice(&source.bytes[start..start + read])
            }
            Self::Pieces(source) => return source.copy_range_to(start, output),
        }
        Some(read)
    }

    pub(super) fn as_cow(&self) -> Cow<'_, [u8]> {
        match self {
            Self::Memory(bytes) => Cow::Borrowed(bytes.as_ref()),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => Cow::Borrowed(source.bytes.as_ref()),
            Self::Pieces(source) => Cow::Owned(source.to_vec()),
        }
    }

    #[cfg(test)]
    pub(super) fn to_vec(&self) -> Vec<u8> {
        match self {
            Self::Memory(bytes) => bytes.to_vec(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => source.bytes.to_vec(),
            Self::Pieces(source) => source.to_vec(),
        }
    }

    pub(super) fn apply_edit(&self, edit: &HexEdit) -> Self {
        let source = PieceBytes::from_source(self).apply_edit(edit);
        Self::Pieces(Arc::new(source))
    }

    pub(super) fn pieces(&self) -> Vec<BytePiece> {
        match self {
            Self::Memory(bytes) => {
                if bytes.is_empty() {
                    Vec::new()
                } else {
                    vec![BytePiece {
                        source: BytePieceSource::Memory(bytes.clone()),
                        start: 0,
                        len: bytes.len(),
                    }]
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => {
                if source.bytes.is_empty() {
                    Vec::new()
                } else {
                    vec![BytePiece {
                        source: BytePieceSource::FileMap(source.clone()),
                        start: 0,
                        len: source.bytes.len(),
                    }]
                }
            }
            Self::Pieces(source) => source.pieces.clone(),
        }
    }

    pub(super) fn same_identity(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Memory(left), Self::Memory(right)) => Arc::ptr_eq(left, right),
            #[cfg(not(target_arch = "wasm32"))]
            (Self::FileMap(left), Self::FileMap(right)) => Arc::ptr_eq(left, right),
            (Self::Pieces(left), Self::Pieces(right)) => Arc::ptr_eq(left, right),
            #[cfg(not(target_arch = "wasm32"))]
            (Self::Memory(_), Self::FileMap(_))
            | (Self::FileMap(_), Self::Memory(_))
            | (Self::Memory(_), Self::Pieces(_))
            | (Self::Pieces(_), Self::Memory(_))
            | (Self::FileMap(_), Self::Pieces(_))
            | (Self::Pieces(_), Self::FileMap(_)) => false,
            #[cfg(target_arch = "wasm32")]
            (Self::Memory(_), Self::Pieces(_)) | (Self::Pieces(_), Self::Memory(_)) => false,
        }
    }
}
