use crate::domain::{
    ArchiveDocument, BrowserItem, PacketDirectoryIndex, PacketRecord, file_kind, format_bytes,
};
use crate::editing::{PacketEdits, RemovedPackets};
use rsb_archive::UnpackedFile;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BrowserSortKey {
    #[default]
    Name,
    Type,
    Size,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum BrowserSortDirection {
    #[default]
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BrowserSort {
    pub(crate) key: BrowserSortKey,
    pub(crate) direction: BrowserSortDirection,
}

impl BrowserSort {
    pub(crate) fn toggled(self, key: BrowserSortKey) -> Self {
        if self.key == key {
            Self {
                key,
                direction: match self.direction {
                    BrowserSortDirection::Ascending => BrowserSortDirection::Descending,
                    BrowserSortDirection::Descending => BrowserSortDirection::Ascending,
                },
            }
        } else {
            Self {
                key,
                direction: BrowserSortDirection::Ascending,
            }
        }
    }

    pub(crate) fn aria_value(self, key: BrowserSortKey) -> &'static str {
        if self.key != key {
            return "none";
        }
        match self.direction {
            BrowserSortDirection::Ascending => "ascending",
            BrowserSortDirection::Descending => "descending",
        }
    }

    pub(crate) fn indicator(self, key: BrowserSortKey) -> &'static str {
        if self.key != key {
            return "↕";
        }
        match self.direction {
            BrowserSortDirection::Ascending => "↑",
            BrowserSortDirection::Descending => "↓",
        }
    }

    fn apply(self, ordering: Ordering) -> Ordering {
        match self.direction {
            BrowserSortDirection::Ascending => ordering,
            BrowserSortDirection::Descending => ordering.reverse(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ArchiveRowSource {
    packets: Arc<Vec<PacketRecord>>,
    edits: PacketEdits,
    removed: RemovedPackets,
}

impl ArchiveRowSource {
    pub(crate) fn new(
        archive: &ArchiveDocument,
        edits: PacketEdits,
        removed: RemovedPackets,
    ) -> Self {
        Self {
            packets: archive.packets.clone(),
            edits,
            removed,
        }
    }

    pub(crate) fn build_rows(&self) -> Arc<Vec<ArchiveTableRow>> {
        Arc::new(
            visible_packet_records_from_parts(&self.packets, &self.edits, &self.removed)
                .into_iter()
                .map(ArchiveTableRow::from)
                .collect(),
        )
    }
}

impl PartialEq for ArchiveRowSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.packets, &other.packets)
            && self.edits == other.edits
            && self.removed == other.removed
    }
}

fn visible_packet_records_from_parts(
    packets: &Arc<Vec<PacketRecord>>,
    edits: &PacketEdits,
    removed: &RemovedPackets,
) -> Vec<PacketRecord> {
    let original_count = packets.len();
    let mut records = packets
        .iter()
        .filter(|record| !removed.contains(&record.index))
        .map(|record| {
            edits
                .get(&record.index)
                .map(|edit| edit.document.record.clone())
                .unwrap_or_else(|| record.clone())
        })
        .collect::<Vec<_>>();
    records.extend(
        edits
            .range(original_count..)
            .map(|(_, edit)| edit.document.record.clone()),
    );
    records
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArchiveTableRow {
    pub(crate) index: usize,
    pub(crate) name: String,
    pub(crate) subtitle: String,
    pub(crate) compression: &'static str,
    pub(crate) unpacked_size_bytes: u64,
    pub(crate) unpacked_size: String,
    pub(crate) stored_size: String,
    pub(crate) ratio: String,
    pub(crate) error: bool,
}

impl From<PacketRecord> for ArchiveTableRow {
    fn from(record: PacketRecord) -> Self {
        let compression = record.compression_label();
        let unpacked_size_bytes = record.unpacked_data_size();
        let stored_size_bytes = record.stored_data_size();
        let unpacked_size = format_bytes(unpacked_size_bytes);
        let stored_size = format_bytes(stored_size_bytes);
        let ratio = record.ratio_label();
        let error = record.error.is_some();
        Self {
            index: record.index,
            name: record.info.name,
            subtitle: format!("RSG packet #{}", record.index),
            compression,
            unpacked_size_bytes,
            unpacked_size,
            stored_size,
            ratio,
            error,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ArchiveRowFilter {
    pub(crate) rows: Arc<Vec<ArchiveTableRow>>,
    pub(crate) query: String,
    pub(crate) sort: BrowserSort,
}

impl PartialEq for ArchiveRowFilter {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.rows, &other.rows) && self.query == other.query && self.sort == other.sort
    }
}

impl ArchiveRowFilter {
    pub(crate) fn indices(&self) -> Arc<Vec<usize>> {
        let query = self.query.to_ascii_lowercase();
        let mut indices = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(row_index, row)| {
                (query.is_empty() || row.name.to_ascii_lowercase().contains(&query))
                    .then_some(row_index)
            })
            .collect::<Vec<_>>();
        indices.sort_by(|left, right| {
            compare_archive_rows(&self.rows[*left], &self.rows[*right], self.sort)
        });
        Arc::new(indices)
    }
}

fn compare_archive_rows(
    left: &ArchiveTableRow,
    right: &ArchiveTableRow,
    sort: BrowserSort,
) -> Ordering {
    let primary = match sort.key {
        BrowserSortKey::Name => compare_text(&left.name, &right.name),
        BrowserSortKey::Type => compare_text(left.compression, right.compression),
        BrowserSortKey::Size => left.unpacked_size_bytes.cmp(&right.unpacked_size_bytes),
    };
    sort.apply(primary)
        .then_with(|| compare_text(&left.name, &right.name))
        .then_with(|| left.index.cmp(&right.index))
}

#[derive(Clone)]
pub(crate) struct PacketItemSource {
    pub(crate) index: Option<Arc<PacketDirectoryIndex>>,
    pub(crate) files: Option<Arc<Vec<UnpackedFile>>>,
    pub(crate) directory: Vec<String>,
    pub(crate) virtual_directories: BTreeSet<Vec<String>>,
    pub(crate) query: String,
    pub(crate) sort: BrowserSort,
}

impl PartialEq for PacketItemSource {
    fn eq(&self, other: &Self) -> bool {
        (match (&self.index, &other.index) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }) && self.directory == other.directory
            && (match (&self.files, &other.files) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            })
            && self.virtual_directories == other.virtual_directories
            && self.query == other.query
            && self.sort == other.sort
    }
}

