use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;

#[cfg(any(not(target_arch = "wasm32"), test))]
use super::ByteDocumentReader;
use super::ByteSource;
use crate::domain::HexEdit;

#[derive(Debug, Clone)]
pub(crate) struct ByteDocument {
    pub(crate) source: ByteSource,
    version: u64,
}

impl ByteDocument {
    pub(crate) fn from_vec(bytes: Vec<u8>) -> Self {
        Self::from_vec_with_version(bytes, 0)
    }

    pub(crate) fn from_vec_with_version(bytes: Vec<u8>, version: u64) -> Self {
        Self {
            source: ByteSource::Memory(Arc::<[u8]>::from(bytes)),
            version,
        }
    }

    pub(crate) fn from_file_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let source = ByteSource::from_file_path(path)?;
        Ok(Self { source, version: 0 })
    }

    pub(crate) fn apply_edit(&self, edit: &HexEdit) -> Self {
        Self {
            source: self.source.apply_edit(edit),
            version: self.next_version(),
        }
    }

    pub(crate) fn apply_edits(&self, edits: &[HexEdit]) -> Self {
        edits
            .iter()
            .fold(self.clone(), |document, edit| document.apply_edit(edit))
    }

    pub(crate) fn len(&self) -> usize {
        self.source.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.source.len() == 0
    }

    pub(crate) fn byte_at(&self, offset: usize) -> Option<u8> {
        self.source.byte_at(offset)
    }

    pub(crate) fn range_to_vec(&self, start: usize, end: usize) -> Option<Vec<u8>> {
        self.source.range_to_vec(start, end)
    }

    pub(crate) fn copy_range_to(&self, start: usize, output: &mut [u8]) -> Option<usize> {
        self.source.copy_range_to(start, output)
    }

    pub(crate) fn as_cow(&self) -> Cow<'_, [u8]> {
        self.source.as_cow()
    }

    #[cfg(any(not(target_arch = "wasm32"), test))]
    pub(crate) fn reader(&self) -> ByteDocumentReader {
        ByteDocumentReader {
            document: self.clone(),
            position: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn to_vec(&self) -> Vec<u8> {
        self.source.to_vec()
    }

    fn next_version(&self) -> u64 {
        self.version.saturating_add(1)
    }
}

impl PartialEq for ByteDocument {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version && self.source.same_identity(&other.source)
    }
}
