use std::sync::Arc;

use dzip::{Compression, RangeSettings};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NamedBytes {
    pub(crate) name: String,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SegmentRecipe {
    pub(crate) length: usize,
    pub(crate) raw_flags: u16,
    pub(crate) volume: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EntrySummary {
    pub(crate) id: usize,
    pub(crate) path: String,
    pub(crate) size: u64,
    pub(crate) packed_size: Option<u64>,
    pub(crate) compression: Compression,
    pub(crate) volume: u16,
    pub(crate) chunks: usize,
    pub(crate) segments: Vec<SegmentRecipe>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArchiveSummary {
    pub(crate) session_id: u64,
    pub(crate) name: String,
    pub(crate) entries: Vec<EntrySummary>,
    pub(crate) source_size: u64,
    pub(crate) source_complete: bool,
    pub(crate) unpacked_size: u64,
    pub(crate) chunk_count: usize,
    pub(crate) volume_count: usize,
    pub(crate) loaded_volume_count: usize,
    pub(crate) range_settings: RangeSettings,
    pub(crate) use_common_buffer: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DraftEntry {
    pub(crate) id: u64,
    pub(crate) source_id: Option<usize>,
    pub(crate) path: String,
    pub(crate) bytes: Option<Arc<[u8]>>,
    pub(crate) size: u64,
    pub(crate) packed_size: Option<u64>,
    pub(crate) compression: Compression,
    pub(crate) volume: u16,
    pub(crate) segments: Option<Vec<SegmentRecipe>>,
}

impl DraftEntry {
    pub(crate) fn from_summary(entry: EntrySummary) -> Self {
        Self {
            id: entry.id as u64 + 1,
            source_id: Some(entry.id),
            path: entry.path,
            bytes: None,
            size: entry.size,
            packed_size: entry.packed_size,
            compression: entry.compression,
            volume: entry.volume,
            segments: Some(entry.segments),
        }
    }

    pub(crate) fn replacement(
        id: u64,
        path: String,
        bytes: Vec<u8>,
        compression: Compression,
    ) -> Self {
        let size = bytes.len() as u64;
        Self {
            id,
            source_id: None,
            path,
            bytes: Some(Arc::from(bytes)),
            size,
            packed_size: None,
            compression,
            volume: 0,
            segments: None,
        }
    }

    pub(crate) fn replace_bytes(&mut self, bytes: Vec<u8>) {
        self.size = bytes.len() as u64;
        self.bytes = Some(Arc::from(bytes));
        self.packed_size = None;
        self.segments = None;
    }

    pub(crate) fn replace_compression(&mut self, compression: Compression) {
        if self.compression != compression {
            self.compression = compression;
            self.packed_size = None;
            self.segments = None;
        }
    }

    pub(crate) fn replace_volume(&mut self, volume: u16) {
        if self.volume != volume {
            self.volume = volume;
            self.packed_size = None;
            self.segments = None;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BuildRequest {
    pub(crate) source_session: Option<u64>,
    pub(crate) archive_name: String,
    pub(crate) volume_count: usize,
    pub(crate) alignment: u32,
    pub(crate) range_settings: RangeSettings,
    pub(crate) use_common_buffer: bool,
    pub(crate) entries: Vec<DraftEntry>,
}

#[derive(Clone, Debug)]
pub(crate) struct BuiltArchive {
    pub(crate) volumes: Vec<NamedBytes>,
    pub(crate) summary: ArchiveSummary,
}

#[derive(Clone, Debug)]
pub(crate) struct MaterializeRequest {
    pub(crate) source_session: Option<u64>,
    pub(crate) entries: Vec<DraftEntry>,
}
