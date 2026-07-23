use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::{DecodedDocument, EncodeOptions, TextFormat, encode_rton_bytes, value_to_text};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchExportMode {
    Rton,
    Json,
    Yaml,
    Toml,
}

impl BatchExportMode {
    pub const ALL: [Self; 4] = [Self::Rton, Self::Json, Self::Yaml, Self::Toml];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rton => "RTON",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Toml => "TOML",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Rton => "rton",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
        }
    }

    pub fn text_format(self) -> Option<TextFormat> {
        match self {
            Self::Rton => None,
            Self::Json => Some(TextFormat::Json),
            Self::Yaml => Some(TextFormat::Yaml),
            Self::Toml => Some(TextFormat::Toml),
        }
    }

    pub fn archive_token(self) -> &'static str {
        self.extension()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipFileEntry {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct ZipArchiveBuilder {
    output: Vec<u8>,
    central_parts: Vec<Vec<u8>>,
    entry_count: usize,
}

impl ZipArchiveBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_entry(&mut self, path: &str, bytes: &[u8]) -> Result<(), String> {
        if self.entry_count >= u16::MAX as usize {
            return Err("ZIP archive has too many entries".to_string());
        }

        let name_bytes = path.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            return Err(format!("ZIP path is too long: {path}"));
        }

        let offset = self.output.len();
        assert_zip_u32(bytes.len(), "ZIP entry is too large")?;
        assert_zip_u32(offset, "ZIP archive is too large")?;
        let crc = crc32(bytes);
        let (dos_time, dos_date) = zip_dos_datetime();

        push_zip_u32(&mut self.output, 0x04034b50);
        push_zip_u16(&mut self.output, 20);
        push_zip_u16(&mut self.output, 0x0800);
        push_zip_u16(&mut self.output, 0);
        push_zip_u16(&mut self.output, dos_time);
        push_zip_u16(&mut self.output, dos_date);
        push_zip_u32(&mut self.output, crc);
        push_zip_u32(&mut self.output, bytes.len() as u32);
        push_zip_u32(&mut self.output, bytes.len() as u32);
        push_zip_u16(&mut self.output, name_bytes.len() as u16);
        push_zip_u16(&mut self.output, 0);
        self.output.extend_from_slice(name_bytes);
        self.output.extend_from_slice(bytes);

        let mut central_header = Vec::with_capacity(46 + name_bytes.len());
        push_zip_u32(&mut central_header, 0x02014b50);
        push_zip_u16(&mut central_header, 20);
        push_zip_u16(&mut central_header, 20);
        push_zip_u16(&mut central_header, 0x0800);
        push_zip_u16(&mut central_header, 0);
        push_zip_u16(&mut central_header, dos_time);
        push_zip_u16(&mut central_header, dos_date);
        push_zip_u32(&mut central_header, crc);
        push_zip_u32(&mut central_header, bytes.len() as u32);
        push_zip_u32(&mut central_header, bytes.len() as u32);
        push_zip_u16(&mut central_header, name_bytes.len() as u16);
        push_zip_u16(&mut central_header, 0);
        push_zip_u16(&mut central_header, 0);
        push_zip_u16(&mut central_header, 0);
        push_zip_u16(&mut central_header, 0);
        push_zip_u32(&mut central_header, 0);
        push_zip_u32(&mut central_header, offset as u32);
        central_header.extend_from_slice(name_bytes);
        self.central_parts.push(central_header);
        self.entry_count += 1;
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        let central_offset = self.output.len();
        let central_size = byte_len_of_parts(&self.central_parts)?;
        assert_zip_u32(central_offset, "ZIP archive is too large")?;
        assert_zip_u32(central_size, "ZIP central directory is too large")?;

        self.output.reserve(central_size.saturating_add(22));
        for part in self.central_parts {
            self.output.extend_from_slice(&part);
        }
        push_zip_u32(&mut self.output, 0x06054b50);
        push_zip_u16(&mut self.output, 0);
        push_zip_u16(&mut self.output, 0);
        push_zip_u16(&mut self.output, self.entry_count as u16);
        push_zip_u16(&mut self.output, self.entry_count as u16);
        push_zip_u32(&mut self.output, central_size as u32);
        push_zip_u32(&mut self.output, central_offset as u32);
        push_zip_u16(&mut self.output, 0);

        Ok(self.output)
    }
}

