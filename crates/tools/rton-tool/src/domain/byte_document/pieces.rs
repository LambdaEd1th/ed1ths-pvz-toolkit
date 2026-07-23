use std::sync::Arc;

use super::ByteSource;
#[cfg(not(target_arch = "wasm32"))]
use super::MappedByteSource;
use crate::domain::HexEdit;

#[derive(Debug, Clone)]
pub(crate) struct PieceBytes {
    pub(crate) pieces: Vec<BytePiece>,
    pub(crate) starts: Vec<usize>,
    pub(crate) len: usize,
}

impl PieceBytes {
    pub(crate) fn from_source(source: &ByteSource) -> Self {
        Self::from_pieces(source.pieces())
    }

    pub(crate) fn from_pieces(pieces: Vec<BytePiece>) -> Self {
        let mut pieces = merge_adjacent_pieces(pieces);
        let mut starts = Vec::with_capacity(pieces.len());
        let mut len = 0usize;
        pieces.retain(|piece| {
            if piece.len == 0 {
                return false;
            }
            starts.push(len);
            len += piece.len;
            true
        });
        Self {
            pieces,
            starts,
            len,
        }
    }

    pub(super) fn apply_edit(&self, edit: &HexEdit) -> Self {
        let offset = edit.offset.min(self.len);
        let delete_length = edit.delete_length.min(self.len.saturating_sub(offset));
        let (mut pieces, rest) = split_pieces_at(&self.pieces, offset);
        let (_, after_deleted) = split_pieces_at(&rest, delete_length);

        if !edit.insert.is_empty() {
            pieces.push(BytePiece {
                source: BytePieceSource::Memory(Arc::<[u8]>::from(edit.insert.clone())),
                start: 0,
                len: edit.insert.len(),
            });
        }
        pieces.extend(after_deleted);

        Self::from_pieces(pieces)
    }

    pub(super) fn byte_at(&self, offset: usize) -> Option<u8> {
        let index = self.piece_index_at(offset)?;
        self.pieces[index].byte_at(offset - self.starts[index])
    }

    pub(crate) fn range_to_vec(&self, start: usize, end: usize) -> Option<Vec<u8>> {
        if start > end || end > self.len {
            return None;
        }
        if start == end {
            return Some(Vec::new());
        }

        let mut output = Vec::with_capacity(end - start);
        let mut index = self.piece_index_at(start)?;
        while let Some(piece) = self.pieces.get(index) {
            let cursor = self.starts[index];
            let piece_end = cursor + piece.len;
            if cursor >= end {
                break;
            }

            let local_start = start.saturating_sub(cursor).min(piece.len);
            let local_end = end.saturating_sub(cursor).min(piece.len);
            if local_start < local_end {
                piece.extend_range(local_start, local_end, &mut output);
            }
            if piece_end >= end {
                break;
            }
            index += 1;
        }
        Some(output)
    }

    pub(super) fn copy_range_to(&self, start: usize, output: &mut [u8]) -> Option<usize> {
        if output.is_empty() {
            return Some(0);
        }
        if start > self.len {
            return None;
        }
        if start == self.len {
            return Some(0);
        }

        let end = start + output.len().min(self.len - start);
        let mut copied = 0usize;
        let mut index = self.piece_index_at(start)?;
        while let Some(piece) = self.pieces.get(index) {
            let cursor = self.starts[index];
            let piece_end = cursor + piece.len;
            if cursor >= end {
                break;
            }

            let local_start = start.saturating_sub(cursor).min(piece.len);
            let local_end = end.saturating_sub(cursor).min(piece.len);
            if local_start < local_end {
                let len = local_end - local_start;
                piece.copy_range_to(local_start, local_end, &mut output[copied..copied + len]);
                copied += len;
            }
            if piece_end >= end {
                break;
            }
            index += 1;
        }
        Some(copied)
    }

