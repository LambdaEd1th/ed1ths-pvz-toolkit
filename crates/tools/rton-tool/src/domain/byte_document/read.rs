use super::ByteDocument;

pub(crate) trait ByteRead: rton_editor_core::ByteRead {}

impl<T: rton_editor_core::ByteRead + ?Sized> ByteRead for T {}

impl rton_editor_core::ByteRead for ByteDocument {
    fn len(&self) -> usize {
        self.len()
    }

    fn byte_at(&self, offset: usize) -> Option<u8> {
        self.byte_at(offset)
    }

    fn range_to_vec(&self, start: usize, end: usize) -> Option<Vec<u8>> {
        self.range_to_vec(start, end)
    }

    fn copy_range_to(&self, start: usize, output: &mut [u8]) -> Option<usize> {
        self.copy_range_to(start, output)
    }
}
