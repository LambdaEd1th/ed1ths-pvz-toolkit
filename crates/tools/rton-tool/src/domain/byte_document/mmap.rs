use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

use super::ByteSource;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub(crate) struct MappedByteSource {
    pub(super) bytes: memmap2::Mmap,
}

#[cfg(not(target_arch = "wasm32"))]
impl MappedByteSource {
    fn open(path: impl AsRef<Path>) -> Result<Option<Self>, String> {
        let path = path.as_ref().to_path_buf();
        let file =
            std::fs::File::open(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        if file
            .metadata()
            .map_err(|error| format!("{}: {error}", path.display()))?
            .len()
            == 0
        {
            return Ok(None);
        }

        let bytes = {
            // Read-only mmap gives the hex editor a borrowed byte slice without copying the file.
            unsafe { memmap2::MmapOptions::new().map(&file) }
                .map_err(|error| format!("{}: {error}", path.display()))?
        };
        Ok(Some(Self { bytes }))
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn from_file_path_source(path: impl AsRef<Path>) -> Result<ByteSource, String> {
    Ok(match MappedByteSource::open(path)? {
        Some(source) => ByteSource::FileMap(Arc::new(source)),
        None => ByteSource::Memory(Arc::<[u8]>::from(Vec::new())),
    })
}

#[cfg(target_arch = "wasm32")]
pub(super) fn from_file_path_source(_path: impl AsRef<Path>) -> Result<ByteSource, String> {
    Err("native file mapping is unavailable on web".to_string())
}
