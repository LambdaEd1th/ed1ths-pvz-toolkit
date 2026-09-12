use rsb_archive::{RsbHeader, RsbPtxInfo, RsgHeader, RsgInfo, UnpackedFile};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::loader::ArchiveSource;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ArchiveChannelOrderMode {
    #[default]
    Auto,
    Rgba,
    Apple,
}

#[derive(Clone)]
pub struct ArchiveDocument {
    pub display_name: String,
    pub byte_len: u64,
    pub header: RsbHeader,
    pub resource_count: usize,
    pub file_index: Arc<Vec<rsb_archive::FileListInfo>>,
    pub packets: Arc<Vec<PacketRecord>>,
    pub ptx_infos: Arc<Vec<RsbPtxInfo>>,
    pub warnings: Arc<Vec<String>>,
    pub channel_order_mode: ArchiveChannelOrderMode,
    pub(crate) source: ArchiveSource,
}

impl PartialEq for ArchiveDocument {
    fn eq(&self, other: &Self) -> bool {
        self.display_name == other.display_name
            && self.byte_len == other.byte_len
            && Arc::ptr_eq(&self.packets, &other.packets)
            && Arc::ptr_eq(&self.ptx_infos, &other.ptx_infos)
            && self.channel_order_mode == other.channel_order_mode
    }
}

#[derive(Clone)]
pub struct PacketRecord {
    pub index: usize,
    pub info: RsgInfo,
    pub header: Option<RsgHeader>,
    pub error: Option<String>,
}

impl PartialEq for PacketRecord {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.info.name == other.info.name
            && self.info.rsg_offset == other.info.rsg_offset
            && self.info.rsg_length == other.info.rsg_length
            && self.info.pool_index == other.info.pool_index
            && self.info.ptx_number == other.info.ptx_number
            && self.info.ptx_before_number == other.info.ptx_before_number
            && self.info.packet_head_info == other.info.packet_head_info
            && self.header.as_ref().map(packet_header_identity)
                == other.header.as_ref().map(packet_header_identity)
            && self.error == other.error
    }
}

fn packet_header_identity(
    header: &RsgHeader,
) -> (u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32) {
    (
        header.version,
        header.flags,
        header.file_offset,
        header.part0_offset,
        header.part0_zlib,
        header.part0_size,
        header.part1_offset,
        header.part1_zlib,
        header.part1_size,
        header.file_list_length,
        header.file_list_offset,
    )
}

impl Eq for PacketRecord {}

impl PacketRecord {
    pub fn stored_data_size(&self) -> u64 {
        self.header
            .as_ref()
            .map(|header| u64::from(header.part0_zlib) + u64::from(header.part1_zlib))
            .unwrap_or_default()
    }

    pub fn unpacked_data_size(&self) -> u64 {
        self.header
            .as_ref()
            .map(|header| u64::from(header.part0_size) + u64::from(header.part1_size))
            .unwrap_or_default()
    }

    pub fn compression_label(&self) -> &'static str {
        match self.header.as_ref().map(|header| header.flags) {
            Some(0) => "Raw",
            Some(1) => "Part 1 · zlib",
            Some(2) => "Part 0 · zlib",
            Some(3) => "Part 0 + Part 1 · zlib",
            _ => "Unknown",
        }
    }

    pub fn ratio_label(&self) -> String {
        let stored = self.stored_data_size();
        let unpacked = self.unpacked_data_size();
        if stored == 0 || unpacked == 0 {
            return "—".into();
        }
        let ratio = (1.0 - stored as f64 / unpacked as f64) * 100.0;
        format!("{ratio:.1}%")
    }
}

#[derive(Clone)]
pub struct PacketDocument {
    pub record: PacketRecord,
    pub raw: Arc<Vec<u8>>,
    pub files: Arc<Vec<UnpackedFile>>,
    pub directory_index: Arc<PacketDirectoryIndex>,
}

impl PartialEq for PacketDocument {
    fn eq(&self, other: &Self) -> bool {
        self.record == other.record
            && Arc::ptr_eq(&self.raw, &other.raw)
            && Arc::ptr_eq(&self.files, &other.files)
    }
}

