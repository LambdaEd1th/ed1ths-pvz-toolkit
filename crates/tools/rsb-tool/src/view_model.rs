use crate::domain::{
    ArchiveDocument, BrowserItem, PacketDirectoryIndex, PacketRecord, format_bytes,
};
use crate::editing::{PacketEdits, RemovedPackets};
use std::collections::BTreeSet;
use std::sync::Arc;

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
    pub(crate) unpacked_size: String,
    pub(crate) stored_size: String,
    pub(crate) ratio: String,
    pub(crate) error: bool,
}

impl From<PacketRecord> for ArchiveTableRow {
    fn from(record: PacketRecord) -> Self {
        let compression = record.compression_label();
        let unpacked_size = format_bytes(record.unpacked_data_size());
        let stored_size = format_bytes(record.stored_data_size());
        let ratio = record.ratio_label();
        let error = record.error.is_some();
        Self {
            index: record.index,
            name: record.info.name,
            subtitle: format!("RSG packet #{}", record.index),
            compression,
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
}

impl PartialEq for ArchiveRowFilter {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.rows, &other.rows) && self.query == other.query
    }
}

impl ArchiveRowFilter {
    pub(crate) fn indices(&self) -> Arc<Vec<usize>> {
        let query = self.query.to_ascii_lowercase();
        Arc::new(
            self.rows
                .iter()
                .enumerate()
                .filter_map(|(row_index, row)| {
                    (query.is_empty() || row.name.to_ascii_lowercase().contains(&query))
                        .then_some(row_index)
                })
                .collect(),
        )
    }
}

#[derive(Clone)]
pub(crate) struct PacketItemSource {
    pub(crate) index: Option<Arc<PacketDirectoryIndex>>,
    pub(crate) directory: Vec<String>,
    pub(crate) virtual_directories: BTreeSet<Vec<String>>,
    pub(crate) query: String,
}

impl PartialEq for PacketItemSource {
    fn eq(&self, other: &Self) -> bool {
        (match (&self.index, &other.index) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        }) && self.directory == other.directory
            && self.virtual_directories == other.virtual_directories
            && self.query == other.query
    }
}

impl PacketItemSource {
    pub(crate) fn items(&self) -> Arc<Vec<BrowserItem>> {
        let query = self.query.to_ascii_lowercase();
        let Some(index) = self.index.as_ref() else {
            return Arc::new(Vec::new());
        };
        Arc::new(
            index
                .items_with_virtual(&self.directory, self.virtual_directories.iter())
                .into_iter()
                .filter(|item| {
                    query.is_empty() || item.name().to_ascii_lowercase().contains(&query)
                })
                .collect(),
        )
    }
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
    use super::{ArchiveRowFilter, ArchiveTableRow, PacketDirectoryIndex, PacketTreeSource};
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
                unpacked_size: String::new(),
                stored_size: String::new(),
                ratio: String::new(),
                error: false,
            },
        ]);
        let indices = ArchiveRowFilter {
            rows,
            query: "b".into(),
        }
        .indices();
        assert_eq!(indices.as_slice(), [1]);
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
