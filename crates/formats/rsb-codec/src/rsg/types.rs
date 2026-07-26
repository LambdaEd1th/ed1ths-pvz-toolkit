use crate::schema::file_list::FileListPayload;
use byteorder::{LE, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Semantic RSG FourCC (`rsgp`).
///
/// RSG stores this numeric value in little-endian order, so the first four
/// bytes on disk are `pgsr`.
pub const RSG_MAGIC: u32 = u32::from_be_bytes(*b"rsgp");

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RsgHeader {
    /// Semantic FourCC value. Written as a little-endian [`u32`].
    pub magic: u32,
    pub version: u32,
    pub flags: u32,
    pub file_offset: u32,
    pub part0_offset: u32,
    pub part0_zlib: u32,
    pub part0_size: u32,
    pub part1_offset: u32,
    pub part1_zlib: u32,
    pub part1_size: u32,
    pub file_list_length: u32,
    pub file_list_offset: u32,
}

#[derive(Debug, Clone)]
pub enum RsgPayload {
    Part0(Part0Info),
    Part1(Part1Info),
}

#[derive(Debug, Clone)]
pub struct Part0Info {
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct Part1Info {
    pub offset: u32,
    pub size: u32,
    pub id: u32,
    pub width: u32,
    pub height: u32,
}

impl FileListPayload for RsgPayload {
    fn read(reader: &mut impl Read) -> std::io::Result<Self> {
        let type_flag = reader.read_i32::<LE>()?;
        if type_flag == 1 {
            let offset = reader.read_u32::<LE>()?;
            let size = reader.read_u32::<LE>()?;
            let id = reader.read_u32::<LE>()?;

            let mut buf = [0u8; 16];
            reader.read_exact(&mut buf)?;

            // buf[0..8] = padding
            // buf[8..12] = width
            // buf[12..16] = height

            let width = u32::from_le_bytes(buf[8..12].try_into().unwrap());
            let height = u32::from_le_bytes(buf[12..16].try_into().unwrap());

            Ok(RsgPayload::Part1(Part1Info {
                offset,
                size,
                id,
                width,
                height,
            }))
        } else {
            let offset = reader.read_u32::<LE>()?;
            let size = reader.read_u32::<LE>()?;
            Ok(RsgPayload::Part0(Part0Info { offset, size }))
        }
    }

    fn write(&self, writer: &mut impl Write) -> std::io::Result<()> {
        match self {
            RsgPayload::Part0(info) => {
                writer.write_i32::<LE>(0)?;
                writer.write_u32::<LE>(info.offset)?;
                writer.write_u32::<LE>(info.size)?;
            }
            RsgPayload::Part1(info) => {
                writer.write_i32::<LE>(1)?;
                writer.write_u32::<LE>(info.offset)?;
                writer.write_u32::<LE>(info.size)?;
                writer.write_u32::<LE>(info.id)?;
                writer.write_all(&[0u8; 8])?;
                writer.write_u32::<LE>(info.width)?;
                writer.write_u32::<LE>(info.height)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpackedFile {
    pub path: String,
    #[serde(skip)]
    pub data: Vec<u8>,
    pub is_part1: bool,
    pub part1_info: Option<Part1Extra>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Part1Extra {
    pub id: u32,
    pub width: u32,
    pub height: u32,
}