pub fn encode_batch_export_document(
    doc: &DecodedDocument,
    mode: BatchExportMode,
    options: EncodeOptions,
) -> Result<Vec<u8>, String> {
    if let Some(format) = mode.text_format() {
        value_to_text(&doc.value, format)
            .map(|text| text.into_bytes())
            .map_err(|error| error.to_string())
    } else {
        encode_rton_bytes(&doc.value, options).map_err(|error| error.to_string())
    }
}

pub fn batch_output_path(path: &str, mode: BatchExportMode) -> String {
    let normalized = path.replace('\\', "/");
    let base = strip_known_rton_extension(&normalized);
    format!("{base}.{}", mode.extension())
}

fn strip_known_rton_extension(path: &str) -> String {
    let Some((stem, extension)) = path.rsplit_once('.') else {
        return non_empty_zip_path(path);
    };
    match extension.to_ascii_lowercase().as_str() {
        "rton" | "json" | "yaml" | "yml" | "toml" => non_empty_zip_path(stem),
        _ => non_empty_zip_path(path),
    }
}

fn non_empty_zip_path(path: &str) -> String {
    let cleaned = path.trim_matches('/').replace("//", "/");
    if cleaned.is_empty() {
        "rton".to_string()
    } else {
        cleaned
    }
}

pub fn unique_zip_path(path: &str, used_paths: &mut HashSet<String>) -> String {
    let clean_path = non_empty_zip_path(path);
    if used_paths.insert(clean_path.clone()) {
        return clean_path;
    }

    let slash_index = clean_path.rfind('/');
    let (directory, leaf) = slash_index
        .map(|index| (&clean_path[..=index], &clean_path[index + 1..]))
        .unwrap_or(("", clean_path.as_str()));
    let dot_index = leaf.rfind('.');
    let (stem, extension) = dot_index
        .map(|index| (&leaf[..index], &leaf[index..]))
        .unwrap_or((leaf, ""));
    for index in 2.. {
        let candidate = format!("{directory}{stem}-{index}{extension}");
        if used_paths.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!("unique zip path loop should always return")
}

pub fn create_zip_archive(entries: Vec<ZipFileEntry>) -> Result<Vec<u8>, String> {
    let mut archive = ZipArchiveBuilder::new();
    for entry in entries {
        archive.push_entry(&entry.path, &entry.bytes)?;
    }
    archive.finish()
}

fn byte_len_of_parts(parts: &[Vec<u8>]) -> Result<usize, String> {
    parts.iter().try_fold(0usize, |total, part| {
        total
            .checked_add(part.len())
            .ok_or_else(|| "ZIP archive is too large".to_string())
    })
}

fn assert_zip_u32(value: usize, message: &str) -> Result<(), String> {
    if value > u32::MAX as usize {
        Err(message.to_string())
    } else {
        Ok(())
    }
}

fn push_zip_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_zip_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn zip_dos_datetime() -> (u16, u16) {
    let time = 0;
    let date = ((2026 - 1980) << 9) | (1 << 5) | 1;
    (time, date)
}

fn crc32(bytes: &[u8]) -> u32 {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    let table = TABLE.get_or_init(create_crc32_table);
    let mut crc = 0xffff_ffff;
    for byte in bytes {
        crc = (crc >> 8) ^ table[((crc ^ u32::from(*byte)) & 0xff) as usize];
    }
    crc ^ 0xffff_ffff
}

fn create_crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (index, slot) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            value = if value & 1 == 1 {
                0xedb8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
        }
        *slot = value;
    }
    table
}
