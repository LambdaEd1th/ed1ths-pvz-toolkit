//! Reuse the codec's validated rebuild with tiny stand-ins for unchanged packets.
//! The resulting plan combines new metadata/packets with ranges of the original
//! file. No unedited payload enters WASM memory, even for multi-gigabyte archives.
use rsb_archive::{Rsb, RsbArchiveEdit, RsbWriter};
use std::{io::Cursor, sync::Arc};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn composed_save_matches_full_rebuild_for_edits_and_structural_changes() {
        let source = crate::edit_fixture::bytes();
        let archive = crate::loader::open_memory("test.rsb".into(), source.clone()).unwrap();
        let edit = rsb_archive::RsgPacketEdit {
            packet_index: 2,
            original_paths: vec!["data/config.txt".into()],
            name: "RENAMED".into(),
            version: 4,
            compression_flags: 2,
            files: vec![crate::edit_fixture::file(
                "data/new.txt",
                b"changed".to_vec(),
            )],
        };
        let addition = rsb_archive::RsgPacketAddition {
            name: "ADDED".into(),
            version: 4,
            compression_flags: 3,
            files: vec![crate::edit_fixture::file(
                "data/added.txt",
                b"added".to_vec(),
            )],
            group: None,
        };
        for change in [
            RsbArchiveEdit::default(),
            RsbArchiveEdit {
                packets: vec![edit.clone()],
                ..Default::default()
            },
            RsbArchiveEdit {
                removed_packets: vec![0],
                added_packets: vec![addition],
                packets: vec![edit],
                ..Default::default()
            },
        ] {
            let expected = rsb_archive::rebuild_rsb(&source, &change).unwrap();
            let actual = rebuild_native(&archive, &change).unwrap();
            let differences = actual
                .iter()
                .zip(&expected)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| (i, *a, *b))
                .take(20)
                .collect::<Vec<_>>();
            assert!(
                actual == expected,
                "length {} / {}, first differences {differences:?}",
                actual.len(),
                expected.len()
            );
        }
    }

    #[test]
    fn large_unchanged_packet_is_a_range_not_an_allocation() {
        let archive = crate::edit_fixture::archive();
        let mut metadata = archive.metadata_bytes().unwrap();
        let last = archive.packets.last().unwrap();
        let length = 1_073_741_824_u64;
        let offset = archive.header.rsg_info_begin_offset as usize
            + last.index * archive.header.rsg_info_each_length as usize
            + 132;
        put_u32(&mut metadata, offset, length).unwrap();
        let headers = archive
            .packets
            .iter()
            .map(|p| {
                archive
                    .source
                    .read_range(p.info.rsg_offset.into(), 80)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let plan = plan(metadata, &headers, &RsbArchiveEdit::default()).unwrap();
        assert!(plan.byte_len > length);
        assert!(plan.rebuilt.len() < 1024 * 1024);
        assert!(matches!(
            plan.packets.last(),
            Some(PacketSource::Original {
                length: 1_073_741_824,
                ..
            })
        ));
    }
}

pub enum PacketSource {
    Original { offset: u64, length: usize },
    Rebuilt { offset: usize, length: usize },
}

pub struct SavePlan {
    pub metadata: Vec<u8>,
    pub rebuilt: Arc<Vec<u8>>,
    pub packets: Vec<PacketSource>,
    pub byte_len: u64,
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u64) -> Result<(), String> {
    let value = u32::try_from(value).map_err(|_| "RSB 偏移或区段超过 4 GiB".to_string())?;
    bytes
        .get_mut(offset..offset + 4)
        .ok_or("RSB 索引记录越界")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

pub fn plan(
    metadata: Vec<u8>,
    packet_headers: &[Vec<u8>],
    edit: &RsbArchiveEdit,
) -> Result<SavePlan, String> {
    let mut reader = Rsb::open(Cursor::new(&metadata)).map_err(|e| e.to_string())?;
    let mut header = reader.header.clone();
    let original_table = (header.rsg_info_begin_offset, header.rsg_info_each_length);
    let original = reader.read_rsg_info().map_err(|e| e.to_string())?;
    if packet_headers.len() != original.len() {
        return Err("RSG 包头数量不匹配".into());
    }
    let mut compact = metadata;
    compact.resize(compact.len().next_multiple_of(4096), 0);
    header.information_section_size = u32::try_from(compact.len()).map_err(|_| "RSB 索引过大")?;
    for (index, bytes) in packet_headers.iter().enumerate() {
        if bytes.len() != 80 {
            return Err("RSG 固定头部长度无效".into());
        }
        let offset = compact.len();
        let record =
            header.rsg_info_begin_offset as usize + index * header.rsg_info_each_length as usize;
        put_u32(&mut compact, record + 128, offset as u64)?;
        put_u32(&mut compact, record + 132, 4096)?;
        compact.extend_from_slice(bytes);
        compact.resize(offset + 4096, 0);
    }
    RsbWriter::new(Cursor::new(&mut compact))
        .write_header(&header)
        .map_err(|e| e.to_string())?;
    let rebuilt = rsb_archive::rebuild_rsb(&compact, edit).map_err(|e| e.to_string())?;
    drop(compact);
    let mut reader = Rsb::open(Cursor::new(&rebuilt)).map_err(|e| e.to_string())?;
    let header = reader.header.clone();
    let infos = reader.read_rsg_info().map_err(|e| e.to_string())?;
    let mut metadata = rebuilt
        .get(..header.information_section_size as usize)
        .ok_or("重建后的 RSB 索引越界")?
        .to_vec();
    // Structural rebuilds append a new table and preserve the old one as opaque
    // metadata. Restore the stand-in offsets in that now-unreferenced table.
    if header.rsg_info_begin_offset != original_table.0 {
        for (index, info) in original.iter().enumerate() {
            let record = original_table.0 as usize + index * original_table.1 as usize;
            put_u32(&mut metadata, record + 128, info.rsg_offset.into())?;
            put_u32(&mut metadata, record + 132, info.rsg_length.into())?;
        }
    }
    let mut packets = Vec::new();
    let mut position = metadata.len() as u64;
    let mut old_indices = (0..original.len()).filter(|i| !edit.removed_packets.contains(i));
    for (index, info) in infos.iter().enumerate() {
        let old = old_indices.next();
        let source = match old {
            Some(old) if !edit.packets.iter().any(|e| e.packet_index == old) => {
                PacketSource::Original {
                    offset: original[old].rsg_offset.into(),
                    length: original[old].rsg_length as usize,
                }
            }
            _ => PacketSource::Rebuilt {
                offset: info.rsg_offset as usize,
                length: info.rsg_length as usize,
            },
        };
        let length = match source {
            PacketSource::Original { length, .. } | PacketSource::Rebuilt { length, .. } => length,
        };
        let padded = length.next_multiple_of(4096);
        let record =
            header.rsg_info_begin_offset as usize + index * header.rsg_info_each_length as usize;
        put_u32(&mut metadata, record + 128, position)?;
        put_u32(&mut metadata, record + 132, padded as u64)?;
        position = position.checked_add(padded as u64).ok_or("RSB 长度溢出")?;
        if position > u64::from(u32::MAX) {
            return Err("重建后的 RSB 超过 4 GiB 格式上限".into());
        }
        packets.push(source);
    }
    Ok(SavePlan {
        metadata,
        rebuilt: Arc::new(rebuilt),
        packets,
        byte_len: position,
    })
}

#[cfg(target_arch = "wasm32")]
pub async fn prepare(
    archive: Arc<crate::domain::ArchiveDocument>,
    edit: &RsbArchiveEdit,
) -> Result<SavePlan, String> {
    let metadata = archive.metadata_bytes()?;
    let mut reader = Rsb::open(Cursor::new(&metadata)).map_err(|e| e.to_string())?;
    let infos = reader.read_rsg_info().map_err(|e| e.to_string())?;
    let mut headers = Vec::with_capacity(infos.len());
    for info in infos {
        if info.rsg_length < 80
            || u64::from(info.rsg_offset) + u64::from(info.rsg_length) > archive.byte_len
        {
            return Err("RSG 包范围无效，不能安全保存".into());
        }
        #[cfg(target_arch = "wasm32")]
        let bytes = archive
            .source
            .read_range_async(info.rsg_offset.into(), 80)
            .await?;
        #[cfg(not(target_arch = "wasm32"))]
        let bytes = archive.source.read_range(info.rsg_offset.into(), 80)?;
        headers.push(bytes);
    }
    plan(metadata, &headers, edit)
}

#[cfg(target_arch = "wasm32")]
pub async fn browser_file(
    archive: Arc<crate::domain::ArchiveDocument>,
    edit: &RsbArchiveEdit,
) -> Result<dioxus_html::FileData, String> {
    use crate::loader::ArchiveSource;
    let plan = prepare(archive.clone(), edit).await?;
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(plan.metadata.as_slice()));
    for part in &plan.packets {
        let length = match *part {
            PacketSource::Original { offset, length } => {
                if offset
                    .checked_add(length as u64)
                    .is_none_or(|end| end > archive.byte_len)
                {
                    return Err("原始 RSG 包超出文件范围".into());
                }
                match &archive.source {
                    ArchiveSource::WebFile { file, .. } => {
                        let blob = crate::web_file::browser_file(file)?
                            .slice_with_f64_and_f64(offset as f64, (offset + length as u64) as f64)
                            .map_err(|e| format!("{e:?}"))?;
                        parts.push(&blob);
                    }
                    ArchiveSource::Memory(_) => {
                        parts.push(&js_sys::Uint8Array::from(
                            archive.source.read_range(offset, length)?.as_slice(),
                        ));
                    }
                }
                length
            }
            PacketSource::Rebuilt { offset, length } => {
                let bytes = plan
                    .rebuilt
                    .get(offset..offset + length)
                    .ok_or("重建后的 RSG 越界")?;
                parts.push(&js_sys::Uint8Array::from(bytes));
                length
            }
        };
        let padding = length.next_multiple_of(4096) - length;
        if padding != 0 {
            parts.push(&js_sys::Uint8Array::new_with_length(padding as u32));
        }
    }
    let file = web_sys::File::new_with_blob_sequence(&parts, &archive.display_name)
        .map_err(|e| format!("{e:?}"))?;
    if file.size() as u64 != plan.byte_len {
        return Err("重建后的归档长度校验失败".into());
    }
    Ok(dioxus_html::FileData::new(dioxus::web::WebFileData::new(
        file,
        web_sys::FileReader::new().map_err(|e| format!("{e:?}"))?,
    )))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn rebuild_native(
    archive: &crate::domain::ArchiveDocument,
    edit: &RsbArchiveEdit,
) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let plan = prepare_native(archive, edit)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(plan.byte_len as usize)
        .map_err(|_| "内存不足，无法重建归档")?;
    output.extend_from_slice(&plan.metadata);
    for part in plan.packets {
        match part {
            PacketSource::Original { offset, length } => output
                .write_all(&archive.source.read_range(offset, length)?)
                .map_err(|e| e.to_string())?,
            PacketSource::Rebuilt { offset, length } => {
                output.extend_from_slice(&plan.rebuilt[offset..offset + length])
            }
        }
        output.resize(output.len().next_multiple_of(4096), 0);
    }
    Ok(output)
}

#[cfg(not(target_arch = "wasm32"))]
fn prepare_native(
    archive: &crate::domain::ArchiveDocument,
    edit: &RsbArchiveEdit,
) -> Result<SavePlan, String> {
    let metadata = archive.metadata_bytes()?;
    let mut reader = Rsb::open(Cursor::new(&metadata)).map_err(|e| e.to_string())?;
    let infos = reader.read_rsg_info().map_err(|e| e.to_string())?;
    let headers = infos
        .iter()
        .map(|i| archive.source.read_range(i.rsg_offset.into(), 80))
        .collect::<Result<Vec<_>, _>>()?;
    plan(metadata, &headers, edit)
}
