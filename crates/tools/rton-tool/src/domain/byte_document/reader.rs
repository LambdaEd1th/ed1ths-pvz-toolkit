use std::io::{Read, Seek, SeekFrom};

use super::ByteDocument;

#[derive(Debug, Clone)]
pub(crate) struct ByteDocumentReader {
    pub(super) document: ByteDocument,
    pub(super) position: u64,
}

impl Read for ByteDocumentReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }

        let start = usize::try_from(self.position).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "byte document reader position exceeds usize",
            )
        })?;
        if start >= self.document.len() {
            return Ok(0);
        }

        let Some(read) = self.document.copy_range_to(start, output) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "byte document range is unavailable",
            ));
        };
        self.position = self.position.saturating_add(read as u64);
        Ok(read)
    }
}

impl Seek for ByteDocumentReader {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        let next = match position {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::End(offset) => self.document.len() as i128 + i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
        };

        if next < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "cannot seek before start of byte document",
            ));
        }

        self.position = u64::try_from(next).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "byte document reader position exceeds u64",
            )
        })?;
        Ok(self.position)
    }
}
