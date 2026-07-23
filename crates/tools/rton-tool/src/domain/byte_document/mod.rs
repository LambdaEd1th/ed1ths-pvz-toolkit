mod document;
mod mmap;
mod pieces;
mod read;
#[cfg(any(not(target_arch = "wasm32"), test))]
mod reader;
mod source;

pub(crate) use document::ByteDocument;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use mmap::MappedByteSource;
#[cfg(test)]
pub(crate) use pieces::{BytePiece, BytePieceSource, PieceBytes};
#[cfg(not(test))]
pub(crate) use pieces::{BytePiece, BytePieceSource, PieceBytes};
pub(crate) use read::ByteRead;
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) use reader::ByteDocumentReader;
pub(crate) use source::ByteSource;
