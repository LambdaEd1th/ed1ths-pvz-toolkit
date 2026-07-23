use super::primitives::read_rton_varint;
use super::types::RtonVarintInfo;
use crate::ByteRead;

pub(crate) struct RtonCursor<'a, B: ByteRead> {
    bytes: &'a B,
    offset: usize,
}

impl<'a, B: ByteRead> RtonCursor<'a, B> {
    pub(crate) fn new(bytes: &'a B, offset: usize) -> Self {
        Self { bytes, offset }
    }

    pub(crate) fn offset(&self) -> usize {
        self.offset
    }

    pub(crate) fn read_varint(&mut self, message: &str) -> Result<RtonVarintInfo, String> {
        let value = read_rton_varint(self.bytes, self.offset).ok_or_else(|| message.to_string())?;
        self.offset = value.next_offset;
        Ok(value)
    }

    pub(crate) fn read_vec(&mut self, length: usize) -> Result<Vec<u8>, String> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| "RTON length is too large".to_string())?;
        if end > self.bytes.len() {
            return Err("Unexpected end of RTON bytes".to_string());
        }
        let bytes = self
            .bytes
            .range_to_vec(self.offset, end)
            .ok_or_else(|| "Unexpected end of RTON bytes".to_string())?;
        self.offset = end;
        Ok(bytes)
    }
}
