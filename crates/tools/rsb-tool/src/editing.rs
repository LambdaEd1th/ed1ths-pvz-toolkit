use crate::domain::{ArchiveDocument, PacketDocument, PacketRecord};
use rsb_archive::{
    RsbArchiveEdit, RsbPtxInfo, RsgHeader, RsgInfo, RsgPacketAddition, RsgPacketEdit,
    RsgPacketGroup, UnpackedFile, pack_rsg, rebuild_rsb, unpack_rsg,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::sync::Arc;

const PACKET_ALIGNMENT: usize = 0x1000;

#[derive(Clone)]
pub struct EditedPacket {
    pub original_paths: Arc<Vec<String>>,
    pub document: Arc<PacketDocument>,
}

impl PartialEq for EditedPacket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.original_paths, &other.original_paths)
            && Arc::ptr_eq(&self.document, &other.document)
    }
}

pub type PacketEdits = BTreeMap<usize, EditedPacket>;
pub type RemovedPackets = BTreeSet<usize>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddedFile {
    pub name: String,
    pub data: Vec<u8>,
}

pub fn repack_packet(
    original: &PacketDocument,
    name: String,
    compression_flags: u32,
    files: Vec<UnpackedFile>,
) -> Result<PacketDocument, String> {
    let version = original
        .record
        .header
        .as_ref()
        .map(|header| header.version)
        .ok_or_else(|| "RSG 包头不可用，无法编辑".to_string())?;
    let mut packed = Cursor::new(Vec::new());
    pack_rsg(&mut packed, &files, version, compression_flags).map_err(|error| error.to_string())?;
    let mut raw = packed.into_inner();
    raw.resize(raw.len().next_multiple_of(PACKET_ALIGNMENT), 0);
    let header = RsgHeader::read_from(&mut Cursor::new(&raw)).map_err(|error| error.to_string())?;
    let mut record = original.record.clone();
    record.info.name = name;
    record.info.ptx_number = u32::try_from(files.iter().filter(|file| file.is_part1).count())
        .map_err(|_| "RSG 纹理数量超过 u32".to_string())?;
    record.info.rsg_length =
        u32::try_from(raw.len()).map_err(|_| "RSG 包大小超过 4 GiB".to_string())?;
    record.info.packet_head_info = raw.get(16..48).map(<[u8]>::to_vec);
    record.header = Some(header);
    record.error = None;
    Ok(PacketDocument::new(record, raw, files))
}

pub fn create_empty_packet(
    packet_index: usize,
    name: String,
    version: u32,
    compression_flags: u32,
) -> Result<PacketDocument, String> {
    let files = Vec::new();
    let mut packed = Cursor::new(Vec::new());
    pack_rsg(&mut packed, &files, version, compression_flags).map_err(|error| error.to_string())?;
    packet_document_from_raw(packet_index, name, 0, 0, packed.into_inner(), files)
}

pub fn import_replacement_packet(
    original: &PacketDocument,
    raw: Vec<u8>,
) -> Result<PacketDocument, String> {
    let header = RsgHeader::read_from(&mut Cursor::new(&raw)).map_err(|error| error.to_string())?;
    let files = unpack_rsg(&mut Cursor::new(&raw)).map_err(|error| error.to_string())?;
    let texture_count = files.iter().filter(|file| file.is_part1).count();
    if texture_count != original.record.info.ptx_number as usize {
        return Err(format!(
            "替换 RSG 必须保持 {} 个 Part 1 纹理，导入文件包含 {texture_count} 个",
            original.record.info.ptx_number
        ));
    }
    let mut packed = Cursor::new(Vec::new());
    pack_rsg(&mut packed, &files, header.version, header.flags)
        .map_err(|error| error.to_string())?;
    packet_document_from_raw(
        original.record.index,
        original.record.info.name.clone(),
        original.record.info.ptx_number,
        original.record.info.ptx_before_number,
        packed.into_inner(),
        files,
    )
}

