use crate::domain::{ArchiveChannelOrderMode, ArchiveDocument, PacketDocument, PacketRecord};
use rsb_archive::{Rsb, RsgHeader, RsgInfo, unpack_rsg};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::sync::Arc;

#[derive(Clone)]
pub enum ArchiveSource {
    #[cfg(not(target_arch = "wasm32"))]
    Native(Arc<std::path::PathBuf>),
    Memory(Arc<Vec<u8>>),
}

impl ArchiveSource {
    pub fn identity(&self) -> usize {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Native(path) => Arc::as_ptr(path) as usize,
            Self::Memory(bytes) => Arc::as_ptr(bytes) as usize,
        }
    }

    fn packet_records(&self, infos: Vec<RsgInfo>) -> Result<Vec<PacketRecord>, String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Native(path) => {
                let mut reader = std::fs::File::open(path.as_ref())
                    .map_err(|error| format!("无法读取 {}：{error}", path.as_ref().display()))?;
                read_packet_records(&mut reader, infos)
            }
            Self::Memory(bytes) => {
                let mut reader = Cursor::new(bytes.as_slice());
                read_packet_records(&mut reader, infos)
            }
        }
    }

    pub fn read_range(&self, offset: u64, length: usize) -> Result<Vec<u8>, String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Native(path) => {
                let mut reader = std::fs::File::open(path.as_ref())
                    .map_err(|error| format!("无法读取 {}：{error}", path.as_ref().display()))?;
                reader
                    .seek(SeekFrom::Start(offset))
                    .map_err(|error| error.to_string())?;
                let mut output = vec![0; length];
                reader
                    .read_exact(&mut output)
                    .map_err(|error| error.to_string())?;
                Ok(output)
            }
            Self::Memory(bytes) => {
                let start =
                    usize::try_from(offset).map_err(|_| "归档偏移无法放入内存地址".to_string())?;
                let end = start
                    .checked_add(length)
                    .ok_or_else(|| "归档范围溢出".to_string())?;
                bytes
                    .get(start..end)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| "归档条目超出文件范围".to_string())
            }
        }
    }

    pub fn read_all(&self) -> Result<Vec<u8>, String> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Native(path) => std::fs::read(path.as_ref())
                .map_err(|error| format!("无法读取 {}：{error}", path.as_ref().display())),
            Self::Memory(bytes) => Ok(bytes.as_ref().clone()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn native_path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Native(path) => Some(path.as_ref()),
            Self::Memory(_) => None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open_native(path: std::path::PathBuf) -> Result<ArchiveDocument, String> {
    let display_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string());
    let byte_len = std::fs::metadata(&path)
        .map_err(|error| error.to_string())?
        .len();
    let reader = std::fs::File::open(&path).map_err(|error| error.to_string())?;
    load_archive(
        reader,
        display_name,
        byte_len,
        ArchiveSource::Native(Arc::new(path)),
    )
}

pub fn open_memory(display_name: String, bytes: Vec<u8>) -> Result<ArchiveDocument, String> {
    let bytes = Arc::new(bytes);
    let reader = Cursor::new(bytes.as_slice());
    load_archive(
        reader,
        display_name,
        bytes.len() as u64,
        ArchiveSource::Memory(bytes.clone()),
    )
}

fn load_archive(
    reader: impl Read + Seek,
    display_name: String,
    byte_len: u64,
    source: ArchiveSource,
) -> Result<ArchiveDocument, String> {
    let mut archive = Rsb::open(reader).map_err(|error| error.to_string())?;
    let header = archive.header.clone();
    let mut warnings = Vec::new();
    let file_index = match archive.read_file_list() {
        Ok(resources) => resources,
        Err(error) => {
            warnings.push(format!("资源路径索引未能读取：{error}"));
            Vec::new()
        }
    };
    let infos = archive.read_rsg_info().map_err(|error| error.to_string())?;
    let ptx_infos = archive.read_ptx_info().map_err(|error| error.to_string())?;
    let packets = source.packet_records(infos)?;

    Ok(ArchiveDocument {
        display_name,
        byte_len,
        header,
        resource_count: file_index.len(),
        file_index: Arc::new(file_index),
        packets: Arc::new(packets),
        ptx_infos: Arc::new(ptx_infos),
        warnings: Arc::new(warnings),
        channel_order_mode: ArchiveChannelOrderMode::Auto,
        source,
    })
}

