//! Unpacked file sizes read from RSG file-list metadata.
//!
//! The file list stores an offset and size for every entry inside the (possibly
//! compressed) part data. Reading just that metadata avoids unpacking packet
//! payloads, which keeps large archives practical in the browser.

use crate::domain::ArchiveDocument;
use crate::editing::{PacketEdits, RemovedPackets};
use crate::resources::{FileSizeMap, normalize_path, physical_files};
use rsb_archive::rsg::RsgPayload;
use rsb_archive::schema::file_list::read_file_list;
use std::collections::BTreeSet;
use std::io::Cursor;
use std::sync::Arc;

const MAX_FILE_LIST_BYTES: usize = 16 * 1024 * 1024;

pub async fn load_file_sizes(
    archive: Arc<ArchiveDocument>,
    edits: PacketEdits,
    removed: RemovedPackets,
) -> FileSizeMap {
    let packets = physical_files(&archive, &edits, &removed)
        .into_iter()
        .map(|file| file.packet_index)
        .collect::<BTreeSet<_>>();
    let mut sizes = FileSizeMap::new();
    for packet_index in packets {
        if let Some(edit) = edits.get(&packet_index) {
            for file in edit.document.files.iter() {
                sizes.insert(
                    (packet_index, normalize_path(&file.path)),
                    file.data.len() as u64,
                );
            }
            continue;
        }
        let Some(packet) = archive.packets.get(packet_index) else {
            continue;
        };
        let Some(header) = packet.header.as_ref() else {
            continue;
        };
        let length = header.file_list_length as usize;
        if length == 0 || length > MAX_FILE_LIST_BYTES {
            continue;
        }
        let offset = u64::from(packet.info.rsg_offset) + u64::from(header.file_list_offset);
        #[cfg(target_arch = "wasm32")]
        let bytes = archive.source.read_range_async(offset, length).await;
        #[cfg(not(target_arch = "wasm32"))]
        let bytes = archive.source.read_range(offset, length);
        let Ok(bytes) = bytes else {
            continue;
        };
        let mut cursor = Cursor::new(bytes.as_slice());
        let Ok(files) = read_file_list::<RsgPayload, _>(&mut cursor, 0, length as u64) else {
            continue;
        };
        for (path, payload) in files {
            let size = match payload {
                RsgPayload::Part0(info) => u64::from(info.size),
                RsgPayload::Part1(info) => u64::from(info.size),
            };
            sizes.insert((packet_index, normalize_path(&path)), size);
        }
    }
    sizes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_unpacked_sizes_from_rsg_file_lists() {
        let archive = crate::edit_fixture::archive();
        let sizes = pollster::block_on(load_file_sizes(
            archive.clone(),
            PacketEdits::default(),
            RemovedPackets::default(),
        ));
        let size_for = |packet_name: &str, path: &str| {
            let packet = archive
                .packets
                .iter()
                .find(|packet| packet.info.name == packet_name)
                .expect("fixture packet");
            sizes.get(&(packet.index, normalize_path(path))).copied()
        };
        assert_eq!(size_for("Data_Common", "data/config.txt"), Some(15));
        assert_eq!(size_for("Textures_768", "atlases/test.ptx"), Some(64));
    }
}