fn packet_document_from_raw(
    packet_index: usize,
    name: String,
    ptx_number: u32,
    ptx_before_number: u32,
    mut raw: Vec<u8>,
    files: Vec<UnpackedFile>,
) -> Result<PacketDocument, String> {
    raw.resize(raw.len().next_multiple_of(PACKET_ALIGNMENT), 0);
    let header = RsgHeader::read_from(&mut Cursor::new(&raw)).map_err(|error| error.to_string())?;
    let rsg_length = u32::try_from(raw.len()).map_err(|_| "RSG 包大小超过 4 GiB".to_string())?;
    Ok(PacketDocument::new(
        PacketRecord {
            index: packet_index,
            info: RsgInfo {
                name,
                rsg_offset: 0,
                rsg_length,
                pool_index: i32::try_from(packet_index)
                    .map_err(|_| "RSG 包索引超过 i32".to_string())?,
                ptx_number,
                ptx_before_number,
                packet_head_info: raw.get(16..48).map(<[u8]>::to_vec),
            },
            header: Some(header),
            error: None,
        },
        raw,
        files,
    ))
}

pub fn record_edit(
    edits: &mut PacketEdits,
    previous: &PacketDocument,
    document: PacketDocument,
) -> Arc<PacketDocument> {
    let packet_index = document.record.index;
    let original_paths = edits
        .get(&packet_index)
        .map(|edit| edit.original_paths.clone())
        .unwrap_or_else(|| {
            Arc::new(
                previous
                    .files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect(),
            )
        });
    let document = Arc::new(document);
    edits.insert(
        packet_index,
        EditedPacket {
            original_paths,
            document: document.clone(),
        },
    );
    document
}

pub fn record_addition(edits: &mut PacketEdits, document: PacketDocument) -> Arc<PacketDocument> {
    let packet_index = document.record.index;
    let document = Arc::new(document);
    edits.insert(
        packet_index,
        EditedPacket {
            original_paths: Arc::new(Vec::new()),
            document: document.clone(),
        },
    );
    document
}

/// Shifts the global PTX range of packets after `packet_index`.
///
/// The source bytes and original PTX table remain untouched so dirty-state
/// comparison and the final archive rebuild still have an immutable baseline.
/// Only the in-memory packet ranges and header count are updated for continued
/// browsing before the archive is saved.
pub fn shift_texture_ranges_after(
    archive: &ArchiveDocument,
    edits: &mut PacketEdits,
    packet_index: usize,
    delta: i64,
) -> Result<ArchiveDocument, String> {
    if delta == 0 {
        return Ok(archive.clone());
    }

    let mut updated = archive.clone();
    updated.header.ptx_number = shift_u32(updated.header.ptx_number, delta, "全局 PTX 数量")?;
    let mut records = updated.packets.as_ref().clone();
    for record in &mut records {
        if record.index == packet_index
            && let Some(edit) = edits.get(&packet_index)
        {
            record.info.ptx_number = edit.document.record.info.ptx_number;
        }
        if record.index > packet_index {
            record.info.ptx_before_number =
                shift_u32(record.info.ptx_before_number, delta, "PTX 起始序号")?;
        }
    }
    updated.packets = Arc::new(records);

    for (&index, edit) in edits.iter_mut() {
        if index <= packet_index {
            continue;
        }
        let mut document = edit.document.as_ref().clone();
        document.record.info.ptx_before_number = shift_u32(
            document.record.info.ptx_before_number,
            delta,
            "已编辑 RSG 的 PTX 起始序号",
        )?;
        edit.document = Arc::new(document);
    }
    Ok(updated)
}

fn shift_u32(value: u32, delta: i64, label: &str) -> Result<u32, String> {
    let shifted = i64::from(value)
        .checked_add(delta)
        .ok_or_else(|| format!("{label}溢出"))?;
    u32::try_from(shifted).map_err(|_| format!("{label}超出 u32 范围"))
}