impl PacketItemSource {
    pub(crate) fn items(&self) -> Arc<Vec<BrowserItem>> {
        let query = self.query.to_ascii_lowercase();
        let Some(index) = self.index.as_ref() else {
            return Arc::new(Vec::new());
        };
        let mut items = index
            .items_with_virtual(&self.directory, self.virtual_directories.iter())
            .into_iter()
            .filter(|item| query.is_empty() || item.name().to_ascii_lowercase().contains(&query))
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            compare_packet_items(left, right, self.files.as_deref(), self.sort)
        });
        Arc::new(items)
    }
}

fn compare_packet_items(
    left: &BrowserItem,
    right: &BrowserItem,
    files: Option<&Vec<UnpackedFile>>,
    sort: BrowserSort,
) -> Ordering {
    let group = packet_item_group(left).cmp(&packet_item_group(right));
    if group != Ordering::Equal {
        return group;
    }

    let primary = match sort.key {
        BrowserSortKey::Name => compare_text(left.name(), right.name()),
        BrowserSortKey::Type => compare_text(
            packet_item_type(left, files),
            packet_item_type(right, files),
        ),
        BrowserSortKey::Size => packet_item_size(left, files).cmp(&packet_item_size(right, files)),
    };
    sort.apply(primary)
        .then_with(|| compare_text(left.name(), right.name()))
        .then_with(|| packet_item_identity(left).cmp(&packet_item_identity(right)))
}

fn packet_item_group(item: &BrowserItem) -> u8 {
    match item {
        BrowserItem::Directory { .. } => 0,
        BrowserItem::File { .. } => 1,
    }
}

