//! Owned and borrowed embedded-media helpers.

use crate::error::{BnkError, Result};
use crate::types::{
    BankChunk, ChunkId, EmbeddedMedia, MediaIndexEntry, OwnedEmbeddedMedia, SoundBank,
};

impl SoundBank {
    /// Resolve the first DIDX/DATA pair into borrowed embedded WEM payloads.
    pub fn embedded_media(&self) -> Result<Vec<EmbeddedMedia<'_>>> {
        let Some(index_position) = self
            .chunks
            .iter()
            .position(|chunk| matches!(chunk, BankChunk::MediaIndex(_)))
        else {
            return Ok(Vec::new());
        };
        let BankChunk::MediaIndex(entries) = &self.chunks[index_position] else {
            unreachable!()
        };
        let Some(BankChunk::MediaData(data)) = self.chunks.get(index_position + 1) else {
            return Err(BnkError::MissingChunk {
                chunk: ChunkId::DATA.0,
                context: "DIDX must be immediately followed by DATA",
            });
        };

        let mut media = Vec::with_capacity(entries.len());
        for entry in entries {
            if entry.id == 0 && entry.offset == 1 && entry.size == 0 {
                media.push(EmbeddedMedia {
                    id: entry.id,
                    data: &[],
                });
                continue;
            }
            let begin = entry.offset as usize;
            let end = begin
                .checked_add(entry.size as usize)
                .ok_or(BnkError::IntegerOverflow {
                    context: "DIDX media range",
                })?;
            let payload = data.get(begin..end).ok_or(BnkError::MediaOutOfBounds {
                id: entry.id,
                offset: entry.offset,
                size: entry.size,
                data_len: data.len(),
            })?;
            media.push(EmbeddedMedia {
                id: entry.id,
                data: payload,
            });
        }
        Ok(media)
    }

    /// Replace or create the first DIDX/DATA pair.
    ///
    /// Payloads are aligned relative to the beginning of DATA. Existing banks
    /// retain their exact original padding until this method is called.
    pub fn set_embedded_media(
        &mut self,
        media: impl IntoIterator<Item = OwnedEmbeddedMedia>,
        alignment: u32,
    ) -> Result<()> {
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(BnkError::invalid(
                "media alignment",
                0,
                format!("expected a non-zero power of two, found {alignment}"),
            ));
        }
        let media: Vec<_> = media.into_iter().collect();
        let mut entries = Vec::with_capacity(media.len());
        let mut data = Vec::new();
        for item in media {
            if item.id == 0 {
                if !item.data.is_empty() {
                    return Err(BnkError::invalid(
                        "media identifier zero",
                        0,
                        "the reserved media entry must have an empty payload",
                    ));
                }
                entries.push(MediaIndexEntry {
                    id: 0,
                    offset: 1,
                    size: 0,
                });
                continue;
            }

            let remainder = data.len() % alignment as usize;
            if remainder != 0 {
                data.resize(data.len() + alignment as usize - remainder, 0);
            }
            let offset = u32::try_from(data.len()).map_err(|_| BnkError::LimitExceeded {
                resource: "DATA offset",
                requested: data.len() as u64,
                limit: u32::MAX as u64,
            })?;
            let size = u32::try_from(item.data.len()).map_err(|_| BnkError::LimitExceeded {
                resource: "media bytes",
                requested: item.data.len() as u64,
                limit: u32::MAX as u64,
            })?;
            data.extend_from_slice(&item.data);
            entries.push(MediaIndexEntry {
                id: item.id,
                offset,
                size,
            });
        }

        if let Some(index_position) = self
            .chunks
            .iter()
            .position(|chunk| matches!(chunk, BankChunk::MediaIndex(_)))
        {
            self.chunks[index_position] = BankChunk::MediaIndex(entries);
            if matches!(
                self.chunks.get(index_position + 1),
                Some(BankChunk::MediaData(_))
            ) {
                self.chunks[index_position + 1] = BankChunk::MediaData(data);
            } else {
                self.chunks
                    .insert(index_position + 1, BankChunk::MediaData(data));
            }
        } else {
            self.chunks.insert(0, BankChunk::MediaData(data));
            self.chunks.insert(0, BankChunk::MediaIndex(entries));
        }
        Ok(())
    }
}