pub fn archive_edit(
    edits: &PacketEdits,
    removed: &RemovedPackets,
    original_packet_count: usize,
    ptx_infos: &[RsbPtxInfo],
    original_ptx_infos: &[RsbPtxInfo],
) -> RsbArchiveEdit {
    RsbArchiveEdit {
        packets: edits
            .iter()
            .filter(|(packet_index, _)| **packet_index < original_packet_count)
            .map(|(_, edit)| {
                let document = &edit.document;
                let header = document
                    .record
                    .header
                    .as_ref()
                    .expect("edited packets always have a validated header");
                RsgPacketEdit {
                    packet_index: document.record.index,
                    original_paths: edit.original_paths.as_ref().clone(),
                    name: document.record.info.name.clone(),
                    version: header.version,
                    compression_flags: header.flags,
                    files: document.files.as_ref().clone(),
                }
            })
            .collect(),
        added_packets: edits
            .iter()
            .filter(|(packet_index, _)| **packet_index >= original_packet_count)
            .map(|edit| {
                let document = &edit.1.document;
                let header = document
                    .record
                    .header
                    .as_ref()
                    .expect("edited packets always have a validated header");
                RsgPacketAddition {
                    name: document.record.info.name.clone(),
                    version: header.version,
                    compression_flags: header.flags,
                    files: document.files.as_ref().clone(),
                    group: Some(RsgPacketGroup {
                        name: document.record.info.name.clone(),
                        is_composite: false,
                        category: ["0".into(), String::new()],
                    }),
                }
            })
            .collect(),
        removed_packets: removed.iter().copied().collect(),
        ptx_infos: (ptx_infos != original_ptx_infos).then(|| ptx_infos.to_vec()),
    }
}

pub fn rebuild_archive(
    archive: &ArchiveDocument,
    edit: &RsbArchiveEdit,
) -> Result<Vec<u8>, String> {
    let source = archive.source_bytes()?;
    rebuild_rsb(&source, edit).map_err(|error| error.to_string())
}

pub fn edited_packet_record(archive_record: &PacketRecord, edits: &PacketEdits) -> PacketRecord {
    edits
        .get(&archive_record.index)
        .map(|edit| edit.document.record.clone())
        .unwrap_or_else(|| archive_record.clone())
}

pub fn visible_packet_records(
    archive: &ArchiveDocument,
    edits: &PacketEdits,
    removed: &RemovedPackets,
) -> Vec<PacketRecord> {
    let original_count = archive.packets.len();
    let mut records = archive
        .packets
        .iter()
        .filter(|record| !removed.contains(&record.index))
        .map(|record| edited_packet_record(record, edits))
        .collect::<Vec<_>>();
    records.extend(
        edits
            .range(original_count..)
            .map(|(_, edit)| edit.document.record.clone()),
    );
    records
}

pub fn visible_packet_count(
    archive: &ArchiveDocument,
    edits: &PacketEdits,
    removed: &RemovedPackets,
) -> usize {
    let original_count = archive.packets.len();
    let removed_originals = removed.range(..original_count).count();
    let additions = edits.range(original_count..).count();
    original_count
        .saturating_sub(removed_originals)
        .saturating_add(additions)
}

pub fn packet_record(
    archive: &ArchiveDocument,
    edits: &PacketEdits,
    packet_index: usize,
) -> Option<PacketRecord> {
    edits
        .get(&packet_index)
        .map(|edit| edit.document.record.clone())
        .or_else(|| archive.packets.get(packet_index).cloned())
}

pub fn next_added_packet_index(original_packet_count: usize, edits: &PacketEdits) -> usize {
    edits
        .range(original_packet_count..)
        .next_back()
        .map_or(original_packet_count, |(index, _)| index.saturating_add(1))
}

pub fn normalize_archive_path(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|component| !component.is_empty() && *component != "." && *component != "..")
        .collect::<Vec<_>>()
        .join("/")
}