fn packet_item_type<'a>(item: &BrowserItem, files: Option<&'a Vec<UnpackedFile>>) -> &'a str {
    match item {
        BrowserItem::Directory { .. } => "文件夹",
        BrowserItem::File { file_index, .. } => files
            .and_then(|files| files.get(*file_index))
            .map(|file| file_kind(&file.path, file.is_part1))
            .unwrap_or("File"),
    }
}

fn packet_item_size(item: &BrowserItem, files: Option<&Vec<UnpackedFile>>) -> u64 {
    match item {
        BrowserItem::Directory { byte_len, .. } => *byte_len,
        BrowserItem::File { file_index, .. } => files
            .and_then(|files| files.get(*file_index))
            .map_or(0, |file| file.data.len() as u64),
    }
}

fn packet_item_identity(item: &BrowserItem) -> usize {
    match item {
        BrowserItem::Directory { .. } => 0,
        BrowserItem::File { file_index, .. } => file_index.saturating_add(1),
    }
}

fn compare_text(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
        .then_with(|| left.cmp(right))
}

#[derive(Clone)]
pub(crate) struct PacketTreeSource {
    pub(crate) index: Option<Arc<PacketDirectoryIndex>>,
    pub(crate) current_directory: Vec<String>,
    pub(crate) virtual_directories: BTreeSet<Vec<String>>,
}