fn read_packet_records(
    reader: &mut (impl Read + Seek),
    infos: Vec<RsgInfo>,
) -> Result<Vec<PacketRecord>, String> {
    let mut records = Vec::with_capacity(infos.len());
    for (index, info) in infos.into_iter().enumerate() {
        let result = if info.rsg_length < 80 {
            Err("RSG 包短于固定头部".to_string())
        } else {
            reader
                .seek(SeekFrom::Start(u64::from(info.rsg_offset)))
                .map_err(|error| error.to_string())
                .and_then(|_| RsgHeader::read_from(reader).map_err(|error| error.to_string()))
        };
        let (header, error) = match result {
            Ok(header) => (Some(header), None),
            Err(error) => (None, Some(error)),
        };
        records.push(PacketRecord {
            index,
            info,
            header,
            error,
        });
    }
    Ok(records)
}

impl ArchiveDocument {
    pub fn source_bytes(&self) -> Result<Vec<u8>, String> {
        self.source.read_all()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn source_path(&self) -> Option<&std::path::Path> {
        self.source.native_path()
    }

    pub fn read_packet_raw(&self, packet_index: usize) -> Result<(PacketRecord, Vec<u8>), String> {
        let record = self
            .packets
            .get(packet_index)
            .cloned()
            .ok_or_else(|| "RSG 包索引已失效".to_string())?;
        let raw = self.source.read_range(
            u64::from(record.info.rsg_offset),
            record.info.rsg_length as usize,
        )?;
        Ok((record, raw))
    }

    pub fn load_packet(&self, packet_index: usize) -> Result<PacketDocument, String> {
        let (record, raw) = self.read_packet_raw(packet_index)?;
        let files =
            unpack_rsg(&mut Cursor::new(&raw)).map_err(|error| format!("RSG 解压失败：{error}"))?;
        Ok(PacketDocument::new(record, raw, files))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::open_native;
    use crate::preview::{PreviewQuality, prepare_png_export, prepare_preview};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    fn real_sample_path() -> Option<PathBuf> {
        if let Some(path) = std::env::var_os("RSB_ARCHIVE_REAL_SAMPLE").map(PathBuf::from) {
            return Some(path);
        }
        let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../pvz2/com.popcap.ios.PvZ2.app/main.rsb");
        sample.exists().then_some(sample)
    }

    fn pvz2_toolkit_sample_path(name: &str) -> Option<PathBuf> {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../pvz2-toolkit/test_data")
            .join(name);
        sample.exists().then_some(sample)
    }

    #[test]
    fn lazily_opens_and_enters_a_packet_from_a_real_archive() -> Result<(), String> {
        let Some(path) = real_sample_path() else {
            eprintln!("skipping RSB app loader test; set RSB_ARCHIVE_REAL_SAMPLE");
            return Ok(());
        };

        let archive = open_native(path)?;
        assert!(!archive.packets.is_empty());
        assert_eq!(
            archive.packets.len(),
            usize::try_from(archive.header.rsg_number).map_err(|error| error.to_string())?
        );

        let packet_index = archive
            .packets
            .iter()
            .filter(|packet| packet.info.rsg_length < 1024 * 1024)
            .find_map(|packet| archive.load_packet(packet.index).ok().map(|_| packet.index))
            .ok_or_else(|| "real RSB has no small readable packet".to_string())?;
        let packet = archive.load_packet(packet_index)?;
        assert!(!packet.files.is_empty());
        assert_eq!(packet.record.index, packet_index);
        Ok(())
    }

    #[test]
    fn decodes_a_ptx_preview_from_a_real_archive() -> Result<(), String> {
        let Some(path) = real_sample_path() else {
            eprintln!("skipping PTX preview test; set RSB_ARCHIVE_REAL_SAMPLE");
            return Ok(());
        };

        let archive = Arc::new(open_native(path)?);
        for record in archive
            .packets
            .iter()
            .filter(|record| record.info.ptx_number > 0 && record.info.rsg_length < 8 * 1024 * 1024)
        {
            let Ok(packet) = archive.load_packet(record.index) else {
                continue;
            };
            let packet = Arc::new(packet);
            for (file_index, file) in packet.files.iter().enumerate() {
                if !file.path.to_ascii_lowercase().ends_with(".ptx") {
                    continue;
                }
                let Ok(prepared) = prepare_preview(
                    &archive,
                    Arc::clone(&packet),
                    file_index,
                    PreviewQuality::Thumbnail,
                ) else {
                    continue;
                };
                let preview =
                    rsb_preview_worker::perform_borrowed(&file.data, &prepared.spec, || false)
                        .map_err(|error| error.to_string())?;
                assert!(preview.png.starts_with(b"\x89PNG\r\n\x1a\n"));
                assert!(preview.width > 0);
                assert!(preview.height > 0);
                assert!(preview.width <= 512);
                assert!(preview.height <= 512);
                eprintln!(
                    "real PTX preview: packet={}; file={}; {}x{}; format={}",
                    record.info.name,
                    file.path,
                    preview.source_width,
                    preview.source_height,
                    prepared.spec.format
                );
                return Ok(());
            }
        }

        Err("real RSB contains no decodable PTX preview".to_string())
    }

    #[test]
    fn decodes_pvz2cn_palette_ptx_from_zombie_archive() -> Result<(), String> {
        let Some(path) = pvz2_toolkit_sample_path("zombie1.rsb") else {
            eprintln!("skipping PvZ2CN palette preview test; zombie1.rsb is unavailable");
            return Ok(());
        };

        let archive = Arc::new(open_native(path)?);
        for record in archive
            .packets
            .iter()
            .filter(|record| record.info.ptx_number > 0)
        {
            let Ok(packet) = archive.load_packet(record.index) else {
                continue;
            };
            let packet = Arc::new(packet);
            for (file_index, file) in packet.files.iter().enumerate() {
                let Some(metadata) = archive.texture_metadata(&packet.record, file) else {
                    continue;
                };
                if metadata.info.format != 147 || metadata.info.alpha_size.unwrap_or_default() <= 0
                {
                    continue;
                }
                let prepared = prepare_preview(
                    &archive,
                    Arc::clone(&packet),
                    file_index,
                    PreviewQuality::Thumbnail,
                )?;
                let preview =
                    rsb_preview_worker::perform_borrowed(&file.data, &prepared.spec, || false)
                        .map_err(|error| error.to_string())?;
                assert!(preview.png.starts_with(b"\x89PNG\r\n\x1a\n"));
                assert!(preview.width > 0);
                assert!(preview.height > 0);
                let export = prepare_png_export(&archive, Arc::clone(&packet), file_index)?;
                let exported =
                    rsb_preview_worker::perform_borrowed(&file.data, &export.spec, || false)
                        .map_err(|error| error.to_string())?;
                assert!(export.output_name.ends_with(".png"));
                assert!(exported.png.starts_with(b"\x89PNG\r\n\x1a\n"));
                assert_eq!(exported.width, exported.source_width);
                assert_eq!(exported.height, exported.source_height);
                eprintln!(
                    "PvZ2CN palette PTX PNG export: packet={}; file={}; {}x{}; bytes={}",
                    record.info.name,
                    file.path,
                    exported.source_width,
                    exported.source_height,
                    file.data.len(),
                );
                return Ok(());
            }
        }

        Err("zombie1.rsb contains no ETC1 palette PTX".to_string())
    }
}