pub fn join_archive_path(directory: &[String], name: &str) -> String {
    let mut components = directory.to_vec();
    components.extend(
        normalize_archive_path(name)
            .split('/')
            .filter(|component| !component.is_empty())
            .map(str::to_string),
    );
    components.join("/")
}

#[cfg(test)]
mod tests {
    use super::{
        PacketEdits, RemovedPackets, archive_edit, create_empty_packet, join_archive_path,
        normalize_archive_path, record_addition, record_edit, shift_texture_ranges_after,
    };
    use crate::domain::{ArchiveDocument, PacketDocument, PacketRecord};
    use crate::loader::ArchiveSource;
    use rsb_archive::{RsbHeader, RsbPtxInfo, RsgHeader, RsgInfo};
    use std::sync::Arc;

    #[test]
    fn normalizes_added_archive_paths() {
        assert_eq!(
            normalize_archive_path(r"\DATA\\LEVELS/../ONE.RTON"),
            "DATA/LEVELS/ONE.RTON"
        );
        assert_eq!(
            join_archive_path(&["DATA".into(), "LEVELS".into()], "ONE.RTON"),
            "DATA/LEVELS/ONE.RTON"
        );
    }

    #[test]
    fn creates_an_empty_part0_rsg_as_an_archive_addition() {
        let document = create_empty_packet(3, "NewPacket".into(), 4, 2).unwrap();
        assert_eq!(document.record.info.name, "NewPacket");
        assert_eq!(document.record.index, 3);
        assert_eq!(document.record.header.as_ref().unwrap().version, 4);
        assert_eq!(document.record.header.as_ref().unwrap().flags, 2);
        assert!(document.files.is_empty());

        let mut edits = PacketEdits::new();
        record_addition(&mut edits, document);
        let edit = archive_edit(&edits, &RemovedPackets::new(), 3, &[], &[]);
        assert!(edit.packets.is_empty());
        assert_eq!(edit.added_packets.len(), 1);
        assert_eq!(edit.added_packets[0].name, "NewPacket");
        assert!(edit.added_packets[0].files.is_empty());
    }

    #[test]
    fn shifts_following_texture_ranges_in_archive_and_pending_edits() {
        let packet_record = |index, ptx_before_number| PacketRecord {
            index,
            info: RsgInfo {
                name: format!("Packet{index}"),
                rsg_offset: 0,
                rsg_length: 0,
                pool_index: index as i32,
                ptx_number: 1,
                ptx_before_number,
                packet_head_info: None,
            },
            header: Some(RsgHeader::default()),
            error: None,
        };
        let records = vec![packet_record(0, 0), packet_record(1, 1)];
        let archive = ArchiveDocument {
            display_name: "memory.rsb".into(),
            byte_len: 0,
            header: RsbHeader {
                ptx_number: 2,
                ..Default::default()
            },
            resource_count: 0,
            file_index: Arc::new(Vec::new()),
            packets: Arc::new(records.clone()),
            ptx_infos: Arc::new(vec![RsbPtxInfo::default(); 2]),
            warnings: Arc::new(Vec::new()),
            channel_order_mode: crate::domain::ArchiveChannelOrderMode::Auto,
            source: ArchiveSource::Memory(Arc::new(Vec::new())),
        };
        let later = PacketDocument::new(records[1].clone(), Vec::new(), Vec::new());
        let mut edits = PacketEdits::new();
        record_edit(&mut edits, &later, later.clone());

        let shifted = shift_texture_ranges_after(&archive, &mut edits, 0, 1).unwrap();
        assert_eq!(shifted.header.ptx_number, 3);
        assert_eq!(shifted.packets[0].info.ptx_before_number, 0);
        assert_eq!(shifted.packets[1].info.ptx_before_number, 2);
        assert_eq!(
            edits[&1].document.record.info.ptx_before_number,
            shifted.packets[1].info.ptx_before_number
        );
    }
}
