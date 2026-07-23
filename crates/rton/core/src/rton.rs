use serde_rton::{Value, decrypt_data, encrypt_data, from_reader, to_bytes, to_compact_bytes};
use std::io::{Read, Seek, SeekFrom};

use crate::{BinaryEncoding, CoreError, DecodedDocument, EncodeOptions, Result};

pub const ENCRYPTED_RTON_PREFIX: &[u8] = &[0x10, 0x00];
const RTON_FILE_HEADER: &[u8] = b"RTON";
const COMPACT_RTON_FILE_VERSION: u32 = 0x0001_0001;
const RTON_DONE_MARKER: &[u8] = b"DONE";

pub fn decode_rton_bytes(bytes: &[u8]) -> Result<DecodedDocument> {
    decode_rton_reader(std::io::Cursor::new(bytes))
}

pub fn decrypt_rton_bytes_if_needed(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    if bytes.starts_with(ENCRYPTED_RTON_PREFIX) {
        let mut plain = decrypt_data(bytes).map_err(|error| CoreError::Rton(error.to_string()))?;
        truncate_decrypted_rton_padding(&mut plain)?;
        Ok(Some(plain))
    } else {
        Ok(None)
    }
}

fn truncate_decrypted_rton_padding(bytes: &mut Vec<u8>) -> Result<()> {
    if let Some(index) = bytes
        .windows(RTON_DONE_MARKER.len())
        .rposition(|window| window == RTON_DONE_MARKER)
    {
        bytes.truncate(index + RTON_DONE_MARKER.len());
        return Ok(());
    }

    let logical_len = {
        let mut cursor = std::io::Cursor::new(&bytes);
        from_reader::<_, Value>(&mut cursor).map_err(|error| CoreError::Rton(error.to_string()))?;
        usize::try_from(cursor.position())
            .map_err(|_| CoreError::Rton("RTON input is too large".into()))?
    };
    bytes.truncate(logical_len);
    Ok(())
}

pub fn decode_rton_reader<R>(mut reader: R) -> Result<DecodedDocument>
where
    R: Read + Seek,
{
    let byte_len = reader
        .seek(SeekFrom::End(0))
        .map_err(|error| CoreError::Rton(error.to_string()))
        .and_then(|len| {
            usize::try_from(len).map_err(|_| CoreError::Rton("RTON input is too large".into()))
        })?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| CoreError::Rton(error.to_string()))?;

    let mut prefix = [0u8; 2];
    let prefix_len = reader
        .read(&mut prefix)
        .map_err(|error| CoreError::Rton(error.to_string()))?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|error| CoreError::Rton(error.to_string()))?;

    let (value, encrypted_source, encoding_source) = if prefix_len == ENCRYPTED_RTON_PREFIX.len()
        && prefix == ENCRYPTED_RTON_PREFIX
    {
        let mut bytes = Vec::with_capacity(byte_len);
        reader
            .read_to_end(&mut bytes)
            .map_err(|error| CoreError::Rton(error.to_string()))?;
        let plain = decrypt_data(&bytes).map_err(|error| CoreError::Rton(error.to_string()))?;
        let encoding_source = detect_rton_binary_encoding(&plain);
        (
            from_reader::<_, Value>(std::io::Cursor::new(plain))
                .map_err(|error| CoreError::Rton(error.to_string()))?,
            true,
            encoding_source,
        )
    } else {
        let encoding_source = rton_binary_encoding_from_reader(&mut reader)?;
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|error| CoreError::Rton(error.to_string()))?;
        (
            from_reader::<_, Value>(reader).map_err(|error| CoreError::Rton(error.to_string()))?,
            false,
            encoding_source,
        )
    };

    Ok(DecodedDocument::new_with_source_encoding(
        value,
        encrypted_source,
        encoding_source,
        Some(byte_len),
    ))
}

fn rton_binary_encoding_from_reader<R>(reader: &mut R) -> Result<BinaryEncoding>
where
    R: Read + Seek,
{
    let mut header = [0_u8; 8];
    let len = reader
        .read(&mut header)
        .map_err(|error| CoreError::Rton(error.to_string()))?;
    if len < header.len() {
        return Ok(BinaryEncoding::Standard);
    }
    Ok(detect_rton_binary_encoding(&header))
}

pub fn detect_rton_binary_encoding(bytes: &[u8]) -> BinaryEncoding {
    if bytes.len() >= 8
        && &bytes[..4] == RTON_FILE_HEADER
        && u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) == COMPACT_RTON_FILE_VERSION
    {
        BinaryEncoding::Compact
    } else {
        BinaryEncoding::Standard
    }
}

pub fn encode_rton_bytes(value: &Value, options: EncodeOptions) -> Result<Vec<u8>> {
    let bytes = match options.encoding {
        BinaryEncoding::Standard => to_bytes(value),
        BinaryEncoding::Compact => to_compact_bytes(value),
    }
    .map_err(|error| CoreError::Rton(error.to_string()))?;

    if options.encrypted {
        encrypt_data(&bytes).map_err(|error| CoreError::Rton(error.to_string()))
    } else {
        Ok(bytes)
    }
}

pub fn decode_hex_rton(text: &str) -> Result<DecodedDocument> {
    let bytes = hex_to_bytes(text)?;
    decode_rton_bytes(&bytes)
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(3));
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            if index.is_multiple_of(16) {
                out.push('\n');
            } else {
                out.push(' ');
            }
        }
        out.push_str(&format!("{byte:02X}"));
    }
    out
}

pub fn hex_to_bytes(text: &str) -> Result<Vec<u8>> {
    let filtered: String = text.chars().filter(|ch| ch.is_ascii_hexdigit()).collect();
    Ok(hex::decode(filtered)?)
}

pub fn format_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}