impl PacketDocument {
    pub fn new(record: PacketRecord, raw: Vec<u8>, files: Vec<UnpackedFile>) -> Self {
        let directory_index = Arc::new(PacketDirectoryIndex::build(&files));
        Self {
            record,
            raw: Arc::new(raw),
            files: Arc::new(files),
            directory_index,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TextureMetadata {
    pub global_index: usize,
    pub width: u32,
    pub height: u32,
    pub info: RsbPtxInfo,
}

impl ArchiveDocument {
    pub fn identity(&self) -> usize {
        self.source.identity()
    }

    pub fn texture_metadata(
        &self,
        packet: &PacketRecord,
        file: &UnpackedFile,
    ) -> Option<TextureMetadata> {
        let local_index = usize::try_from(file.part1_info.as_ref()?.id).ok()?;
        let global_index = usize::try_from(packet.info.ptx_before_number)
            .ok()?
            .checked_add(local_index)?;
        let info = self.ptx_infos.get(global_index)?.clone();
        let packet_dimensions = file
            .part1_info
            .as_ref()
            .filter(|value| value.width > 0 && value.height > 0);
        let width = packet_dimensions
            .map(|value| value.width)
            .or_else(|| u32::try_from(info.width).ok())
            .filter(|value| *value > 0)?;
        let height = packet_dimensions
            .map(|value| value.height)
            .or_else(|| u32::try_from(info.height).ok())
            .filter(|value| *value > 0)?;
        Some(TextureMetadata {
            global_index,
            width,
            height,
            info,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum BrowserLocation {
    #[default]
    Archive,
    Packet {
        packet_index: usize,
        directory: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowSelection {
    Packet(usize),
    Directory(Vec<String>),
    File(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserItem {
    Directory {
        name: String,
        path: Vec<String>,
        file_count: usize,
        byte_len: u64,
    },
    File {
        name: String,
        file_index: usize,
    },
}

impl BrowserItem {
    pub fn name(&self) -> &str {
        match self {
            Self::Directory { name, .. } | Self::File { name, .. } => name,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct PacketDirectoryNode {
    directories: BTreeMap<String, Vec<String>>,
    direct_files: Vec<(String, usize)>,
    descendant_files: Vec<usize>,
    byte_len: u64,
}

/// Immutable directory lookup built once when an RSG is unpacked.
///
/// The UI previously split and scanned every file path on every render. Keeping
/// the hierarchy next to the packet turns directory navigation, status counts,
/// and recursive selection into indexed lookups.
#[derive(Clone, Debug, Default)]
pub struct PacketDirectoryIndex {
    nodes: BTreeMap<Vec<String>, PacketDirectoryNode>,
}

impl PacketDirectoryIndex {
    pub fn build(files: &[UnpackedFile]) -> Self {
        let mut nodes = BTreeMap::<Vec<String>, PacketDirectoryNode>::new();
        nodes.entry(Vec::new()).or_default();

        for (file_index, file) in files.iter().enumerate() {
            let components = archive_path_components(&file.path);
            if components.is_empty() {
                continue;
            }
            let byte_len = file.data.len() as u64;
            for depth in 0..components.len() {
                let path = components[..depth].to_vec();
                let node = nodes.entry(path).or_default();
                node.descendant_files.push(file_index);
                node.byte_len = node.byte_len.saturating_add(byte_len);
                if depth + 1 == components.len() {
                    node.direct_files
                        .push((components[depth].clone(), file_index));
                } else {
                    node.directories
                        .entry(components[depth].clone())
                        .or_insert_with(|| components[..=depth].to_vec());
                }
            }
        }

        for node in nodes.values_mut() {
            node.direct_files
                .sort_by_cached_key(|(name, _)| name.to_ascii_lowercase());
        }
        Self { nodes }
    }

    pub fn items_with_virtual<'a>(
        &self,
        directory: &[String],
        virtual_directories: impl IntoIterator<Item = &'a Vec<String>>,
    ) -> Vec<BrowserItem> {
        let mut directories = self
            .nodes
            .get(directory)
            .map(|node| node.directories.clone())
            .unwrap_or_default();

        for path in virtual_directories {
            if path.len() <= directory.len() || !path.starts_with(directory) {
                continue;
            }
            directories
                .entry(path[directory.len()].clone())
                .or_insert_with(|| path[..=directory.len()].to_vec());
        }

        let mut directory_items = directories
            .into_iter()
            .map(|(name, path)| {
                let (file_count, byte_len) = self
                    .nodes
                    .get(&path)
                    .map(|node| (node.descendant_files.len(), node.byte_len))
                    .unwrap_or_default();
                BrowserItem::Directory {
                    name,
                    path,
                    file_count,
                    byte_len,
                }
            })
            .collect::<Vec<_>>();
        directory_items.sort_by_cached_key(|item| item.name().to_ascii_lowercase());

        if let Some(node) = self.nodes.get(directory) {
            directory_items.extend(node.direct_files.iter().map(|(name, file_index)| {
                BrowserItem::File {
                    name: name.clone(),
                    file_index: *file_index,
                }
            }));
        }
        directory_items
    }

    pub fn item_count_with_virtual(
        &self,
        directory: &[String],
        virtual_directories: &BTreeSet<Vec<String>>,
    ) -> usize {
        let direct_file_count = self
            .nodes
            .get(directory)
            .map_or(0, |node| node.direct_files.len());
        let mut directories = self
            .nodes
            .get(directory)
            .map(|node| node.directories.keys().cloned().collect::<BTreeSet<_>>())
            .unwrap_or_default();
        directories.extend(
            virtual_directories
                .iter()
                .filter(|path| path.len() > directory.len() && path.starts_with(directory))
                .map(|path| path[directory.len()].clone()),
        );
        direct_file_count + directories.len()
    }

    pub fn files_below<'a>(
        &self,
        files: &'a [UnpackedFile],
        directory: &[String],
    ) -> Vec<(usize, &'a UnpackedFile)> {
        self.nodes
            .get(directory)
            .into_iter()
            .flat_map(|node| node.descendant_files.iter().copied())
            .filter_map(|index| files.get(index).map(|file| (index, file)))
            .collect()
    }

    pub fn directory_summary(&self, directory: &[String]) -> (usize, u64) {
        self.nodes
            .get(directory)
            .map(|node| (node.descendant_files.len(), node.byte_len))
            .unwrap_or_default()
    }
}

pub fn archive_path_components(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .filter(|component| *component != "..")
        .map(str::to_string)
        .collect()
}

pub fn safe_archive_path(path: &str, fallback: &str) -> String {
    let components = archive_path_components(path);
    if components.is_empty() {
        fallback.to_string()
    } else {
        components.join("/")
    }
}

pub fn file_kind(path: &str, is_part1: bool) -> &'static str {
    if is_part1 {
        return "Texture";
    }
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "rton" => "RTON",
        "pam" => "PAM",
        "ptx" => "PTX",
        "json" => "JSON",
        "xml" => "XML",
        "wem" | "bnk" => "Audio",
        _ => "File",
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else if value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BrowserItem, PacketDirectoryIndex, PacketRecord, archive_path_components, format_bytes,
        safe_archive_path,
    };
    use rsb_archive::{RSG_MAGIC, RsgHeader, RsgInfo, UnpackedFile};

    fn file(path: &str, size: usize) -> UnpackedFile {
        UnpackedFile {
            path: path.into(),
            data: vec![0; size],
            is_part1: false,
            part1_info: None,
        }
    }

    #[test]
    fn builds_archive_style_directory_rows() {
        let files = vec![
            file("DATA/LEVELS/ONE.RTON", 3),
            file("DATA/LEVELS/TWO.RTON", 5),
            file("DATA/CONFIG.RTON", 7),
        ];
        let index = PacketDirectoryIndex::build(&files);
        let virtual_directories = Vec::new();
        let root = index.items_with_virtual(&[], virtual_directories.iter());
        assert!(matches!(
            &root[0],
            BrowserItem::Directory {
                name,
                file_count: 3,
                byte_len: 15,
                ..
            } if name == "DATA"
        ));

        let data = index.items_with_virtual(&["DATA".into()], virtual_directories.iter());
        assert!(matches!(
            &data[0],
            BrowserItem::Directory {
                name,
                file_count: 2,
                byte_len: 8,
                ..
            } if name == "LEVELS"
        ));
        assert!(matches!(&data[1], BrowserItem::File { name, .. } if name == "CONFIG.RTON"));
    }

    #[test]
    fn merges_virtual_empty_directories_into_directory_rows() {
        let files = vec![file("DATA/CONFIG.RTON", 7)];
        let virtual_directories = [vec!["EMPTY".into()], vec!["DATA".into(), "NESTED".into()]];

        let index = PacketDirectoryIndex::build(&files);
        let root = index.items_with_virtual(&[], virtual_directories.iter());
        assert!(root.iter().any(|item| {
            matches!(
                item,
                BrowserItem::Directory {
                    name,
                    file_count: 0,
                    byte_len: 0,
                    ..
                } if name == "EMPTY"
            )
        }));

        let data = index.items_with_virtual(&["DATA".into()], virtual_directories.iter());
        assert!(data.iter().any(|item| {
            matches!(
                item,
                BrowserItem::Directory {
                    name,
                    file_count: 0,
                    byte_len: 0,
                    ..
                } if name == "NESTED"
            )
        }));
    }

    #[test]
    fn sanitizes_paths_and_formats_sizes() {
        assert_eq!(
            archive_path_components("../DATA\\./FILE.RTON"),
            ["DATA", "FILE.RTON"]
        );
        assert_eq!(safe_archive_path("../../", "entry.bin"), "entry.bin");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(10 * 1024 * 1024), "10.0 MB");
    }

    #[test]
    fn packet_identity_includes_editable_header_properties() {
        let record = |flags| PacketRecord {
            index: 0,
            info: RsgInfo {
                name: "Packet".into(),
                rsg_offset: 0x1000,
                rsg_length: 0x1000,
                pool_index: 0,
                ptx_number: 0,
                ptx_before_number: 0,
                packet_head_info: Some(vec![0; 32]),
            },
            header: Some(RsgHeader {
                magic: RSG_MAGIC,
                version: 4,
                flags,
                ..Default::default()
            }),
            error: None,
        };
        assert!(record(2) != record(3));
    }
}
