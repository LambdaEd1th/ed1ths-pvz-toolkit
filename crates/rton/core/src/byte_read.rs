pub trait ByteRead {
    fn len(&self) -> usize;
    fn byte_at(&self, offset: usize) -> Option<u8>;
    fn range_to_vec(&self, start: usize, end: usize) -> Option<Vec<u8>>;

    fn copy_range_to(&self, start: usize, output: &mut [u8]) -> Option<usize> {
        let end = start.checked_add(output.len())?;
        if end > self.len() {
            return None;
        }
        for (index, slot) in output.iter_mut().enumerate() {
            *slot = self.byte_at(start + index)?;
        }
        Some(output.len())
    }

    fn read_array<const N: usize>(&self, offset: usize) -> Option<[u8; N]> {
        let mut output = [0_u8; N];
        self.copy_range_to(offset, &mut output)?;
        Some(output)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn starts_with_bytes(&self, pattern: &[u8]) -> bool {
        self.matches_bytes_at(0, pattern)
    }

    fn ends_with_bytes(&self, pattern: &[u8]) -> bool {
        if self.len() < pattern.len() {
            return false;
        }
        let start = self.len() - pattern.len();
        self.matches_bytes_at(start, pattern)
    }

    fn matches_bytes_at(&self, offset: usize, pattern: &[u8]) -> bool {
        if offset
            .checked_add(pattern.len())
            .is_none_or(|end| end > self.len())
        {
            return false;
        }
        pattern
            .iter()
            .enumerate()
            .all(|(index, byte)| self.byte_at(offset + index) == Some(*byte))
    }
}

impl ByteRead for [u8] {
    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    fn byte_at(&self, offset: usize) -> Option<u8> {
        self.get(offset).copied()
    }

    fn range_to_vec(&self, start: usize, end: usize) -> Option<Vec<u8>> {
        self.get(start..end).map(|bytes| bytes.to_vec())
    }
}

impl ByteRead for Vec<u8> {
    fn len(&self) -> usize {
        self.as_slice().len()
    }

    fn byte_at(&self, offset: usize) -> Option<u8> {
        self.as_slice().get(offset).copied()
    }

    fn range_to_vec(&self, start: usize, end: usize) -> Option<Vec<u8>> {
        self.as_slice().get(start..end).map(|bytes| bytes.to_vec())
    }
}

impl<const N: usize> ByteRead for [u8; N] {
    fn len(&self) -> usize {
        N
    }

    fn byte_at(&self, offset: usize) -> Option<u8> {
        self.as_slice().get(offset).copied()
    }

    fn range_to_vec(&self, start: usize, end: usize) -> Option<Vec<u8>> {
        self.as_slice().get(start..end).map(|bytes| bytes.to_vec())
    }
}