impl PartialEq for PacketTreeSource {
    fn eq(&self, other: &Self) -> bool {
        (match (&self.index, &other.index) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }) && self.current_directory == other.current_directory
            && self.virtual_directories == other.virtual_directories
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PacketTreeRow {
    pub(crate) name: String,
    pub(crate) path: Vec<String>,
    pub(crate) depth: usize,
    pub(crate) file_count: usize,
    pub(crate) is_current: bool,
    pub(crate) is_expanded: bool,
}

impl PacketTreeSource {
    pub(crate) fn rows(&self) -> Arc<Vec<PacketTreeRow>> {
        let Some(index) = self.index.as_ref() else {
            return Arc::new(Vec::new());
        };

        let mut rows = Vec::new();
        let mut parent = Vec::<String>::new();
        for depth in 0..=self.current_directory.len() {
            rows.extend(
                index
                    .items_with_virtual(&parent, self.virtual_directories.iter())
                    .into_iter()
                    .filter_map(|item| match item {
                        BrowserItem::Directory {
                            name,
                            path,
                            file_count,
                            ..
                        } => {
                            let is_current = path == self.current_directory;
                            let is_expanded = !is_current
                                && path.len() < self.current_directory.len()
                                && self.current_directory.starts_with(&path);
                            Some(PacketTreeRow {
                                name,
                                path,
                                depth: depth.saturating_add(2),
                                file_count,
                                is_current,
                                is_expanded,
                            })
                        }
                        BrowserItem::File { .. } => None,
                    }),
            );

            let Some(component) = self.current_directory.get(depth) else {
                break;
            };
            parent.push(component.clone());
        }
        Arc::new(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArchiveRowFilter, ArchiveTableRow, BrowserSort, BrowserSortDirection, BrowserSortKey,
        PacketDirectoryIndex, PacketItemSource, PacketTreeSource,
    };
    use rsb_archive::UnpackedFile;
    use std::collections::BTreeSet;
    use std::sync::Arc;

    #[test]
    fn filters_cached_rows_without_copying_row_payloads() {
        let rows = Arc::new(vec![
            ArchiveTableRow {
                index: 0,
                name: "PACKET_A".into(),
                subtitle: String::new(),
                compression: "Raw",
                unpacked_size_bytes: 10,
                unpacked_size: String::new(),
                stored_size: String::new(),
                ratio: String::new(),
                error: false,
            },
            ArchiveTableRow {
                index: 1,
                name: "PACKET_B".into(),
                subtitle: String::new(),
                compression: "Raw",
                unpacked_size_bytes: 20,
                unpacked_size: String::new(),
                stored_size: String::new(),
                ratio: String::new(),
                error: false,
            },
        ]);
        let indices = ArchiveRowFilter {
            rows,
            query: "b".into(),
            sort: BrowserSort::default(),
        }
        .indices();
        assert_eq!(indices.as_slice(), [1]);
    }

    #[test]
    fn archive_rows_sort_by_raw_size_in_both_directions() {
        let rows = Arc::new(vec![
            ArchiveTableRow {
                index: 0,
                name: "MEDIUM".into(),
                subtitle: String::new(),
                compression: "Raw",
                unpacked_size_bytes: 1_024,
                unpacked_size: "1.00 KB".into(),
                stored_size: "900 B".into(),
                ratio: String::new(),
                error: false,
            },
            ArchiveTableRow {
                index: 1,
                name: "SMALL".into(),
                subtitle: String::new(),
                compression: "Raw",
                unpacked_size_bytes: 900,
                unpacked_size: "900 B".into(),
                stored_size: "800 B".into(),
                ratio: String::new(),
                error: false,
            },
        ]);

        let ascending = ArchiveRowFilter {
            rows: rows.clone(),
            query: String::new(),
            sort: BrowserSort {
                key: BrowserSortKey::Size,
                direction: BrowserSortDirection::Ascending,
            },
        }
        .indices();
        let descending = ArchiveRowFilter {
            rows,
            query: String::new(),
            sort: BrowserSort {
                key: BrowserSortKey::Size,
                direction: BrowserSortDirection::Descending,
            },
        }
        .indices();

        assert_eq!(ascending.as_slice(), [1, 0]);
        assert_eq!(descending.as_slice(), [0, 1]);
    }

    #[test]
    fn packet_items_keep_directories_first_and_sort_files_by_type() {
        let files = Arc::new(vec![
            UnpackedFile {
                path: "small.rton".into(),
                data: vec![0; 2],
                is_part1: false,
                part1_info: None,
            },
            UnpackedFile {
                path: "large.wem".into(),
                data: vec![0; 10],
                is_part1: false,
                part1_info: None,
            },
            UnpackedFile {
                path: "folder/inside.json".into(),
                data: vec![0],
                is_part1: false,
                part1_info: None,
            },
        ]);
        let items = PacketItemSource {
            index: Some(Arc::new(PacketDirectoryIndex::build(&files))),
            files: Some(files),
            directory: Vec::new(),
            virtual_directories: BTreeSet::new(),
            query: String::new(),
            sort: BrowserSort {
                key: BrowserSortKey::Type,
                direction: BrowserSortDirection::Ascending,
            },
        }
        .items();

        assert_eq!(items[0].name(), "folder");
        assert_eq!(items[1].name(), "large.wem");
        assert_eq!(items[2].name(), "small.rton");
    }

    #[test]
    fn tree_rows_expand_every_level_of_the_current_directory() {
        let files = vec![
            UnpackedFile {
                path: "DATA/LEVELS/WORLD/ONE.RTON".into(),
                data: vec![1],
                is_part1: false,
                part1_info: None,
            },
            UnpackedFile {
                path: "DATA/LEVELS/BONUS.RTON".into(),
                data: vec![2],
                is_part1: false,
                part1_info: None,
            },
            UnpackedFile {
                path: "IMAGES/ATLAS.PTX".into(),
                data: vec![3],
                is_part1: true,
                part1_info: None,
            },
        ];
        let rows = PacketTreeSource {
            index: Some(Arc::new(PacketDirectoryIndex::build(&files))),
            current_directory: vec!["DATA".into(), "LEVELS".into(), "WORLD".into()],
            virtual_directories: BTreeSet::new(),
        }
        .rows();

        assert!(rows.iter().any(|row| row.path == ["IMAGES"]));
        assert!(
            rows.iter()
                .any(|row| { row.path == ["DATA"] && row.depth == 2 && row.is_expanded })
        );
        assert!(
            rows.iter()
                .any(|row| { row.path == ["DATA", "LEVELS"] && row.depth == 3 && row.is_expanded })
        );
        assert!(rows.iter().any(|row| {
            row.path == ["DATA", "LEVELS", "WORLD"] && row.depth == 4 && row.is_current
        }));
    }
}