    pub(super) fn to_vec(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.len);
        for piece in &self.pieces {
            piece.extend_range(0, piece.len, &mut output);
        }
        output
    }

    pub(crate) fn piece_index_at(&self, offset: usize) -> Option<usize> {
        if offset >= self.len {
            return None;
        }
        self.starts
            .partition_point(|start| *start <= offset)
            .checked_sub(1)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BytePiece {
    pub(crate) source: BytePieceSource,
    pub(crate) start: usize,
    pub(crate) len: usize,
}

impl BytePiece {
    fn slice(&self, local_start: usize, len: usize) -> Self {
        Self {
            source: self.source.clone(),
            start: self.start + local_start,
            len,
        }
    }

    pub(super) fn byte_at(&self, local_offset: usize) -> Option<u8> {
        (local_offset < self.len)
            .then(|| self.source.byte_at(self.start + local_offset))
            .flatten()
    }

    fn extend_range(&self, local_start: usize, local_end: usize, output: &mut Vec<u8>) {
        if local_start >= local_end || local_end > self.len {
            return;
        }
        self.source
            .extend_range(self.start + local_start, self.start + local_end, output);
    }

    pub(super) fn copy_range_to(&self, local_start: usize, local_end: usize, output: &mut [u8]) {
        if local_start >= local_end || local_end > self.len {
            return;
        }
        self.source
            .copy_range_to(self.start + local_start, self.start + local_end, output);
    }

    fn can_merge_with(&self, next: &Self) -> bool {
        self.start.checked_add(self.len) == Some(next.start)
            && self.source.same_identity(&next.source)
    }

    fn merge_with(&mut self, next: &Self) {
        self.len += next.len;
    }
}

#[derive(Debug, Clone)]
pub(crate) enum BytePieceSource {
    Memory(Arc<[u8]>),
    #[cfg(not(target_arch = "wasm32"))]
    FileMap(Arc<MappedByteSource>),
}

impl BytePieceSource {
    pub(super) fn byte_at(&self, offset: usize) -> Option<u8> {
        match self {
            Self::Memory(bytes) => bytes.get(offset).copied(),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => source.bytes.get(offset).copied(),
        }
    }

    fn extend_range(&self, start: usize, end: usize, output: &mut Vec<u8>) {
        match self {
            Self::Memory(bytes) => output.extend_from_slice(&bytes[start..end]),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => output.extend_from_slice(&source.bytes[start..end]),
        }
    }

    pub(super) fn copy_range_to(&self, start: usize, end: usize, output: &mut [u8]) {
        match self {
            Self::Memory(bytes) => output.copy_from_slice(&bytes[start..end]),
            #[cfg(not(target_arch = "wasm32"))]
            Self::FileMap(source) => output.copy_from_slice(&source.bytes[start..end]),
        }
    }

    fn same_identity(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Memory(left), Self::Memory(right)) => Arc::ptr_eq(left, right),
            #[cfg(not(target_arch = "wasm32"))]
            (Self::FileMap(left), Self::FileMap(right)) => Arc::ptr_eq(left, right),
            #[cfg(not(target_arch = "wasm32"))]
            (Self::Memory(_), Self::FileMap(_)) | (Self::FileMap(_), Self::Memory(_)) => false,
        }
    }
}

fn merge_adjacent_pieces(pieces: Vec<BytePiece>) -> Vec<BytePiece> {
    let mut merged = Vec::<BytePiece>::new();
    for piece in pieces {
        if piece.len == 0 {
            continue;
        }
        if let Some(previous) = merged.last_mut()
            && previous.can_merge_with(&piece)
        {
            previous.merge_with(&piece);
            continue;
        }
        merged.push(piece);
    }
    merged
}

fn split_pieces_at(pieces: &[BytePiece], offset: usize) -> (Vec<BytePiece>, Vec<BytePiece>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut cursor = 0usize;

    for piece in pieces {
        let piece_end = cursor + piece.len;
        if piece_end <= offset {
            left.push(piece.clone());
        } else if cursor >= offset {
            right.push(piece.clone());
        } else {
            let split = offset - cursor;
            if split > 0 {
                left.push(piece.slice(0, split));
            }
            if split < piece.len {
                right.push(piece.slice(split, piece.len - split));
            }
        }
        cursor = piece_end;
    }

    (left, right)
}
