use crate::error::{Result, RsbError};
use crate::rsg::types::{Part1Extra, RSG_MAGIC, RsgPayload, UnpackedFile};
use crate::schema::file_list::read_file_list;
use byteorder::{LE, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::io::{Read, Seek, SeekFrom};

pub fn unpack_rsg(reader: &mut (impl Read + Seek)) -> Result<Vec<UnpackedFile>> {
    let start_pos = reader.stream_position()?;

    // Read Header
    let magic = reader.read_u32::<LE>()?;
    if magic != RSG_MAGIC {
        let found = magic.to_be_bytes();
        return Err(RsbError::InvalidMagic(
            "rsgp".to_string(),
            String::from_utf8_lossy(&found).to_string(),
        ));
    }

    let version = reader.read_u32::<LE>()?;
    if version != 3 && version != 4 {
        return Err(RsbError::InvalidVersion(version));
    }
    reader.read_u64::<LE>()?; // Skip 8 bytes

    let flags = reader.read_u32::<LE>()?;
    if flags > 3 {
        return Err(RsbError::InvalidCompression(flags));
    }

    // Read rest of header
    let _file_offset = reader.read_u32::<LE>()?;
    let part0_offset = reader.read_u32::<LE>()?;
    let part0_zlib = reader.read_u32::<LE>()?;
    let part0_size = reader.read_u32::<LE>()?;
    reader.read_u32::<LE>()?; // Skip 4
    let part1_offset = reader.read_u32::<LE>()?;
    let part1_zlib = reader.read_u32::<LE>()?;
    let part1_size = reader.read_u32::<LE>()?;
    reader.read_u64::<LE>()?; // Skip 20 bytes
    reader.read_u64::<LE>()?;
    reader.read_u32::<LE>()?;

    let file_list_length = reader.read_u32::<LE>()?;
    let file_list_offset = reader.read_u32::<LE>()?;

    let files = read_file_list::<RsgPayload, _>(
        reader,
        start_pos + file_list_offset as u64,
        file_list_length as u64,
    )?;

    // Helper to get data
    let mut outputs = Vec::new();

    // Read Data Blobs
    // Part0 Data
    let part0_data = read_packet_data(
        reader,
        start_pos,
        part0_offset as u64,
        part0_size as u64,
        part0_zlib as u64,
        flags,
        false,
    )?;

    // Part1 Data (Atlas/Textures)
    let part1_data = read_packet_data(
        reader,
        start_pos,
        part1_offset as u64,
        part1_size as u64,
        part1_zlib as u64,
        flags,
        true,
    )?;

    for (path, payload) in files {
        let (data_slice, info_offset, info_size, is_part1, part1_info) = match payload {
            RsgPayload::Part0(info) => (&part0_data, info.offset, info.size, false, None),
            RsgPayload::Part1(info) => (
                &part1_data,
                info.offset,
                info.size,
                true,
                Some(Part1Extra {
                    id: info.id,
                    width: info.width,
                    height: info.height,
                }),
            ),
        };

        let end_idx = info_offset as usize + info_size as usize;
        if end_idx <= data_slice.len() {
            let file_bytes = data_slice[info_offset as usize..end_idx].to_vec();
            outputs.push(UnpackedFile {
                path,
                data: file_bytes,
                is_part1,
                part1_info,
            });
        } else {
            return Err(RsbError::PacketDataOutOfBounds { path });
        }
    }

    Ok(outputs)
}

pub(crate) fn read_packet_data(
    reader: &mut (impl Read + Seek),
    start_pos: u64,
    offset: u64,
    size: u64,
    z_size: u64,
    flags: u32,
    is_atlas: bool,
) -> Result<Vec<u8>> {
    if size == 0 {
        return Ok(Vec::new());
    }

    let read_offset = start_pos + offset;
    reader.seek(SeekFrom::Start(read_offset))?;

    let mut raw_data = vec![0u8; z_size as usize];
    reader.read_exact(&mut raw_data)?;

    // Zlib check logic
    // Unused strict check omitted or simplified

    let is_zlib_header = |b: &[u8]| -> bool {
        if b.len() < 2 {
            return false;
        }
        b[0] == 0x78 && (b[1] == 0x01 || b[1] == 0x5E || b[1] == 0x9C || b[1] == 0xDA)
    };

    let actually_zlib = if is_atlas {
        if flags == 0 || flags == 2 {
            // Check if it LOOKS like zlib despite flags
            is_zlib_header(&raw_data)
        } else {
            true
        }
    } else if flags < 2 {
        is_zlib_header(&raw_data)
    } else {
        true
    };

    if actually_zlib {
        let mut d = ZlibDecoder::new(&raw_data[..]);
        let mut out = Vec::new(); // should reserve size?
        d.read_to_end(&mut out).map_err(|_| RsbError::Zlib)?;
        Ok(out)
    } else {
        // If size != z_size, and not zlib, what is it?
        // Should verify size matches.
        Ok(raw_data)
    }
}
